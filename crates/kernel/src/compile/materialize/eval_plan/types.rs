use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::compile::analysis::eval_context::{RequestDagMetrics, RuntimeMetricEvalScope};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EvalPlan {
    #[serde(default)]
    pub scope: EvalPlanScope,
    #[serde(default)]
    pub targets: Vec<String>,
    #[serde(default)]
    pub nodes: BTreeMap<String, EvalPlanNode>,
    #[serde(default)]
    pub edges: Vec<EvalPlanEdge>,
}

impl EvalPlan {
    pub fn node_count_by_kind(&self, kind: EvalPlanNodeKind) -> usize {
        self.nodes.values().filter(|node| node.kind == kind).count()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EvalPlanScope {
    pub base_dataset_id: String,
    pub scene_id: String,
    pub target: String,
    pub search: String,
    pub filters_fingerprint: String,
    pub group_identity_key: String,
    pub time_range_identity_key: String,
    pub dependency_revision_key: String,
}

impl From<&RuntimeMetricEvalScope> for EvalPlanScope {
    fn from(scope: &RuntimeMetricEvalScope) -> Self {
        Self {
            base_dataset_id: scope.base_dataset_id.clone(),
            scene_id: scope.scene_id.clone(),
            target: scope.target.clone(),
            search: scope.search.clone(),
            filters_fingerprint: scope.filters_fingerprint.clone(),
            group_identity_key: scope.query_state.group_identity_key(),
            time_range_identity_key: scope.query_state.time_range_identity_key(),
            dependency_revision_key: scope.dependency_revision_key.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum EvalPlanNodeKind {
    #[default]
    Unknown,
    MetricEval,
    Rowset,
    ScalarExpr,
    Hydrate,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EvalPlanNode {
    pub id: String,
    #[serde(default)]
    pub kind: EvalPlanNodeKind,
    #[serde(default)]
    pub metric_id: Option<String>,
    #[serde(default)]
    pub dataset_id: Option<String>,
    #[serde(default)]
    pub expr_fingerprint: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Default)]
#[serde(rename_all = "snake_case")]
pub enum EvalPlanEdgeKind {
    #[default]
    DependsOn,
    Hydrates,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EvalPlanEdge {
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub kind: EvalPlanEdgeKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RuntimeMetricEvalReport {
    #[serde(default)]
    pub eval_plan: EvalPlan,
    #[serde(default)]
    pub request_dag_metrics: RequestDagMetrics,
}
