use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::eval::active_authoring_helpers;
use crate::mei_config::MEI_WORKSPACE_CONFIG_FILENAME;
use crate::model::{
    ResourceDecl, WorkspaceNode, WorldSemanticDataset, WorldSemanticExplainBlock,
    WorldSemanticFileIndex, WorldSemanticMetric,
};

use super::authoring_eval::with_authoring_eval_context;
use super::load_external::load_world_from_file;
use super::materialize::WORLD_METRICS_RESOURCE_ID;

fn is_world_capsule_path(path: &str) -> bool {
    path.trim().replace('\\', "/").ends_with(".world.mei")
}

fn resolve_source_root_from_app_root(app_root: &Path) -> Option<PathBuf> {
    let mut cursor = app_root.to_path_buf();
    loop {
        if cursor.join(MEI_WORKSPACE_CONFIG_FILENAME).is_file() {
            return Some(cursor);
        }
        if !cursor.pop() {
            break;
        }
    }
    None
}

fn load_world_for_semantic_index(
    app_root: &Path,
    relative_path: &str,
) -> Option<crate::model::WorldDecl> {
    let load = || load_world_from_file(app_root, relative_path, None);
    if active_authoring_helpers().is_some() {
        return load().ok();
    }
    if let Some(source_root) = resolve_source_root_from_app_root(app_root) {
        return with_authoring_eval_context(&source_root, load).ok();
    }
    load().ok()
}

fn string_field(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        value
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(|text| text.to_string())
    })
}

fn explain_block_id(block: &Value, kind: &str, index: usize) -> String {
    string_field(
        block,
        &["id", "key", "analysis_scoped_id", "analysis_node_id"],
    )
    .unwrap_or_else(|| format!("{kind}_{index}"))
}

fn extract_explain_blocks(metric: &Value) -> Vec<WorldSemanticExplainBlock> {
    let Some(items) = metric.get("explain").and_then(Value::as_array) else {
        return Vec::new();
    };
    items
        .iter()
        .enumerate()
        .filter_map(|(index, block)| {
            let kind = string_field(block, &["kind", "__kind"])?;
            Some(WorldSemanticExplainBlock {
                id: explain_block_id(block, kind.as_str(), index),
                kind,
                label: string_field(block, &["label"]),
                by: string_field(block, &["by"]),
                support_role: string_field(block, &["support_role"]),
            })
        })
        .collect()
}

fn schema_columns_from_resource(resource: &ResourceDecl) -> Vec<String> {
    let mut columns = Vec::new();
    if let Some(dataset) = resource.dataset.as_ref() {
        if let Some(values) = dataset.get("columns").and_then(Value::as_array) {
            for column in values {
                if let Some(name) = string_field(column, &["name", "id", "key", "label"]) {
                    columns.push(name);
                }
            }
        }
        if let Some(values) = dataset.get("schema").and_then(Value::as_array) {
            for column in values {
                if let Some(name) = string_field(column, &["name", "id", "key", "label"]) {
                    columns.push(name);
                }
            }
        }
    }
    if columns.is_empty() {
        if let Some(values) = resource
            .metrics
            .as_ref()
            .and_then(|metrics| metrics.get("schema"))
            .and_then(Value::as_array)
        {
            for column in values {
                if let Some(name) = string_field(column, &["name", "id", "key", "label"]) {
                    columns.push(name);
                }
            }
        }
    }
    columns
}

fn dataset_from_resource(resource: &ResourceDecl) -> Option<WorldSemanticDataset> {
    let id = resource.id.trim();
    if id.is_empty() {
        return None;
    }
    let filter_field_count = resource
        .filters
        .as_ref()
        .and_then(Value::as_object)
        .map(|map| map.len())
        .unwrap_or(0);
    Some(WorldSemanticDataset {
        id: id.to_string(),
        title: resource
            .title
            .clone()
            .filter(|text| !text.trim().is_empty()),
        schema_columns: schema_columns_from_resource(resource),
        source_kind: resource
            .source
            .as_ref()
            .map(|source| source.kind.clone())
            .filter(|kind| !kind.trim().is_empty()),
        filter_field_count,
    })
}

fn collect_dataset_refs(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for key in ["dataset", "data_ref", "rowset_dataset_id"] {
                if let Some(raw) = map.get(key).and_then(Value::as_str) {
                    let token = raw.trim().strip_prefix("dataset.").unwrap_or(raw.trim());
                    if !token.is_empty() {
                        out.push(token.to_string());
                    }
                }
            }
            for child in map.values() {
                collect_dataset_refs(child, out);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_dataset_refs(item, out);
            }
        }
        _ => {}
    }
}

fn enrich_dataset_titles(datasets: &mut [WorldSemanticDataset], metrics: &[Value]) {
    let mut label_by_dataset: BTreeMap<String, String> = BTreeMap::new();
    for metric in metrics {
        let Some(metric_id) = string_field(metric, &["id", "key"]) else {
            continue;
        };
        let Some(label) = string_field(metric, &["label", "title"]) else {
            continue;
        };
        let mut dataset_ids = Vec::new();
        collect_dataset_refs(metric, &mut dataset_ids);
        if dataset_ids.is_empty() {
            if let Some(stem) = metric_id.strip_suffix("_count") {
                dataset_ids.push(stem.to_string());
            }
            if let Some(stem) = metric_id.strip_suffix("_rows") {
                dataset_ids.push(stem.to_string());
            }
        }
        for dataset_id in dataset_ids {
            label_by_dataset
                .entry(dataset_id)
                .or_insert_with(|| label.clone());
        }
    }
    for dataset in datasets.iter_mut() {
        if dataset
            .title
            .as_ref()
            .is_some_and(|text| !text.trim().is_empty())
        {
            continue;
        }
        if let Some(label) = label_by_dataset.get(&dataset.id) {
            dataset.title = Some(label.clone());
        }
    }
}

