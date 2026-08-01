use super::{
    imported_capsule_path_from_world_metrics_resource_id, namespace_runtime_metric_defs,
    namespaced_world_metric_key, WORLD_METRICS_RESOURCE_ID,
};

use std::collections::BTreeMap;

use serde_json::Value;

use crate::model::{DatasetView, LoadedResource, MetricContract, SourceDecl};

use crate::compile::entry_payload::import_scope::rewrite_imported_binding_refs;

use super::super::analysis_graph::build_analysis_artifacts;

/// 将 `world(metrics=...)` / `world.add_metric(...)` 物化为可被 runtime API 定位的 dataset 资源。
pub(crate) fn append_world_metrics_dataset_resource(
    resources: &mut Vec<LoadedResource>,
    ledger: &BTreeMap<String, crate::model::WorldMetricLedgerEntry>,
    raw_metric_values: &[Value],
) {
    append_world_metrics_dataset_resource_with_id(
        resources,
        ledger,
        raw_metric_values,
        WORLD_METRICS_RESOURCE_ID,
    );
}

pub(crate) fn append_world_metrics_dataset_resource_with_id(
    resources: &mut Vec<LoadedResource>,
    ledger: &BTreeMap<String, crate::model::WorldMetricLedgerEntry>,
    raw_metric_values: &[Value],
    resource_id: &str,
) {
    let resource_id = resource_id.trim();
    let resource_id = if resource_id.is_empty() {
        WORLD_METRICS_RESOURCE_ID
    } else {
        resource_id
    };
    if resources.iter().any(|resource| resource.id == resource_id) {
        return;
    }
    let capsule_path = imported_capsule_path_from_world_metrics_resource_id(resource_id);
    let mut metrics = BTreeMap::<String, MetricContract>::new();
    let mut raw_runtime_metric_defs = BTreeMap::<String, Value>::new();
    for value in raw_metric_values {
        let Some(key) = value
            .get("key")
            .or_else(|| value.get("id"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
        else {
            continue;
        };
        raw_runtime_metric_defs.insert(key.to_string(), value.clone());
    }
    let mut raw_runtime_metric_defs =
        namespace_runtime_metric_defs(raw_runtime_metric_defs, capsule_path.as_deref());
    if let Some(path) = capsule_path.as_deref() {
        for value in raw_runtime_metric_defs.values_mut() {
            rewrite_imported_binding_refs(value, path);
        }
    }
    let (runtime_metric_defs, runtime_analysis_graph, runtime_analysis_contracts) =
        build_analysis_artifacts(&raw_runtime_metric_defs, resource_id);
    for entry in ledger.values() {
        if entry.owner_resource_id != resource_id {
            continue;
        }
        let metric_id = namespaced_world_metric_key(capsule_path.as_deref(), entry.id.as_str());
        metrics.insert(metric_id, entry.metric.clone());
    }
    if metrics.is_empty() {
        return;
    }
    resources.push(LoadedResource {
        id: resource_id.to_string(),
        kind: "dataset".to_string(),
        title: Some("world metrics".to_string()),
        document: None,
        dataset: Some(DatasetView {
            id: resource_id.to_string(),
            title: Some("world metrics".to_string()),
            purpose: Some("direct world metrics ledger".to_string()),
            schema: Vec::new(),
            stage_schema: Vec::new(),
            columns: Vec::new(),
            rows: Vec::new(),
            source: SourceDecl {
                kind: "world_metrics".to_string(),
                path: String::new(),
                sheet: None,
                header_row: None,
                preview_rows: None,
                page_size: None,
                max_page_size: None,
                table: None,
                query: None,
                connection: None,
                primary_key: None,
                content: None,
            },
            sources: Vec::new(),
            metrics,
            runtime_metric_defs,
            runtime_analysis_graph,
            runtime_analysis_contracts,
        }),
    });
}
