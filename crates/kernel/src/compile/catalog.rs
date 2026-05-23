use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    path::Path,
};

use serde_json::Value;
use walkdir::WalkDir;

use crate::model::{ComponentAsset, DatasetView, LoadedResource};

fn dataset_schema_width(dataset: &DatasetView) -> usize {
    if !dataset.schema.is_empty() {
        return dataset.schema.len();
    }
    dataset.columns.len()
}

fn merge_dataset_resource(existing: &mut LoadedResource, incoming: LoadedResource) {
    let Some(incoming_ds) = incoming.dataset.as_ref() else {
        return;
    };
    let Some(existing_ds) = existing.dataset.as_mut() else {
        existing.dataset = incoming.dataset.clone();
        return;
    };
    for (metric_id, metric) in &incoming_ds.metrics {
        existing_ds.metrics.insert(metric_id.clone(), metric.clone());
    }
    for (metric_id, raw) in &incoming_ds.runtime_metric_defs {
        existing_ds
            .runtime_metric_defs
            .insert(metric_id.clone(), raw.clone());
    }
    if dataset_schema_width(incoming_ds) > dataset_schema_width(existing_ds) {
        existing_ds.schema = incoming_ds.schema.clone();
        existing_ds.stage_schema = incoming_ds.stage_schema.clone();
        existing_ds.columns = incoming_ds.columns.clone();
        existing_ds.rows = incoming_ds.rows.clone();
        existing_ds.source = incoming_ds.source.clone();
        existing_ds.sources = incoming_ds.sources.clone();
        if let Some(title) = incoming_ds.title.as_ref().filter(|s| !s.is_empty()) {
            existing_ds.title = Some(title.clone());
        }
    }
}

fn upsert_catalog_dataset_resource(
    by_id: &mut BTreeMap<String, LoadedResource>,
    resource: LoadedResource,
) {
    let id = resource.id.clone();
    if resource.dataset.is_none() {
        by_id.insert(id, resource);
        return;
    }
    match by_id.get_mut(&id) {
        None => {
            by_id.insert(id, resource);
        }
        Some(existing) => {
            merge_dataset_resource(existing, resource);
        }
    }
}

use super::scene_payload_cache::compile_scene_payload_for_target;
use crate::typed_refs::SceneRegistry;

/// 管理页预览 scene/widget 时，仅物化脚本里通过 `metric_ref` / `data_ref` 可追溯到的数据集，避免扫全库 xlsx。
#[derive(Debug, Default)]
pub struct DatasetCatalogFilter {
    pub resource_ids: HashSet<String>,
    pub metric_ids: HashSet<String>,
    pub dataset_paths: HashSet<String>,
}

impl DatasetCatalogFilter {
    pub fn is_active(&self) -> bool {
        !self.resource_ids.is_empty() || !self.metric_ids.is_empty() || !self.dataset_paths.is_empty()
    }
}

/// 同文件内 `LAYOUT_LEFT = "scenes/..."` 形式，供 `panel_ref(..., scene_file = LAYOUT_LEFT)` 解析。
fn extract_string_assignment_constants(content: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for line in content.lines() {
        let line = line.split('#').next().unwrap_or(line).trim();
        if line.is_empty() {
            continue;
        }
        let Some(eq_idx) = line.find('=') else {
            continue;
        };
        let name = line[..eq_idx].trim();
        if name.is_empty() || name.contains(' ') || name.contains('(') {
            continue;
        }
        let after_eq = line[eq_idx + 1..].trim();
        if let Some(value) = parse_quoted_string(after_eq) {
            let value = value.trim();
            if !value.is_empty() {
                out.insert(name.to_string(), value.to_string());
            }
        }
    }
    out
}

fn resolve_scene_file_token(token: &str, constants: &HashMap<String, String>) -> Option<String> {
    let token = token.trim();
    if token.is_empty() {
        return None;
    }
    if let Some(value) = parse_quoted_string(token) {
        let value = value.trim();
        if !value.is_empty() {
            return Some(value.to_string());
        }
        return None;
    }
    let ident = token
        .split(|c: char| c == ',' || c == ')' || c.is_whitespace())
        .next()
        .unwrap_or(token)
        .trim();
    constants.get(ident).cloned()
}

/// 仅收集 typed `panel_ref(id=..., scene_file=...)` 的外部 panel 引用；忽略带 `area=` 的旧 block embed。
fn extract_typed_panel_ref_scene_files(content: &str) -> Vec<String> {
    let constants = extract_string_assignment_constants(content);
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
            if let Some(value) = resolve_scene_file_token(after_eq, &constants) {
                out.push(value);
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

fn file_declares_metric_id(content: &str, id: &str) -> bool {
    content.contains(&format!("scalar_map(id = \"{id}\""))
        || content.contains(&format!("scalar_map(id=\"{id}\""))
        || content.contains(&format!("metric(id = \"{id}\""))
        || content.contains(&format!("metric(id=\"{id}\""))
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

fn extract_metric_ref_tokens(content: &str) -> Vec<(String, Option<String>)> {
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

fn dataset_file_matches_filter(app_root: &Path, rel: &str, filter: &DatasetCatalogFilter) -> bool {
    if filter.dataset_paths.contains(rel) {
        return true;
    }
    if filter.resource_ids.is_empty() && filter.metric_ids.is_empty() {
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
        || filter
            .metric_ids
            .iter()
            .any(|metric_id| file_declares_metric_id(&content, metric_id))
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
            upsert_catalog_dataset_resource(&mut by_id, resource);
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
        upsert_catalog_dataset_resource(&mut by_id, resource);
    }
    for resource in scene_resources {
        upsert_catalog_dataset_resource(&mut by_id, resource);
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

    #[test]
    fn extract_metric_ref_tokens_supports_positional_and_named_id() {
        let src = r#"
            component("x", props = {"metric": metric_ref("sales_total")})
            component("x", props = {"metric": metric_ref(id = "alerts_total", from_dataset = "warning_view")})
        "#;
        let tokens = extract_metric_ref_tokens(src);
        assert!(tokens.contains(&("sales_total".to_string(), None)));
        assert!(tokens.contains(&(
            "alerts_total".to_string(),
            Some("warning_view".to_string())
        )));
    }
}
