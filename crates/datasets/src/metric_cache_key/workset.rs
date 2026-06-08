use std::collections::BTreeMap;

use mei_lang_kernel::{
    resolve_runtime_metric_def_key, runtime_analysis_closure_metric_ids, DatasetView,
};
use serde_json::Value;

#[derive(Debug, Clone, Default)]
pub(crate) struct RuntimeMetricWorkset {
    pub closure_metric_ids: Vec<String>,
    pub eval_metric_ids: Option<Vec<String>>,
    pub defs_for_hydrate: BTreeMap<String, Value>,
}

pub(crate) fn resolve_runtime_metric_ids(
    resource_id: &str,
    requested_metric_ids: &[String],
    defs: &BTreeMap<String, Value>,
) -> Vec<String> {
    requested_metric_ids
        .iter()
        .filter_map(|metric_id| resolve_runtime_metric_def_key(resource_id, metric_id, defs))
        .collect()
}

pub(crate) fn select_metric_defs(
    metric_defs: &BTreeMap<String, Value>,
    resolved_metric_ids: &[String],
) -> BTreeMap<String, Value> {
    if resolved_metric_ids.is_empty() {
        return metric_defs.clone();
    }
    resolved_metric_ids
        .iter()
        .filter_map(|metric_id| {
            metric_defs
                .get(metric_id)
                .cloned()
                .map(|value| (metric_id.clone(), value))
        })
        .collect()
}

pub(crate) fn runtime_metric_workset(
    resource_id: &str,
    requested_metric_ids: &[String],
    dataset: &DatasetView,
) -> RuntimeMetricWorkset {
    let resolved_metric_ids = resolve_runtime_metric_ids(
        resource_id,
        requested_metric_ids,
        &dataset.runtime_metric_defs,
    );
    if requested_metric_ids.is_empty() {
        return RuntimeMetricWorkset {
            closure_metric_ids: Vec::new(),
            eval_metric_ids: None,
            defs_for_hydrate: dataset.runtime_metric_defs.clone(),
        };
    }
    let closure_metric_ids =
        runtime_analysis_closure_metric_ids(&dataset.runtime_analysis_graph, &resolved_metric_ids);
    let eval_metric_ids = if closure_metric_ids.is_empty() {
        resolved_metric_ids.clone()
    } else {
        closure_metric_ids.clone()
    };
    RuntimeMetricWorkset {
        closure_metric_ids,
        defs_for_hydrate: select_metric_defs(&dataset.runtime_metric_defs, &eval_metric_ids),
        eval_metric_ids: Some(eval_metric_ids),
    }
}
