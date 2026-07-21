use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::schema::{ColumnSchema, DatasetSourceRef, MetricContract};
use crate::model::resource::SourceDecl;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetView {
    pub id: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub purpose: Option<String>,
    #[serde(default)]
    pub schema: Vec<ColumnSchema>,
    #[serde(default)]
    pub stage_schema: Vec<ColumnSchema>,
    pub columns: Vec<String>,
    pub rows: Vec<Value>,
    pub source: SourceDecl,
    #[serde(default)]
    pub sources: Vec<DatasetSourceRef>,
    /// Compile-time materialized metric snapshot.
    ///
    /// This field exists for preview/static fallback and for datasets that only
    /// expose already-materialized metrics. Runtime re-evaluation must prefer
    /// `runtime_metric_defs` when available.
    #[serde(default)]
    pub metrics: BTreeMap<String, MetricContract>,
    /// Runtime-authoritative metric definitions after explain-scope expansion.
    ///
    /// When non-empty, runtime metric APIs should re-evaluate from these defs
    /// under the current `RuntimeMetricEvalScope` instead of trusting the
    /// compile-time `metrics` snapshot above.
    #[serde(skip, default)]
    pub runtime_metric_defs: BTreeMap<String, Value>,
    /// Semantic analysis graph derived from runtime metric defs and explain
    /// scope. This is the current compile artifact closest to the language's
    /// semantic DAG, not a UI graph and not the request eval DAG.
    #[serde(skip, default)]
    pub runtime_analysis_graph: AnalysisGraph,
    /// Projection contracts derived from the semantic analysis graph for
    /// consumers such as drilldown/popup. This is not a semantic source of
    /// truth and should never drive runtime evaluation directly.
    #[serde(skip, default)]
    pub runtime_analysis_contracts: BTreeMap<String, Value>,
}

impl DatasetView {
    pub fn has_runtime_metric_defs(&self) -> bool {
        !self.runtime_metric_defs.is_empty()
    }

    pub fn uses_compiled_metric_snapshot_only(&self) -> bool {
        self.runtime_metric_defs.is_empty() && !self.metrics.is_empty()
    }

