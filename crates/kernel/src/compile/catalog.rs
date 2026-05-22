use std::{
    collections::{BTreeMap, HashSet},
    fs,
    path::Path,
};

use serde_json::Value;
use walkdir::WalkDir;

use crate::model::{ComponentAsset, LoadedResource};

use super::scene_payload_cache::compile_scene_payload_for_target;
use crate::typed_refs::SceneRegistry;

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

/// 仅收集 typed `panel_ref(id=..., scene_file=...)` 的外部 panel 引用；忽略带 `area=` 的旧 block embed。
fn extract_typed_panel_ref_scene_files(content: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = content;
    const NEEDLE: &str = "panel_ref(";
    while let Some(start) = rest.find(NEEDLE) {
        let chunk_start = start + NEEDLE.len();
        let Some(end_rel) = rest[chunk_start..].find(')') else {
            break;
        };
        let chunk = &rest[chunk_start..chunk_start + end_rel];
        if chunk.contains("area") && chunk.contains('=') {
            rest = &rest[chunk_start + end_rel..];
            continue;
        }
        let mut sub = chunk;
        const SF: &str = "scene_file";
        while let Some(idx) = sub.find(SF) {
            let tail = &sub[idx + SF.len()..];
            let Some(eq_idx) = tail.find('=') else {
                break;
            };
            let after_eq = tail[eq_idx + 1..].trim_start();
            if let Some(value) = parse_quoted_string(after_eq) {
                let value = value.trim();
                if !value.is_empty() {
                    out.push(value.to_string());
                }
            }
            sub = tail;
        }
        rest = &rest[chunk_start + end_rel..];
    }
    out
}

fn seed_paths_for_catalog_scan(
    app_root: &Path,
    preview_target: Option<&str>,
    route_targets: &[String],
) -> Vec<String> {
    let mut seeds = Vec::new();
    let preview = preview_target.map(str::trim).filter(|s| !s.is_empty());
    match preview {
        None => {
            for target in route_targets {
                let path = normalize_rel_path(target);
                if !path.is_empty() && path.ends_with(".mei") {
                    seeds.push(path);
                }
            }
        }
        Some("main.mei") => {
            if let Ok(main_content) = fs::read_to_string(app_root.join("main.mei")) {
                for path in extract_typed_panel_ref_scene_files(&main_content) {
                    seeds.push(normalize_rel_path(&path));
                }
            }
            if seeds.is_empty() {
                seeds.push("scenes/home.mei".to_string());
            }
        }
        Some(p) if p.starts_with("data/") || p.contains("/datasets/") => {}
        Some(p) => seeds.push(normalize_rel_path(p)),
    }
    seeds
}

/// 从预览入口 + `panel_ref` 嵌入树收集 catalog 范围；**恒返回 `Some`**，无匹配时为空过滤器（不触发全量扫描）。
pub fn build_dataset_catalog_filter(
    app_root: &Path,
    preview_target: Option<&str>,
    route_targets: &[String],
) -> DatasetCatalogFilter {
    let mut filter = DatasetCatalogFilter::default();
    let seeds = seed_paths_for_catalog_scan(app_root, preview_target, route_targets);
    let mut queue = seeds;
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
        for embed in extract_scene_file_tokens(&content) {
            let path = normalize_rel_path(&embed);
            filter.dataset_paths.insert(path.clone());
            if queued.insert(path.clone()) {
                queue.push(path);
            }
        }
        for embed in extract_typed_panel_ref_scene_files(&content) {
            let path = normalize_rel_path(&embed);
            if queued.insert(path.clone()) {
                queue.push(path);
            }
        }
    }
    if filter.is_active() {
        expand_catalog_filter_data_refs(app_root, &mut filter);
    }
    filter
}

