//! Imported panel/frame capsule 私有 world：按来源 `.mei` 路径内部命名，不进入宿主扁平 id 空间。

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde_json::Value;

use crate::model::{DatasetView, Diagnostic, LoadedResource, PanelDecl, Severity, UiNodeDecl};

fn collect_panel_import_scopes(panels: &[PanelDecl], out: &mut BTreeSet<String>) {
    for panel in panels {
        if let Some(scope) = panel
            .import_scope
            .as_deref()
            .map(str::trim)
            .filter(|p| !p.is_empty())
        {
            out.insert(scope.to_string());
        }
        for node in &panel.blocks {
            if let UiNodeDecl::Panel(nested) = node {
                collect_panel_import_scopes(std::slice::from_ref(nested), out);
            }
        }
    }
}

use super::super::materialize::{build_analysis_artifacts, imported_world_metrics_resource_id};
use super::helpers::load_resources_from_capsule_file;

fn namespaced_metric_key(capsule_path: &str, local_key: &str) -> String {
    if local_key.contains("::") {
        local_key.to_string()
    } else {
        namespaced_import_id(capsule_path, local_key)
    }
}

fn rewrite_metric_def_id_fields(value: &mut Value, namespaced_key: &str) {
    let Value::Object(map) = value else {
        return;
    };
    for field in ["id", "key"] {
        let Some(id) = map
            .get(field)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
        else {
            continue;
        };
        if !id.contains("::") {
            map.insert(field.to_string(), Value::String(namespaced_key.to_string()));
        }
    }
}

fn rename_dataset_metric_keys(dataset: &mut DatasetView, capsule_path: &str) {
    let keys: Vec<String> = dataset.metrics.keys().cloned().collect();
    let mut renamed = BTreeMap::new();
    for key in keys {
        let Some(metric) = dataset.metrics.remove(&key) else {
            continue;
        };
        renamed.insert(namespaced_metric_key(capsule_path, key.as_str()), metric);
    }
    dataset.metrics = renamed;

    let def_keys: Vec<String> = dataset.runtime_metric_defs.keys().cloned().collect();
    let mut renamed_defs = BTreeMap::new();
    for key in def_keys {
        let Some(mut value) = dataset.runtime_metric_defs.remove(&key) else {
            continue;
        };
        let namespaced = namespaced_metric_key(capsule_path, key.as_str());
        rewrite_metric_def_id_fields(&mut value, &namespaced);
        renamed_defs.insert(namespaced, value);
    }
    dataset.runtime_metric_defs = renamed_defs;

    if dataset.runtime_metric_defs.is_empty() {
        dataset.runtime_analysis_graph = Default::default();
        dataset.runtime_analysis_contracts = Default::default();
        return;
    }
    let (expanded_defs, graph, contracts) =
        build_analysis_artifacts(&dataset.runtime_metric_defs, dataset.id.as_str());
    dataset.runtime_metric_defs = expanded_defs;
    dataset.runtime_analysis_graph = graph;
    dataset.runtime_analysis_contracts = contracts;
}

/// 编译期私有 id：`{capsule_path}::{local_id}`（不进入作者态 DSL）。
pub(crate) fn namespaced_import_id(capsule_path: &str, local_id: &str) -> String {
    let path = capsule_path.trim();
    let id = local_id.trim();
    if path.is_empty() {
        return id.to_string();
    }
    format!("{path}::{id}")
}

/// 将 capsule 内资源加载为路径命名空间 id（供 imported UI 闭包消费）。
pub(crate) fn load_namespaced_capsule_resources(
    app_root: &Path,
    capsule_path: &str,
) -> anyhow::Result<Vec<LoadedResource>> {
    let capsule_path = capsule_path.trim();
    if capsule_path.is_empty() {
        return Ok(Vec::new());
    }
    let raw = load_resources_from_capsule_file(app_root, capsule_path)?;
    let mut out = Vec::new();
    for mut resource in raw {
        if resource.id.contains("::metrics") {
            if let Some(dataset) = resource.dataset.as_mut() {
                rename_dataset_metric_keys(dataset, capsule_path);
            }
            out.push(resource);
            continue;
        }
        let local_id = resource.id.clone();
        resource.id = namespaced_import_id(capsule_path, &local_id);
        if let Some(dataset) = resource.dataset.as_mut() {
            dataset.id = resource.id.clone();
            rename_dataset_metric_keys(dataset, capsule_path);
            for def in dataset.runtime_metric_defs.values_mut() {
                rewrite_value_refs(def, capsule_path);
            }
        }
        out.push(resource);
    }
    Ok(out)
}

fn rewrite_local_dataset_token(
    map: &mut serde_json::Map<String, Value>,
    field: &str,
    capsule_path: &str,
) {
    let Some(token) = map
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
    else {
        return;
    };
    if token.contains("::") || token.ends_with(".mei") || token == "__source_path__" {
        return;
    }
    let stripped = token.strip_prefix("dataset.").unwrap_or(token).trim();
    if stripped.is_empty() {
        return;
    }
    map.insert(
        field.to_string(),
        Value::String(namespaced_import_id(capsule_path, stripped)),
    );
}

