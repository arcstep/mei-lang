use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    fs,
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
    sync::Mutex,
};

use serde_json::Value;
use walkdir::WalkDir;

use crate::compile::dependency_graph::DependencyGraph;
use crate::compile::scene_payload_cache::file_mtime_ms;

/// 管理页预览 scene/widget 时，仅物化脚本里通过 `metric_ref` / `data_ref` 可追溯到的数据集，避免扫全库 xlsx。
#[derive(Debug, Default)]
pub struct DatasetCatalogFilter {
    pub resource_ids: HashSet<String>,
    pub metric_ids: HashSet<String>,
    pub dataset_paths: HashSet<String>,
}

impl DatasetCatalogFilter {
    pub fn is_active(&self) -> bool {
        !self.resource_ids.is_empty()
            || !self.metric_ids.is_empty()
            || !self.dataset_paths.is_empty()
    }
}

#[derive(Debug, Clone, Default)]
struct DatasetCatalogIndex {
    dataset_ids: BTreeMap<String, String>,
    metric_ids: BTreeMap<String, String>,
}

static DATASET_CATALOG_INDEX_CACHE: Mutex<BTreeMap<String, DatasetCatalogIndex>> =
    Mutex::new(BTreeMap::new());
static DATASET_CATALOG_INDEX_CACHE_HITS: AtomicU64 = AtomicU64::new(0);
static DATASET_CATALOG_INDEX_CACHE_MISSES: AtomicU64 = AtomicU64::new(0);
const MAX_DATASET_CATALOG_INDEX_CACHE_ENTRIES: usize = 32;

/// 从预览入口 + `panel_ref` 嵌入树收集 catalog 范围；**恒返回 `Some`**，无匹配时为空过滤器（不触发全量扫描）。
pub fn build_dataset_catalog_filter(
    app_root: &Path,
    app_decls: &Value,
    dependency_graph: &DependencyGraph,
    preview_target: Option<&str>,
) -> DatasetCatalogFilter {
    let mut filter = DatasetCatalogFilter::default();
    let mut queue: Vec<String> = dependency_graph
        .catalog_seed_files(app_root, app_decls, preview_target)
        .into_iter()
        .collect();
    let mut queued = HashSet::<String>::new();
    let mut processed = HashSet::<String>::new();
    for seed in &queue {
        queued.insert(normalize_rel_path(seed));
    }
    while let Some(rel) = queue.pop() {
        let rel = normalize_rel_path(rel.as_str());
        if rel.is_empty() || !processed.insert(rel.clone()) {
            continue;
        }
        filter.dataset_paths.insert(rel.clone());
        let path = app_root.join(&rel);
        let Ok(content) = fs::read_to_string(&path) else {
            continue;
        };
        for (metric_id, from_dataset) in extract_metric_ref_tokens(&content) {
            if let Some(token) = from_dataset {
                if token.contains('/') {
                    let path = normalize_rel_path(&token);
                    filter.dataset_paths.insert(path.clone());
                    if queued.insert(path.clone()) {
                        queue.push(path);
                    }
                } else {
                    filter.resource_ids.insert(token);
                }
            } else if !metric_id.is_empty() {
                filter.metric_ids.insert(metric_id);
            }
        }
        for token in extract_from_dataset_tokens(&content) {
            if token.contains('/') {
                let path = normalize_rel_path(&token);
                filter.dataset_paths.insert(path.clone());
                if queued.insert(path.clone()) {
                    queue.push(path);
                }
            } else {
                filter.resource_ids.insert(token);
            }
        }
    }
    if filter.is_active() {
        expand_catalog_filter_data_refs(app_root, &mut filter);
    }
    filter
}

/// 收集应用内所有 dataset 声明 `.mei`（`data/dataset/**` 或 `scenes/**/datasets/**`）。
pub(crate) fn collect_dataset_catalog_mei_files(app_root: &Path) -> Vec<String> {
    let mut mei_files = Vec::new();
    for root_rel in ["data/dataset", "scenes"] {
        let root = app_root.join(root_rel);
        if !root.is_dir() {
            continue;
        }
        for entry in WalkDir::new(&root).into_iter().filter_entry(|entry| {
            let name = entry.file_name().to_string_lossy();
            if entry.depth() > 0 {
                if matches!(name.as_ref(), ".git" | "node_modules" | "target" | ".mei") {
                    return false;
                }
            }
            true
        }) {
            let Ok(entry) = entry else { continue };
            if !entry.file_type().is_file() {
                continue;
            }
            if entry.path().extension().and_then(|ext| ext.to_str()) != Some("mei") {
                continue;
            }
            let Ok(rel) = entry.path().strip_prefix(app_root) else {
                continue;
            };
            let rel = rel.to_string_lossy().replace('\\', "/");
            if root_rel == "scenes" && !rel.contains("/datasets/") {
                continue;
            }
            mei_files.push(rel);
        }
    }
    mei_files.sort();
    const MAX_DATASET_ENTRIES: usize = 256;
    mei_files.truncate(MAX_DATASET_ENTRIES);
    mei_files
}

