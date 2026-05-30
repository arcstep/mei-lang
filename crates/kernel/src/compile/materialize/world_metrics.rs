use std::collections::BTreeMap;

use anyhow::Result;
use serde_json::Value;

use crate::model::{DatasetView, LoadedResource, MetricContract, SourceDecl};

use super::analysis_graph::{build_analysis_artifacts, expand_runtime_metric_defs};
use super::metric_packs::{
    materialize_legacy_metric_map, materialize_legacy_metric_map_with_scope_and_dag,
};
use crate::compile::analysis::eval_context::{RequestDagMetrics, RuntimeMetricEvalScope};

pub const WORLD_METRICS_RESOURCE_ID: &str = "__world_metrics__";

pub(crate) fn imported_world_metrics_resource_id(relative_path: &str) -> String {
    let path = relative_path.trim();
    if path.is_empty() {
        return WORLD_METRICS_RESOURCE_ID.to_string();
    }
    format!("{WORLD_METRICS_RESOURCE_ID}::{path}::metrics")
}

const IMPORTED_WORLD_METRICS_PREFIX: &str = "__world_metrics__::";
const IMPORTED_WORLD_METRICS_SUFFIX: &str = "::metrics";

/// 从 `__world_metrics__::{capsule_path}::metrics` 解析嵌入 capsule 的相对路径。
pub fn imported_capsule_path_from_world_metrics_resource_id(resource_id: &str) -> Option<String> {
    let resource_id = resource_id.trim();
    let inner = resource_id.strip_prefix(IMPORTED_WORLD_METRICS_PREFIX)?;
    let path = inner.strip_suffix(IMPORTED_WORLD_METRICS_SUFFIX)?;
    if path.is_empty() {
        None
    } else {
        Some(path.to_string())
    }
}

/// 解析 runtime metric def 键：直查失败后，对 imported world metrics 尝试 `{capsule}::{local_id}`。
pub fn resolve_runtime_metric_def_key(
    resource_id: &str,
    metric_id: &str,
    defs: &BTreeMap<String, Value>,
) -> Option<String> {
    let metric_id = metric_id.trim();
    if metric_id.is_empty() {
        return None;
    }
    if defs.contains_key(metric_id) {
        return Some(metric_id.to_string());
    }
    let capsule_path = imported_capsule_path_from_world_metrics_resource_id(resource_id)?;
    let namespaced = if metric_id.contains("::") {
        metric_id.to_string()
    } else {
        format!("{capsule_path}::{metric_id}")
    };
    if defs.contains_key(&namespaced) {
        Some(namespaced)
    } else {
        None
    }
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
    if resources.iter().any(|resource| resource.id == resource_id) {
        return;
    }
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
    let (runtime_metric_defs, runtime_analysis_graph, runtime_analysis_contracts) =
        build_analysis_artifacts(&raw_runtime_metric_defs, resource_id);
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
            runtime_analysis_graph,
            runtime_analysis_contracts,
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
            datasets
                .entry(dataset.id.clone())
                .or_insert_with(|| dataset.clone());
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
    materialize_legacy_metric_map(&expand_runtime_metric_defs(&raw_metrics), &[], &datasets)
}

pub(crate) fn evaluate_runtime_metric_defs(
    metric_defs: &BTreeMap<String, Value>,
    base_rows: &[Value],
    datasets: &BTreeMap<String, DatasetView>,
    metric_ids: Option<&[String]>,
) -> Result<BTreeMap<String, MetricContract>> {
    evaluate_runtime_metric_defs_with_scope(
        metric_defs,
        base_rows,
        datasets,
        metric_ids,
        &RuntimeMetricEvalScope::default(),
    )
}

pub(crate) fn evaluate_runtime_metric_defs_with_scope(
    metric_defs: &BTreeMap<String, Value>,
    base_rows: &[Value],
    datasets: &BTreeMap<String, DatasetView>,
    metric_ids: Option<&[String]>,
    scope: &RuntimeMetricEvalScope,
) -> Result<BTreeMap<String, MetricContract>> {
    Ok(
        evaluate_runtime_metric_defs_with_scope_and_dag(metric_defs, base_rows, datasets, metric_ids, scope)?
            .0,
    )
}

pub(crate) fn evaluate_runtime_metric_defs_with_scope_and_dag(
    metric_defs: &BTreeMap<String, Value>,
    base_rows: &[Value],
    datasets: &BTreeMap<String, DatasetView>,
    metric_ids: Option<&[String]>,
    scope: &RuntimeMetricEvalScope,
) -> Result<(BTreeMap<String, MetricContract>, RequestDagMetrics)> {
    let expanded_defs = expand_runtime_metric_defs(metric_defs);
    if let Some(ids) = metric_ids {
        let selected = ids
            .iter()
            .filter_map(|id| {
                expanded_defs
                    .get(id)
                    .cloned()
                    .map(|value| (id.clone(), value))
            })
            .collect::<BTreeMap<_, _>>();
        return materialize_legacy_metric_map_with_scope_and_dag(
            &selected, base_rows, datasets, scope,
        );
    }
    materialize_legacy_metric_map_with_scope_and_dag(&expanded_defs, base_rows, datasets, scope)
}

#[cfg(test)]
mod resolve_key_tests {
    use std::collections::BTreeMap;

    use super::{
        imported_capsule_path_from_world_metrics_resource_id, resolve_runtime_metric_def_key,
    };
    use serde_json::json;

    #[test]
    fn imported_capsule_path_from_world_metrics_resource_id_parses_scoped_owner() {
        assert_eq!(
            imported_capsule_path_from_world_metrics_resource_id(
                "__world_metrics__::scenes/5_问题办理/问题办理.mei::metrics"
            ),
            Some("scenes/5_问题办理/问题办理.mei".to_string())
        );
        assert!(imported_capsule_path_from_world_metrics_resource_id("__world_metrics__").is_none());
    }

    #[test]
    fn resolve_runtime_metric_def_key_falls_back_to_namespaced_import_key() {
        let resource_id = "__world_metrics__::scenes/5_问题办理/问题办理.mei::metrics";
        let mut defs = BTreeMap::new();
        defs.insert(
            "scenes/5_问题办理/问题办理.mei::warnings_pending_table".to_string(),
            json!({"id": "scenes/5_问题办理/问题办理.mei::warnings_pending_table"}),
        );
        assert_eq!(
            resolve_runtime_metric_def_key(resource_id, "warnings_pending_table", &defs),
            Some("scenes/5_问题办理/问题办理.mei::warnings_pending_table".to_string())
        );
        assert_eq!(
            resolve_runtime_metric_def_key(resource_id, "missing", &defs),
            None
        );
    }
}