fn metric_from_value(metric: &Value) -> Option<WorldSemanticMetric> {
    let id = string_field(metric, &["id", "key"])?;
    Some(WorldSemanticMetric {
        id,
        label: string_field(metric, &["label", "title"]),
        unit: string_field(metric, &["unit"]),
        note: string_field(metric, &["note"]),
        explain: extract_explain_blocks(metric),
    })
}

pub(crate) fn build_world_semantic_index(
    app_root: &Path,
    relative_path: &str,
) -> Option<WorldSemanticFileIndex> {
    if !is_world_capsule_path(relative_path) {
        return None;
    }
    let world = load_world_for_semantic_index(app_root, relative_path)?;
    let mut datasets = world
        .datasets
        .iter()
        .filter_map(dataset_from_resource)
        .collect::<Vec<_>>();
    enrich_dataset_titles(&mut datasets, &world.metrics);
    datasets.sort_by(|left, right| left.id.cmp(&right.id));
    let mut metrics = world
        .metrics
        .iter()
        .filter_map(metric_from_value)
        .collect::<Vec<_>>();
    metrics.sort_by(|left, right| left.id.cmp(&right.id));
    Some(WorldSemanticFileIndex {
        world_id: world.id,
        datasets,
        metrics,
        resource_id: WORLD_METRICS_RESOURCE_ID.to_string(),
    })
}

fn workspace_child_node(
    file_path: &str,
    name: String,
    kind: &str,
    semantic_label: Option<String>,
    world_dataset_id: Option<String>,
    world_metric_id: Option<String>,
    explain_block_id: Option<String>,
) -> WorkspaceNode {
    WorkspaceNode {
        name,
        path: file_path.to_string(),
        kind: kind.to_string(),
        mei_kind: None,
        scene_export_id: None,
        world_dataset_id,
        world_metric_id,
        explain_block_id,
        semantic_label,
        children: Vec::new(),
    }
}

fn build_world_capsule_children(
    file_path: &str,
    index: &WorldSemanticFileIndex,
) -> Vec<WorkspaceNode> {
    let mut children = Vec::new();
    if !index.datasets.is_empty() {
        let dataset_nodes = index
            .datasets
            .iter()
            .map(|dataset| {
                let label = dataset
                    .title
                    .clone()
                    .filter(|text| !text.trim().is_empty())
                    .unwrap_or_else(|| dataset.id.clone());
                workspace_child_node(
                    file_path,
                    label,
                    "world_dataset",
                    Some(dataset.id.clone()),
                    Some(dataset.id.clone()),
                    None,
                    None,
                )
            })
            .collect();
        children.push(workspace_child_node(
            file_path,
            "数据集".to_string(),
            "world_group",
            None,
            None,
            None,
            None,
        ));
        if let Some(group) = children.last_mut() {
            group.children = dataset_nodes;
        }
    }
    if !index.metrics.is_empty() {
        let metric_nodes = index
            .metrics
            .iter()
            .map(|metric| {
                let label = metric
                    .label
                    .clone()
                    .filter(|text| !text.trim().is_empty())
                    .unwrap_or_else(|| metric.id.clone());
                let explain_children = metric
                    .explain
                    .iter()
                    .map(|block| {
                        let name = block
                            .label
                            .clone()
                            .filter(|text| !text.trim().is_empty())
                            .unwrap_or_else(|| block.id.clone());
                        workspace_child_node(
                            file_path,
                            name,
                            "explain_block",
                            Some(block.id.clone()),
                            None,
                            Some(metric.id.clone()),
                            Some(block.id.clone()),
                        )
                    })
                    .collect();
                let mut node = workspace_child_node(
                    file_path,
                    label,
                    "world_metric",
                    Some(metric.id.clone()),
                    None,
                    Some(metric.id.clone()),
                    None,
                );
                node.children = explain_children;
                node
            })
            .collect();
        children.push(workspace_child_node(
            file_path,
            "指标".to_string(),
            "world_group",
            None,
            None,
            None,
            None,
        ));
        if let Some(group) = children.last_mut() {
            group.children = metric_nodes;
        }
    }
    children
}

pub(crate) fn enrich_source_tree_with_world_capsules(
    app_root: &Path,
    nodes: &mut [WorkspaceNode],
    index_cache: &mut BTreeMap<String, WorldSemanticFileIndex>,
) {
    for node in nodes.iter_mut() {
        if node.kind == "dir" {
            enrich_source_tree_with_world_capsules(app_root, &mut node.children, index_cache);
            continue;
        }
        if node.kind != "file" || !is_world_capsule_path(node.path.as_str()) {
            continue;
        }
        let index = index_cache
            .entry(node.path.clone())
            .or_insert_with(|| {
                build_world_semantic_index(app_root, node.path.as_str()).unwrap_or(
                    WorldSemanticFileIndex {
                        world_id: None,
                        datasets: Vec::new(),
                        metrics: Vec::new(),
                        resource_id: WORLD_METRICS_RESOURCE_ID.to_string(),
                    },
                )
            })
            .clone();
        node.children = build_world_capsule_children(node.path.as_str(), &index);
    }
}