fn build_dataset_catalog_index(app_root: &Path) -> DatasetCatalogIndex {
    let mut index = DatasetCatalogIndex::default();
    for rel in collect_dataset_catalog_mei_files(app_root) {
        let path = app_root.join(&rel);
        let Ok(content) = fs::read_to_string(&path) else {
            continue;
        };
        for id in extract_world_dataset_ids(&content) {
            index.dataset_ids.entry(id).or_insert(rel.clone());
        }
        for metric_id in extract_declared_metric_ids(&content) {
            index.metric_ids.entry(metric_id).or_insert(rel.clone());
        }
    }
    index
}

fn dataset_catalog_index_cache_key(app_root: &Path) -> String {
    let mut parts = Vec::<String>::new();
    for rel in collect_dataset_catalog_mei_files(app_root) {
        let mtime = file_mtime_ms(&app_root.join(&rel));
        parts.push(format!("{rel}@{mtime}"));
    }
    format!("{}|{}", app_root.display(), parts.join("|"))
}

fn get_cached_dataset_catalog_index(app_root: &Path) -> DatasetCatalogIndex {
    let key = dataset_catalog_index_cache_key(app_root);
    if let Ok(cache) = DATASET_CATALOG_INDEX_CACHE.lock() {
        if let Some(index) = cache.get(&key).cloned() {
            DATASET_CATALOG_INDEX_CACHE_HITS.fetch_add(1, Ordering::Relaxed);
            return index;
        }
    }

    DATASET_CATALOG_INDEX_CACHE_MISSES.fetch_add(1, Ordering::Relaxed);
    let index = build_dataset_catalog_index(app_root);
    if let Ok(mut cache) = DATASET_CATALOG_INDEX_CACHE.lock() {
        if cache.len() >= MAX_DATASET_CATALOG_INDEX_CACHE_ENTRIES {
            cache.clear();
        }
        cache.insert(key, index.clone());
    }
    index
}

pub(crate) fn dataset_catalog_index_cache_metrics_snapshot() -> (u64, u64) {
    (
        DATASET_CATALOG_INDEX_CACHE_HITS.load(Ordering::Relaxed),
        DATASET_CATALOG_INDEX_CACHE_MISSES.load(Ordering::Relaxed),
    )
}

#[cfg(test)]
pub(crate) fn clear_dataset_catalog_index_cache_for_tests() {
    if let Ok(mut cache) = DATASET_CATALOG_INDEX_CACHE.lock() {
        cache.clear();
    }
}

pub(crate) fn build_dataset_id_to_scene_file_map(app_root: &Path) -> BTreeMap<String, String> {
    get_cached_dataset_catalog_index(app_root).dataset_ids
}

pub(crate) fn resolve_dataset_catalog_compile_rels(
    app_root: &Path,
    filter: &DatasetCatalogFilter,
) -> Vec<String> {
    let mut rels = std::collections::BTreeSet::<String>::new();
    for rel in &filter.dataset_paths {
        let normalized = normalize_rel_path(rel);
        if normalized.ends_with(".mei") {
            rels.insert(normalized);
        }
    }
    let index = get_cached_dataset_catalog_index(app_root);
    for id in &filter.resource_ids {
        if let Some(rel) = index.dataset_ids.get(id) {
            rels.insert(rel.clone());
        }
    }
    for metric_id in &filter.metric_ids {
        if let Some(rel) = index.metric_ids.get(metric_id) {
            rels.insert(rel.clone());
        }
    }
    rels.into_iter().collect()
}

/// 派生数据集（如执法要素概览）在 .mei 内 `ds.data_ref` / `metric_ref(from_dataset=...)` 依赖其它 world id，需一并物化。
fn expand_catalog_filter_data_refs(app_root: &Path, filter: &mut DatasetCatalogFilter) {
    let dataset_scene_files = build_dataset_id_to_scene_file_map(app_root);
    for (id, rel) in &dataset_scene_files {
        if filter.resource_ids.contains(id) {
            filter.dataset_paths.insert(rel.clone());
        }
    }
    let mut rel_by_id = BTreeMap::<String, String>::new();
    rel_by_id.extend(dataset_scene_files);
    let mut scan_rels: Vec<String> = collect_dataset_catalog_mei_files(app_root);
    for rel in &filter.dataset_paths {
        if rel.ends_with(".mei") && !scan_rels.iter().any(|r| r == rel) {
            scan_rels.push(rel.clone());
        }
    }
    for rel in scan_rels {
        let path = app_root.join(&rel);
        let Ok(content) = fs::read_to_string(&path) else {
            continue;
        };
        for id in extract_world_dataset_ids(&content) {
            filter.resource_ids.insert(id.clone());
            rel_by_id.entry(id).or_insert(rel.clone());
        }
    }

    let mut queue: Vec<String> = filter.resource_ids.iter().cloned().collect();
    let mut expanded_ids = HashSet::<String>::new();
    while let Some(id) = queue.pop() {
        if !expanded_ids.insert(id.clone()) {
            continue;
        }
        let Some(rel) = rel_by_id.get(&id) else {
            continue;
        };
        let path = app_root.join(rel);
        let Ok(content) = fs::read_to_string(&path) else {
            continue;
        };
        for dep in extract_data_ref_tokens(&content) {
            if filter.resource_ids.insert(dep.clone()) {
                queue.push(dep);
            }
        }
    }
}