    /// Drop in-memory JSON row working set after Disk/runtime eval (pack-first).
    /// Does not touch schema/columns/source; packs and parquet remain on disk.
    pub fn release_row_working_set(&mut self) -> usize {
        let n = self.rows.len();
        if n == 0 {
            return 0;
        }
        self.rows.clear();
        self.rows.shrink_to_fit();
        n
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AnalysisGraph {
    /// Semantic nodes, typically metric-like analysis nodes plus lightweight
    /// narrative support nodes. This graph is compile-derived semantic
    /// structure, not the request-time evaluation DAG.
    #[serde(default)]
    pub nodes: BTreeMap<String, AnalysisNode>,
    /// Semantic edges between analysis nodes. Current roles include support
    /// roles such as `detail`/`trend` and scoped child expansion links. They
    /// should not be confused with execution dependencies.
    #[serde(default)]
    pub edges: Vec<AnalysisEdge>,
}

impl AnalysisGraph {
    pub fn validate_invariants(&self) -> Vec<String> {
        let mut errors = Vec::new();
        for (node_id, node) in &self.nodes {
            if node.id.trim().is_empty() {
                errors.push("analysis node id must not be blank".to_string());
            }
            if node.id != *node_id {
                errors.push(format!(
                    "analysis node key/id mismatch: key=`{node_id}` id=`{}`",
                    node.id
                ));
            }
            match node.semantic_kind() {
                SemanticNodeKind::Metric => {
                    if node
                        .canonical_metric_id
                        .as_deref()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .is_none()
                    {
                        errors.push(format!(
                            "metric analysis node `{node_id}` must have canonical_metric_id"
                        ));
                    }
                }
                SemanticNodeKind::NarrativeSupport => {
                    if node.can_explain {
                        errors.push(format!(
                            "narrative support node `{node_id}` must not carry explain scope"
                        ));
                    }
                }
                SemanticNodeKind::TabularSource => {
                    if node
                        .tabular_source_dataset_id
                        .as_deref()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .is_none()
                    {
                        errors.push(format!(
                            "tabular source node `{node_id}` must carry tabular_source_dataset_id"
                        ));
                    }
                    if node.can_explain {
                        errors.push(format!(
                            "tabular source node `{node_id}` must not carry explain scope"
                        ));
                    }
                }
                SemanticNodeKind::Unknown => {}
            }
        }
        for edge in &self.edges {
            let Some(target) = self.nodes.get(&edge.to) else {
                errors.push(format!(
                    "analysis edge `{}` -> `{}` points to missing node",
                    edge.from, edge.to
                ));
                continue;
            };
            if !self.nodes.contains_key(&edge.from) {
                errors.push(format!(
                    "analysis edge `{}` -> `{}` starts from missing node",
                    edge.from, edge.to
                ));
            }
            if edge.semantic_kind() == SemanticEdgeKind::ScopeMetric
                && !target.participates_in_metric_closure()
            {
                errors.push(format!(
                    "scope_metric edge `{}` -> `{}` must target a metric node",
                    edge.from, edge.to
                ));
            }
            if edge.semantic_kind() == SemanticEdgeKind::Lineage
                && target.semantic_kind() != SemanticNodeKind::TabularSource
            {
                errors.push(format!(
                    "lineage edge `{}` -> `{}` must target a tabular source node",
                    edge.from, edge.to
                ));
            }
        }
        errors
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AnalysisNode {
    pub id: String,
    #[serde(default)]
    pub canonical_metric_id: Option<String>,
    #[serde(default)]
    pub parent_id: Option<String>,
    #[serde(default)]
    pub node_kind: String,
    #[serde(default)]
    pub semantic_kind: SemanticNodeKind,
    #[serde(default)]
    pub support_role: Option<String>,
    #[serde(default)]
    pub shape: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub lineage_dataset_id: Option<String>,
    #[serde(default)]
    pub tabular_source_dataset_id: Option<String>,
    #[serde(default)]
    pub can_explain: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SemanticNodeKind {
    #[default]
    Unknown,
    Metric,
    NarrativeSupport,
    TabularSource,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AnalysisEdge {
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub semantic_kind: SemanticEdgeKind,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SemanticEdgeKind {
    #[default]
    Unknown,
    ScopeMetric,
    Support,
    Lineage,
    Association,
    Reuse,
}

impl AnalysisNode {
    pub fn semantic_kind(&self) -> SemanticNodeKind {
        if self.semantic_kind != SemanticNodeKind::Unknown {
            return self.semantic_kind;
        }
        match self.node_kind.trim() {
            "metric" => SemanticNodeKind::Metric,
            "narrative" => SemanticNodeKind::NarrativeSupport,
            "tabular_source" => SemanticNodeKind::TabularSource,
            _ => SemanticNodeKind::Unknown,
        }
    }

    pub fn participates_in_metric_closure(&self) -> bool {
        matches!(self.semantic_kind(), SemanticNodeKind::Metric)
    }

    pub fn is_focusable(&self) -> bool {
        matches!(self.semantic_kind(), SemanticNodeKind::Metric)
    }
}

impl AnalysisEdge {
    pub fn semantic_kind(&self) -> SemanticEdgeKind {
        if self.semantic_kind != SemanticEdgeKind::Unknown {
            return self.semantic_kind;
        }
        match self.role.trim() {
            "scope_metric" => SemanticEdgeKind::ScopeMetric,
            "support"
            | "definition"
            | "detail"
            | "trend"
            | "composition"
            | "numerator_denominator"
            | "attribution"
            | "note" => SemanticEdgeKind::Support,
            "lineage" => SemanticEdgeKind::Lineage,
            "association" => SemanticEdgeKind::Association,
            "reuse" => SemanticEdgeKind::Reuse,
            _ => SemanticEdgeKind::Unknown,
        }
    }

    pub fn participates_in_default_closure(&self) -> bool {
        matches!(
            self.semantic_kind(),
            SemanticEdgeKind::ScopeMetric | SemanticEdgeKind::Support
        )
    }
}
