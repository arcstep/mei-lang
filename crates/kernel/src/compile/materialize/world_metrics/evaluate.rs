use std::collections::BTreeMap;

use anyhow::Result;
use serde_json::Value;

use crate::model::{DatasetView, LoadedResource, MetricContract};

use super::super::analysis_graph::expand_runtime_metric_defs;
use super::super::eval_plan::{
    build_eval_plan, execute_eval_plan, EvalPlan, RuntimeMetricEvalReport,
};
use super::super::metric_packs::{
    materialize_legacy_metric_map, materialize_legacy_metric_map_with_scope_and_dag,
};
use crate::compile::analysis::eval_context::RuntimeMetricEvalScope;

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
    Ok(evaluate_runtime_metric_defs_with_scope_and_dag(
        metric_defs,
        base_rows,
        datasets,
        metric_ids,
        scope,
    )?
    .0)
}

pub(crate) fn evaluate_runtime_metric_defs_with_scope_and_dag(
    metric_defs: &BTreeMap<String, Value>,
    base_rows: &[Value],
    datasets: &BTreeMap<String, DatasetView>,
    metric_ids: Option<&[String]>,
    scope: &RuntimeMetricEvalScope,
) -> Result<(BTreeMap<String, MetricContract>, RuntimeMetricEvalReport)> {
    // Runtime evaluation always treats metric defs as the authoritative source
    // of truth. Compile-time `DatasetView.metrics` snapshots are only used by
    // higher layers when no runtime defs exist at all.
    let expanded_defs = expand_runtime_metric_defs(metric_defs);
    let selected_defs = if let Some(ids) = metric_ids {
        ids.iter()
            .filter_map(|id| {
                expanded_defs
                    .get(id)
                    .cloned()
                    .map(|value| (id.clone(), value))
            })
            .collect::<BTreeMap<_, _>>()
    } else {
        expanded_defs.clone()
    };
    let eval_plan = build_eval_plan(&expanded_defs, metric_ids, datasets, scope);
    let (metrics, request_dag_metrics) = materialize_legacy_metric_map_with_scope_and_dag(
        &selected_defs,
        base_rows,
        datasets,
        scope,
        Some(&expanded_defs),
    )?;
    Ok((
        metrics,
        RuntimeMetricEvalReport {
            eval_plan,
            request_dag_metrics,
        },
    ))
}

pub fn evaluate_runtime_metric_defs_with_plan_and_dag(
    metric_defs: &BTreeMap<String, Value>,
    base_rows: &[Value],
    datasets: &BTreeMap<String, DatasetView>,
    scope: &RuntimeMetricEvalScope,
    plan: &EvalPlan,
    cached_metrics: &BTreeMap<String, MetricContract>,
) -> Result<(BTreeMap<String, MetricContract>, RuntimeMetricEvalReport)> {
    let (metrics, request_dag_metrics) = execute_eval_plan(
        metric_defs,
        base_rows,
        datasets,
        scope,
        plan,
        cached_metrics,
    )?;
    Ok((
        metrics,
        RuntimeMetricEvalReport {
            eval_plan: plan.clone(),
            request_dag_metrics,
        },
    ))
}