fn rewrite_value_refs(value: &mut Value, capsule_path: &str) {
    match value {
        Value::Object(map) => {
            if let Some(ref_kind) = map.get("__ref").and_then(Value::as_str) {
                if matches!(
                    ref_kind,
                    "dataset" | "metric" | "resource" | "entity" | "data"
                ) {
                    if let Some(id) = map
                        .get("id")
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                    {
                        if !id.contains("::") && !id.ends_with(".mei") && id != "__source_path__" {
                            map.insert(
                                "id".to_string(),
                                Value::String(namespaced_import_id(capsule_path, id)),
                            );
                        }
                    }
                }
            }
            rewrite_local_dataset_token(map, "from_dataset", capsule_path);
            rewrite_local_dataset_token(map, "from", capsule_path);
            if map
                .get("__kind")
                .and_then(Value::as_str)
                .is_some_and(|k| k == "analysis_expr")
            {
                rewrite_local_dataset_token(map, "dataset", capsule_path);
            }
            let keys: Vec<String> = map.keys().cloned().collect();
            for key in keys {
                if let Some(child) = map.get_mut(&key) {
                    rewrite_value_refs(child, capsule_path);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                rewrite_value_refs(item, capsule_path);
            }
        }
        _ => {}
    }
}

pub(crate) fn rewrite_panel_import_refs(panel: &mut PanelDecl, capsule_path: &str) {
    rewrite_value_refs(&mut panel.props, capsule_path);
    rewrite_value_refs(&mut panel.head_props, capsule_path);
    rewrite_value_refs(&mut panel.body_props, capsule_path);
    rewrite_panel_blocks_refs(&mut panel.blocks, capsule_path);
}

fn rewrite_panel_blocks_refs(blocks: &mut [UiNodeDecl], capsule_path: &str) {
    for node in blocks {
        match node {
            UiNodeDecl::Panel(panel) => {
                rewrite_panel_import_refs(panel, capsule_path);
            }
            UiNodeDecl::Block(block) => {
                rewrite_value_refs(&mut block.props, capsule_path);
            }
            UiNodeDecl::PanelRefEmbed(_) => {}
        }
    }
}

/// 为 panel 树中全部 imported scope 附加私有资源；返回待并入 runtime 的 namespaced 资源。
pub(crate) fn finalize_private_import_world(
    app_root: &Path,
    panels: &[PanelDecl],
    host_resource_local_ids: &BTreeSet<String>,
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<LoadedResource> {
    let mut cache: BTreeMap<String, Vec<LoadedResource>> = BTreeMap::new();
    let mut runtime_imported = Vec::new();
    let mut import_scopes = BTreeSet::new();
    collect_panel_import_scopes(panels, &mut import_scopes);

    for capsule_path in import_scopes {
        if let Ok(raw) = load_resources_from_capsule_file(app_root, &capsule_path) {
            for resource in raw {
                if resource.id.contains("::metrics") {
                    continue;
                }
                let local_id = resource.id.as_str();
                if host_resource_local_ids.contains(local_id) {
                    diagnostics.push(Diagnostic {
                        severity: Severity::Warning,
                        code: "host_world_shadows_imported_panel_resource".to_string(),
                        message: format!(
                            "宿主 world 资源 id `{local_id}` 与 imported panel `{capsule_path}` 内部依赖同名，但不再覆盖 imported UI；imported panel 仍使用 `{capsule_path}::{local_id}` 私有绑定"
                        ),
                        source_path: Some(target_file.to_string()),
                    });
                }
            }
        }

        let scoped = cache.entry(capsule_path.clone()).or_insert_with(|| {
            load_namespaced_capsule_resources(app_root, &capsule_path).unwrap_or_default()
        });

        for resource in scoped {
            if runtime_imported
                .iter()
                .any(|existing: &LoadedResource| existing.id == resource.id)
            {
                continue;
            }
            runtime_imported.push(resource.clone());
        }
    }

    runtime_imported
}

pub(crate) fn resource_and_metric_ids_for_scope(
    all_resources: &[LoadedResource],
    capsule_path: &str,
) -> (BTreeSet<String>, BTreeSet<String>) {
    let prefix = format!("{}::", capsule_path.trim());
    let metrics_owner = imported_world_metrics_resource_id(capsule_path);
    let mut resource_ids = BTreeSet::new();
    let mut metric_ids = BTreeSet::new();
    for resource in all_resources {
        if resource.id.starts_with(&prefix) || resource.id == metrics_owner {
            resource_ids.insert(resource.id.clone());
            if let Some(dataset) = resource.dataset.as_ref() {
                for key in dataset.metrics.keys() {
                    metric_ids.insert(key.clone());
                }
            }
        }
    }
    (resource_ids, metric_ids)
}

pub(crate) fn host_local_resource_ids(resources: &[LoadedResource]) -> BTreeSet<String> {
    resources
        .iter()
        .filter_map(|r| {
            if r.id.contains("::") {
                None
            } else {
                Some(r.id.clone())
            }
        })
        .collect()
}