/// 收集应用内所有 dataset 声明 `.mei`（`data/dataset/**` 或 `scenes/**/datasets/**`）。
fn collect_dataset_catalog_mei_files(app_root: &Path) -> Vec<String> {
    let mut mei_files = Vec::new();
    for root_rel in ["data/dataset", "scenes"] {
        let root = app_root.join(root_rel);
        if !root.is_dir() {
            continue;
        }
        for entry in WalkDir::new(&root)
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

fn extract_scene_file_tokens(content: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = content;
    const NEEDLE: &str = "scene_file";
    while let Some(idx) = rest.find(NEEDLE) {
        let tail = &rest[idx + NEEDLE.len()..];
        let Some(eq_idx) = tail.find('=') else {
            rest = tail;
            continue;
        };
        let after_eq = tail[eq_idx + 1..].trim_start();
        if let Some(value) = parse_quoted_string(after_eq) {
            let value = value.trim();
            if !value.is_empty() && value.ends_with(".mei") {
                out.push(value.to_string());
            }
        }
        rest = tail;
    }
    out
}

fn build_dataset_id_to_scene_file_map(app_root: &Path) -> BTreeMap<String, String> {
    let mut map = BTreeMap::<String, String>::new();
    let scenes_root = app_root.join("scenes");
    if !scenes_root.is_dir() {
        return map;
    }
    for entry in WalkDir::new(&scenes_root)
        .into_iter()
        .filter_entry(|entry| {
            let name = entry.file_name().to_string_lossy();
            entry.depth() == 0 || !matches!(name.as_ref(), ".git" | "node_modules" | "target")
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
        let rel = rel.to_string_lossy().replace('\\', "/");
        let Ok(content) = fs::read_to_string(entry.path()) else {
            continue;
        };
        for id in extract_world_dataset_ids(&content) {
            map.entry(id).or_insert(rel.clone());
        }
    }
    map
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

/// 收集 dataset 声明 `.mei`（`data/dataset/**` 或 `scenes/**`），供驾驶舱 panel 等跨入口 `metric_ref` 解析。
///
/// **硬约束**：仅当 `filter.is_active()` 且路径命中过滤器时才物化；绝不因 `filter == None` 或空过滤器而扫全库。
pub(super) fn compile_dataset_catalog_resources(
    app_root: &Path,
    source_root: &Path,
    app_decls: &Value,
    asset_map: &BTreeMap<String, ComponentAsset>,
    filter: &DatasetCatalogFilter,
) -> Vec<LoadedResource> {
    if !filter.is_active() {
        return Vec::new();
    }

    let mut by_id = BTreeMap::<String, LoadedResource>::new();

    let mut compile_rels: Vec<String> = collect_dataset_catalog_mei_files(app_root);
    for rel in &filter.dataset_paths {
        if rel.ends_with(".mei") && !compile_rels.iter().any(|r| r == rel) {
            compile_rels.push(rel.clone());
        }
    }
    let dataset_scene_files = build_dataset_id_to_scene_file_map(app_root);
    for id in &filter.resource_ids {
        if let Some(rel) = dataset_scene_files.get(id) {
            if !compile_rels.iter().any(|r| r == rel) {
                compile_rels.push(rel.clone());
            }
        }
    }
    if compile_rels.is_empty() {
        return Vec::new();
    }

    for rel in compile_rels {
        if !dataset_file_matches_filter(app_root, rel.as_str(), filter) {
            continue;
        }
        let payload = compile_scene_payload_for_target(
            app_root,
            source_root,
            app_decls,
            asset_map,
            rel.as_str(),
            None,
            &SceneRegistry::new(),
        );
        let mut dataset_resources = Vec::new();
        for resource in payload.resources {
            if resource.dataset.is_some() {
                dataset_resources.push(resource);
            }
        }
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
    use std::fs;

    #[test]
    fn inactive_filter_compiles_no_catalog_files() {
        let filter = DatasetCatalogFilter::default();
        assert!(!filter.is_active());
        let root = std::env::temp_dir().join(format!(
            "mei-catalog-inactive-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("scenes")).unwrap();
        fs::write(
            root.join("main.mei"),
            r#"app(id = "t") scene = scene_ref(scene_file = "scenes/a.mei")"#,
        )
        .unwrap();
        fs::write(root.join("scenes/a.mei"), r#"scene(id="a") world() frame()"#).unwrap();
        fs::write(root.join("scenes/b.mei"), r#"scene(id="b") world() frame()"#).unwrap();
        let out = compile_dataset_catalog_resources(
            &root,
            &root,
            &serde_json::json!([]),
            &BTreeMap::new(),
            &filter,
        );
        assert!(out.is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn build_filter_never_returns_none_and_expands_panel_ref() {
        let root = std::env::temp_dir().join(format!(
            "mei-catalog-embed-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("scenes/layouts")).unwrap();
        fs::write(
            root.join("scenes/layouts/left.mei"),
            r#"
scene(id = "left")
world()
frame(
    panels = [
        panel_ref(id = "child_panel", scene_file = "scenes/child.mei"),
    ],
)
"#,
        )
        .unwrap();
        fs::write(
            root.join("scenes/child.mei"),
            r#"
scene(id = "child")
world()
world.add_dataset(id = "child_ds", source = ds.csv("x.csv"), schema = [ds.column("a", "string")])
frame()
frame.add_panel(id = "child_panel", area = "auto", blocks = [])
"#,
        )
        .unwrap();
        let filter = build_dataset_catalog_filter(&root, Some("scenes/layouts/left.mei"), &[]);
        assert!(filter.is_active());
        assert!(filter.dataset_paths.contains("scenes/layouts/left.mei"));
        assert!(filter.dataset_paths.contains("scenes/child.mei"));
        let _ = fs::remove_dir_all(&root);
    }

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
