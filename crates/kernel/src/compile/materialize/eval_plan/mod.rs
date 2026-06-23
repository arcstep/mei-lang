mod executor;
mod fingerprint;
mod graph;
mod types;

#[cfg(test)]
mod tests;

pub use types::{
    EvalPlan, EvalPlanEdge, EvalPlanEdgeKind, EvalPlanNode, EvalPlanNodeKind, EvalPlanScope,
    RuntimeMetricEvalReport,
};

use crate::compile::analysis::eval_context::RuntimeMetricEvalScope;
use crate::model::DatasetView;
use serde_json::Value;
use std::collections::BTreeMap;

pub(crate) use executor::execute_eval_plan;
pub(crate) use graph::build_eval_plan;

pub fn build_runtime_eval_plan(
    metric_defs: &BTreeMap<String, Value>,
    metric_ids: Option<&[String]>,
    datasets: &BTreeMap<String, DatasetView>,
    scope: &RuntimeMetricEvalScope,
) -> EvalPlan {
    build_eval_plan(metric_defs, metric_ids, datasets, scope)
}
