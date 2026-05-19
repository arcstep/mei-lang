use std::{
    collections::{BTreeMap, HashSet},
    fs,
    path::Path,
    sync::Mutex,
    time::UNIX_EPOCH,
};

use serde_json::Value;
use walkdir::WalkDir;

use crate::model::{ComponentAsset, LoadedResource};

use super::entry_payload::compile_scene_payload_for_target;

static DATASET_CATALOG_COMPILE_CACHE: Mutex<BTreeMap<String, Vec<LoadedResource>>> =
    Mutex::new(BTreeMap::new());

fn file_mtime_ms(path: &Path) -> u128 {
    fs::metadata(path)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

fn catalog_compile_cache_key(app_root: &Path, rel: &str) -> Option<String> {
    let path = app_root.join(rel);
    if !path.is_file() {
        return None;
    }
    Some(format!(
        "{}|{rel}|{}",
        app_root.display(),
        file_mtime_ms(&path)
    ))
}

fn take_cached_catalog_resources(key: &str) -> Option<Vec<LoadedResource>> {
    DATASET_CATALOG_COMPILE_CACHE
        .lock()
        .ok()
        .and_then(|cache| cache.get(key).cloned())
}

fn store_cached_catalog_resources(key: String, resources: Vec<LoadedResource>) {
    if let Ok(mut cache) = DATASET_CATALOG_COMPILE_CACHE.lock() {
        if cache.len() >= 96 {
            cache.clear();
        }
        cache.insert(key, resources);
    }
}

/// 管理页预览 scene/widget 时，仅物化脚本里 `from_dataset` 引用到的数据集，避免扫全库 xlsx。
#[derive(Debug, Default)]
pub struct DatasetCatalogFilter {
    pub resource_ids: HashSet<String>,
    pub dataset_paths: HashSet<String>,
}

impl DatasetCatalogFilter {
    pub fn is_active(&self) -> bool {
        !self.resource_ids.is_empty() || !self.dataset_paths.is_empty()
    }
}

/// 从预览入口 `.mei` 提取 `from_dataset = "..."`（world id 或 `data/dataset/...` 路径）。
pub fn build_dataset_catalog_filter(
    app_root: &Path,
    preview_target: Option<&str>,
) -> Option<DatasetCatalogFilter> {
    let preview = preview_target?.trim();
    if preview.is_empty() || preview == "main.mei" || preview.starts_with("data/") {
        return None;
    }
    let path = app_root.join(preview);
    let content = fs::read_to_string(&path).ok()?;
    let mut filter = DatasetCatalogFilter::default();
    for token in extract_from_dataset_tokens(&content) {
        if token.contains('/') {
            filter
                .dataset_paths
                .insert(normalize_rel_path(&token));
        } else {
            filter.resource_ids.insert(token);
        }
    }
    if filter.is_active() {
        expand_catalog_filter_data_refs(app_root, &mut filter);
        Some(filter)
    } else {
        None
    }
}

/// 派生数据集（如执法要素概览）在 .mei 内 `ds.data_ref` 依赖其它 world id，需一并物化。
fn expand_catalog_filter_data_refs(app_root: &Path, filter: &mut DatasetCatalogFilter) {
    let dataset_root = app_root.join("data/dataset");
    if !dataset_root.is_dir() {
        return;
    }
    let mut rel_by_id = BTreeMap::<String, String>::new();
    for entry in WalkDir::new(&dataset_root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("mei") {
            continue;
        }
        let Ok(rel) = path.strip_prefix(app_root) else {
            continue;
        };
        let rel = rel.to_string_lossy().replace('\\', "/");
        let Ok(content) = fs::read_to_string(path) else {
            continue;
        };
        for id in extract_world_dataset_ids(&content) {
            rel_by_id.entry(id).or_insert_with(|| rel.clone());
        }
    }

    let mut queue: Vec<String> = filter.resource_ids.iter().cloned().collect();
    let mut visited = HashSet::<String>::new();
    while let Some(id) = queue.pop() {
        if !visited.insert(id.clone()) {
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

fn extract_from_dataset_tokens(content: &str) -> Vec<String> {
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
    path.trim()
        .trim_start_matches("./")
        .replace('\\', "/")
}

fn file_declares_resource_id(content: &str, id: &str) -> bool {
    content.contains(&format!("id = \"{id}\""))
        || content.contains(&format!("id=\"{id}\""))
}

fn dataset_file_matches_filter(app_root: &Path, rel: &str, filter: &DatasetCatalogFilter) -> bool {
    if filter.dataset_paths.contains(rel) {
        return true;
    }
    if filter.resource_ids.is_empty() {
        return false;
    }
    let path = app_root.join(rel);
    let Ok(content) = fs::read_to_string(&path) else {
        return false;
    };
    filter
        .resource_ids
        .iter()
        .any(|id| file_declares_resource_id(&content, id))
}

/// 收集 `data/dataset/**/*.mei` 中声明的 world 数据集资源，供驾驶舱 widget 等跨入口 `metric_ref` 解析。
pub(super) fn compile_dataset_catalog_resources(
    app_root: &Path,
    app_decls: &Value,
    asset_map: &BTreeMap<String, ComponentAsset>,
    filter: Option<&DatasetCatalogFilter>,
) -> Vec<LoadedResource> {
    let dataset_root = app_root.join("data/dataset");
    if !dataset_root.is_dir() {
        return Vec::new();
    }

    let mut by_id = BTreeMap::<String, LoadedResource>::new();
    const MAX_DATASET_ENTRIES: usize = 256;

    let mut mei_files = Vec::new();
    for entry in WalkDir::new(&dataset_root)
        .into_iter()
        .filter_entry(|entry| {
            let name = entry.file_name().to_string_lossy();
            if entry.depth() > 0 {
                if matches!(name.as_ref(), ".git" | "node_modules" | "target" | ".mei") {
                    return false;
                }
            }
            true
        })
    {
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
        mei_files.push(rel.to_string_lossy().replace('\\', "/"));
    }
    mei_files.sort();
    mei_files.truncate(MAX_DATASET_ENTRIES);

    for rel in mei_files {
        if let Some(filter) = filter {
            if filter.is_active() && !dataset_file_matches_filter(app_root, rel.as_str(), filter) {
                continue;
            }
        }
        let cached = catalog_compile_cache_key(app_root, rel.as_str())
            .as_deref()
            .and_then(take_cached_catalog_resources);
        let dataset_resources = if let Some(resources) = cached {
            resources
        } else {
            let payload = compile_scene_payload_for_target(
                app_root,
                app_decls,
                asset_map,
                rel.as_str(),
                None,
            );
            let mut dataset_resources = Vec::new();
            for resource in payload.resources {
                if resource.dataset.is_some() {
                    dataset_resources.push(resource);
                }
            }
            if let Some(key) = catalog_compile_cache_key(app_root, rel.as_str()) {
                store_cached_catalog_resources(key, dataset_resources.clone());
            }
            dataset_resources
        };
        for resource in dataset_resources {
            by_id.insert(resource.id.clone(), resource);
        }
    }

    by_id.into_values().collect()
}

pub(super) fn merge_resource_catalog(
    catalog: Vec<LoadedResource>,
    scene_resources: Vec<LoadedResource>,
) -> Vec<LoadedResource> {
    let mut by_id = BTreeMap::<String, LoadedResource>::new();
    for resource in catalog {
        by_id.insert(resource.id.clone(), resource);
    }
    for resource in scene_resources {
        by_id.insert(resource.id.clone(), resource);
    }
    by_id.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_from_dataset_tokens_parses_world_id_and_path() {
        let src = r#"
            metric_ref("a", from_dataset = "typical_cases")
            metric_ref("b", from_dataset = "data/dataset/典型案例/监督典型案例.mei")
        "#;
        let tokens = extract_from_dataset_tokens(src);
        assert!(tokens.contains(&"typical_cases".to_string()));
        assert!(tokens.iter().any(|t| t.contains("监督典型案例.mei")));
    }
}
