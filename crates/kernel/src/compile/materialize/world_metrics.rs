use std::collections::BTreeMap;

use anyhow::Result;
use serde_json::Value;

use crate::model::{DatasetView, LoadedResource, MetricContract, SourceDecl};

use super::metric_packs::materialize_legacy_metric_map;

pub const WORLD_METRICS_RESOURCE_ID: &str = "__world_metrics__";

pub(crate) fn imported_world_metrics_resource_id(relative_path: &str) -> String {
    let path = relative_path.trim();
    if path.is_empty() {
        return WORLD_METRICS_RESOURCE_ID.to_string();
    }
    format!("{WORLD_METRICS_RESOURCE_ID}::{path}::metrics")
}

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
    if resources
        .iter()
        .any(|resource| resource.id == resource_id)
    {
        return;
    }
    let mut metrics = BTreeMap::<String, MetricContract>::new();
    let mut runtime_metric_defs = BTreeMap::<String, Value>::new();
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
        runtime_metric_defs.insert(key.to_string(), value.clone());
    }
    for entry in ledger.values() {
        if entry.owner_resource_id != resource_id {
            continue;
        }
        metrics.insert(entry.id.clone(), entry.metric.clone());
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
                content: None,
            },
            sources: Vec::new(),
            metrics,
            runtime_metric_defs,
        }),
    });
}

pub(crate) fn materialize_world_metrics(
    resources: &[LoadedResource],
    metric_values: &[Value],
) -> Result<BTreeMap<String, MetricContract>> {
    let mut datasets = BTreeMap::<String, DatasetView>::new();
    for resource in resources {
        if let Some(dataset) = &resource.dataset {
            datasets.insert(resource.id.clone(), dataset.clone());
        }
    }
    let mut raw_metrics = BTreeMap::<String, Value>::new();
    for value in metric_values {
        let Some(key) = value
            .get("key")
            .or_else(|| value.get("id"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
        else {
            continue;
        };
        raw_metrics.insert(key.to_string(), value.clone());
    }
    materialize_legacy_metric_map(&raw_metrics, &[], &datasets)
}

pub(crate) fn evaluate_runtime_metric_defs(
    metric_defs: &BTreeMap<String, Value>,
    base_rows: &[Value],
    datasets: &BTreeMap<String, DatasetView>,
    metric_ids: Option<&[String]>,
) -> Result<BTreeMap<String, MetricContract>> {
    if let Some(ids) = metric_ids {
        let selected = ids
            .iter()
            .filter_map(|id| {
                metric_defs
                    .get(id)
                    .cloned()
                    .map(|value| (id.clone(), value))
            })
            .collect::<BTreeMap<_, _>>();
        return materialize_legacy_metric_map(&selected, base_rows, datasets);
    }
    materialize_legacy_metric_map(metric_defs, base_rows, datasets)
}