fn extract_world_dataset_ids(content: &str) -> Vec<String> {
    let mut ids = Vec::new();
    let mut rest = content;
    while let Some(idx) = rest.find("world.add_dataset(") {
        let tail = &rest[idx..];
        if let Some(id) = extract_id_after_world_add(tail) {
            ids.push(id);
        }
        rest = &rest[idx + 1..];
    }
    ids
}

fn extract_id_after_world_add(block: &str) -> Option<String> {
    let id_needle = "id";
    let idx = block.find(id_needle)?;
    let tail = &block[idx + id_needle.len()..];
    let eq = tail.find('=')?;
    parse_quoted_string(tail[eq + 1..].trim_start())
}

fn extract_data_ref_tokens(content: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = content;
    while let Some(idx) = rest.find("data_ref(") {
        let tail = &rest[idx + "data_ref(".len()..];
        if let Some(value) = parse_quoted_string(tail.trim_start()) {
            let value = value.trim();
            if !value.is_empty() && !value.contains('/') {
                out.push(value.to_string());
            }
        }
        rest = &rest[idx + 1..];
    }
    out
}

fn extract_declared_metric_ids(content: &str) -> Vec<String> {
    let mut ids = BTreeSet::<String>::new();
    for needle in ["scalar_map(", "metric("] {
        let mut rest = content;
        while let Some(idx) = rest.find(needle) {
            let tail = &rest[idx + needle.len()..];
            let Some(end_idx) = tail.find(')') else {
                break;
            };
            let args = &tail[..end_idx];
            if let Some(metric_id) = parse_named_arg_string(args, "id")
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
            {
                ids.insert(metric_id);
            }
            rest = &tail[end_idx + 1..];
        }
    }
    ids.into_iter().collect()
}

pub(crate) fn extract_from_dataset_tokens(content: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = content;
    const NEEDLE: &str = "from_dataset";
    while let Some(idx) = rest.find(NEEDLE) {
        let tail = &rest[idx + NEEDLE.len()..];
        let Some(eq_idx) = tail.find('=') else {
            rest = tail;
            continue;
        };
        let after_eq = tail[eq_idx + 1..].trim_start();
        if let Some(value) = parse_quoted_string(after_eq) {
            let value = value.trim();
            if !value.is_empty() {
                out.push(value.to_string());
            }
        }
        rest = tail;
    }
    out
}

fn parse_quoted_string(input: &str) -> Option<String> {
    let bytes = input.as_bytes();
    let quote = bytes.first()?;
    if *quote != b'"' && *quote != b'\'' {
        return None;
    }
    let mut out = String::new();
    for ch in input[1..].chars() {
        if ch as u8 == *quote {
            return Some(out);
        }
        out.push(ch);
    }
    None
}

fn normalize_rel_path(path: &str) -> String {
    path.trim().trim_start_matches("./").replace('\\', "/")
}

fn parse_named_arg_string(raw: &str, name: &str) -> Option<String> {
    let needle = format!("{name}");
    let mut rest = raw;
    while let Some(idx) = rest.find(&needle) {
        let before = rest[..idx].chars().last();
        let after = rest[idx + needle.len()..].chars().next();
        let invalid_before = before.is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_');
        let invalid_after = after.is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_');
        if invalid_before || invalid_after {
            rest = &rest[idx + needle.len()..];
            continue;
        }
        let tail = &rest[idx + needle.len()..];
        let Some(eq_idx) = tail.find('=') else {
            return None;
        };
        let after_eq = tail[eq_idx + 1..].trim_start();
        return parse_quoted_string(after_eq).map(|s| s.trim().to_string());
    }
    None
}

fn parse_metric_ref_call(raw_args: &str) -> Option<(String, Option<String>)> {
    let trimmed = raw_args.trim_start();
    let metric_id = if let Some(id) = parse_quoted_string(trimmed) {
        id.trim().to_string()
    } else if let Some(id) = parse_named_arg_string(trimmed, "id") {
        id.trim().to_string()
    } else {
        String::new()
    };
    if metric_id.is_empty() {
        return None;
    }
    let from_dataset = parse_named_arg_string(trimmed, "from_dataset")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    Some((metric_id, from_dataset))
}

pub(crate) fn extract_metric_ref_tokens(content: &str) -> Vec<(String, Option<String>)> {
    let mut out = Vec::new();
    let mut rest = content;
    const NEEDLE: &str = "metric_ref(";
    while let Some(idx) = rest.find(NEEDLE) {
        let tail = &rest[idx + NEEDLE.len()..];
        let Some(end_idx) = tail.find(')') else {
            break;
        };
        let args = &tail[..end_idx];
        if let Some(parsed) = parse_metric_ref_call(args) {
            out.push(parsed);
        }
        rest = &tail[end_idx + 1..];
    }
    out
}
