use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};

use mei_lang_kernel::{
    runtime_eval_node_cache_enabled, EvalPlanNodeKind, FilterIntent, MetricContract, QueryState,
    RuntimeMetricEvalReport, RuntimeMetricEvalScope,
};
use serde::{Deserialize, Serialize};

fn hash_fingerprint(value: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

pub(super) fn metric_eval_diagnostic_code(message: &str) -> &'static str {
    if message.contains("cyclic_eval_dependency")
        || message.contains("metric_eval_recursion_guard_tripped")
    {
        "metric_eval_recursion_guard_tripped"
    } else {
        "metric_eval_failed"
    }
}

pub(super) fn hash_metric_response_cache_key(key: &str) -> u64 {
    hash_fingerprint(key)
}

pub(super) fn write_dag_perf(
    perf: &mut BTreeMap<String, u64>,
    eval_report: &RuntimeMetricEvalReport,
    eval_scope: &RuntimeMetricEvalScope,
    closure_metric_ids: &[String],
    dataset: &mei_lang_kernel::DatasetView,
) {
    let dag_metrics = &eval_report.request_dag_metrics;
    let eval_plan = &eval_report.eval_plan;
    let eval_scope_key = mei_lang_datasets::eval_node_cache_key("metric_scope", eval_scope);
    perf.insert(
        "eval_plan_targets".to_string(),
        eval_plan.targets.len() as u64,
    );
    perf.insert("eval_plan_nodes".to_string(), eval_plan.nodes.len() as u64);
    perf.insert("eval_plan_edges".to_string(), eval_plan.edges.len() as u64);
    perf.insert(
        "eval_plan_metric_nodes".to_string(),
        eval_plan.node_count_by_kind(EvalPlanNodeKind::MetricEval) as u64,
    );
    perf.insert(
        "eval_plan_rowset_nodes".to_string(),
        eval_plan.node_count_by_kind(EvalPlanNodeKind::Rowset) as u64,
    );
    perf.insert(
        "eval_plan_scalar_nodes".to_string(),
        eval_plan.node_count_by_kind(EvalPlanNodeKind::ScalarExpr) as u64,
    );
    perf.insert(
        "eval_plan_hydrate_nodes".to_string(),
        eval_plan.node_count_by_kind(EvalPlanNodeKind::Hydrate) as u64,
    );
    perf.insert(
        "eval_scope_key_hash".to_string(),
        hash_fingerprint(&eval_scope_key),
    );
    perf.insert(
        "eval_scope_group_key_hash".to_string(),
        hash_fingerprint(&eval_scope.query_state.group_identity_key()),
    );
    perf.insert(
        "eval_scope_time_range_key_hash".to_string(),
        hash_fingerprint(&eval_scope.query_state.time_range_identity_key()),
    );
    perf.insert(
        "eval_scope_group_dimensions".to_string(),
        eval_scope.query_state.group.len() as u64,
    );
    perf.insert("request_dag_nodes".to_string(), dag_metrics.nodes as u64);
    perf.insert("request_dag_edges".to_string(), dag_metrics.edges as u64);
    perf.insert("request_dag_hits".to_string(), dag_metrics.hits);
    perf.insert("request_dag_misses".to_string(), dag_metrics.misses);
    perf.insert(
        "request_dag_request_cache_hits".to_string(),
        dag_metrics.request_cache_hits,
    );
    perf.insert(
        "request_dag_eval_node_cache_hits".to_string(),
        dag_metrics.eval_node_cache_hits,
    );
    perf.insert(
        "request_dag_eval_node_cache_misses".to_string(),
        dag_metrics.eval_node_cache_misses,
    );
    if !closure_metric_ids.is_empty() {
        perf.insert(
            "analysis_closure_nodes".to_string(),
            closure_metric_ids.len() as u64,
        );
        let closure_set = closure_metric_ids.iter().cloned().collect::<BTreeSet<_>>();
        let closure_edges = dataset
            .runtime_analysis_graph
            .edges
            .iter()
            .filter(|edge| closure_set.contains(&edge.from) && closure_set.contains(&edge.to))
            .count() as u64;
        perf.insert("analysis_closure_edges".to_string(), closure_edges);
    }
    perf.insert(
        "eval_node_cache_enabled".to_string(),
        u64::from(runtime_eval_node_cache_enabled()),
    );
}

#[derive(Debug, Deserialize)]
pub struct MetricQueryRequest {
    #[serde(default)]
    pub scene_id: Option<String>,
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub dataset_id: String,
    #[serde(default)]
    pub metric_ids: Vec<String>,
    #[serde(default)]
    pub metric_groups: Vec<MetricQueryGroupRequest>,
    #[serde(default)]
    pub search: Option<String>,
    #[serde(default)]
    pub filters: BTreeMap<String, String>,
    #[serde(default)]
    pub query_state: Option<QueryState>,
    #[serde(default)]
    pub filter_intents: Vec<FilterIntent>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MetricQueryGroupRequest {
    pub dataset_id: String,
    #[serde(default)]
    pub metric_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MetricQueryGroupResponse {
    pub dataset_id: String,
    pub total_rows: usize,
    pub metrics: Vec<MetricContract>,
    pub perf: BTreeMap<String, u64>,
}

#[derive(Debug, Serialize)]
pub struct MetricQueryResponse {
    pub scene_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scene_path: Option<String>,
    pub dataset_id: String,
    pub total_rows: usize,
    pub metrics: Vec<MetricContract>,
    pub perf: BTreeMap<String, u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<MetricQueryGroupResponse>,
}
