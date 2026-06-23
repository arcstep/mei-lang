use std::collections::BTreeMap;

use anyhow::Result;
use serde_json::Value;

use crate::compile::analysis::eval_context::{RequestDagMetrics, RuntimeMetricEvalScope};
use crate::model::{DatasetView, MetricContract};

use super::super::metric_packs::materialize_legacy_metric_map_with_scope_and_dag;
use super::build_eval_plan;
use super::EvalPlan;
use super::EvalPlanNodeKind;

pub(crate) fn execute_eval_plan(
    metric_defs: &BTreeMap<String, Value>,
    base_rows: &[Value],
    datasets: &BTreeMap<String, DatasetView>,
    scope: &RuntimeMetricEvalScope,
    plan: &EvalPlan,
    cached_metrics: &BTreeMap<String, MetricContract>,
) -> Result<(BTreeMap<String, MetricContract>, RequestDagMetrics)> {
    let expanded_defs = metric_defs.clone();
    let mut metrics = cached_metrics.clone();
    let planned_metric_ids = if !plan.targets.is_empty() {
        plan.targets.clone()
    } else {
        build_eval_plan(metric_defs, None, datasets, scope).targets
    };
    let missing_metric_ids = planned_metric_ids
        .iter()
        .filter(|metric_id| {
            plan.nodes
                .get(&format!("metric:{metric_id}"))
                .is_some_and(|node| node.kind == EvalPlanNodeKind::MetricEval)
                && !metrics.contains_key(*metric_id)
        })
        .cloned()
        .collect::<Vec<_>>();
    if missing_metric_ids.is_empty() {
        return Ok((metrics, RequestDagMetrics::default()));
    }
    let selected_defs = missing_metric_ids
        .iter()
        .filter_map(|metric_id| {
            expanded_defs
                .get(metric_id)
                .cloned()
                .map(|value| (metric_id.clone(), value))
        })
        .collect::<BTreeMap<_, _>>();
    let (computed, dag_metrics) = materialize_legacy_metric_map_with_scope_and_dag(
        &selected_defs,
        base_rows,
        datasets,
        scope,
        Some(&expanded_defs),
    )?;
    metrics.extend(computed);
    Ok((metrics, dag_metrics))
}
