use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::resource::SourceDecl;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnSchema {
    pub name: String,
    #[serde(rename = "type")]
    pub type_name: String,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub optional: bool,
    #[serde(default)]
    pub unit: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MetricShape {
    Scalar,
    Series,
    Table,
    Dataframe,
}

fn default_metric_shape() -> MetricShape {
    MetricShape::Dataframe
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricContract {
    pub id: String,
    #[serde(default)]
    pub label: Option<String>,
    /// `ds.scalar_map(..., unit = "...")` 等声明的展示单位，供指标卡等 UI 与数值分列展示。
    #[serde(default)]
    pub unit: Option<String>,
    #[serde(default)]
    pub purpose: Option<String>,
    #[serde(default = "default_metric_shape")]
    pub shape: MetricShape,
    #[serde(default)]
    pub schema: Vec<ColumnSchema>,
    #[serde(default)]
    pub dataset: Option<String>,
    #[serde(default)]
    pub transforms: Vec<DataTransform>,
    #[serde(default)]
    pub value: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetSourceRef {
    pub id: String,
    #[serde(default)]
    pub alias: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataTransform {
    #[serde(rename = "type")]
    pub transform_type: String,
    #[serde(default)]
    pub config: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataRef {
    pub id: String,
    #[serde(default)]
    pub from_dataset: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum FilterOperator {
    #[default]
    Eq,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum FilterIntentSource {
    #[default]
    QueryState,
    FilterBar,
    MetricClick,
    ChartSelection,
    TableSelection,
    Drilldown,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct FilterIntent {
    /// Semantic dimension requested by runtime interaction/query state.
    pub dimension: String,
    #[serde(default)]
    pub operator: FilterOperator,
    /// Normalized filter literal under the current host/runtime conventions.
    pub value: String,
    #[serde(default)]
    pub source: FilterIntentSource,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct QueryState {
    /// Shared runtime query-state filters before lowering into eval scope.
    #[serde(default)]
    pub filters: BTreeMap<String, String>,
    /// Shared free-text search carried alongside filters in host/runtime state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search: Option<String>,
    /// Semantic grouping dimensions selected by the host/runtime state.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub group: Vec<String>,
    /// Optional shared time window carried by the host/runtime state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_range: Option<QueryTimeRange>,
}

impl QueryState {
    pub fn group_identity_key(&self) -> String {
        serde_json::to_string(&self.group).unwrap_or_default()
    }

    pub fn time_range_identity_key(&self) -> String {
        serde_json::to_string(&self.time_range).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct QueryTimeRange {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dimension: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct DimensionBinding {
    /// Semantic dimension name consumed by filter/eval layers.
    pub dimension: String,
    /// Concrete dataset field selected for the current evaluation pass.
    pub field: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricRef {
    pub id: String,
    #[serde(default)]
    pub from_dataset: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldMetricLedgerEntry {
    pub id: String,
    pub owner_resource_id: String,
    pub order: usize,
    pub metric: MetricContract,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricPackContract {
    pub id: String,
    #[serde(default)]
    pub purpose: Option<String>,
    #[serde(default)]
    pub metrics: BTreeMap<String, MetricContract>,
}

#[cfg(test)]
mod tests {
    use super::{
        AnalysisEdge, AnalysisGraph, AnalysisNode, DatasetView, SemanticEdgeKind, SemanticNodeKind,
        SourceDecl,
    };
    use std::collections::BTreeMap;

    fn empty_dataset() -> DatasetView {
        DatasetView {
            id: "sample".to_string(),
            title: None,
            purpose: None,
            schema: Vec::new(),
            stage_schema: Vec::new(),
            columns: Vec::new(),
            rows: Vec::new(),
            source: SourceDecl {
                kind: "derived".to_string(),
                path: "legacy.metric_pack:sample".to_string(),
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
            metrics: BTreeMap::new(),
            runtime_metric_defs: BTreeMap::new(),
            runtime_analysis_graph: Default::default(),
            runtime_analysis_contracts: Default::default(),
        }
    }

    #[test]
    fn dataset_view_runtime_metric_helpers_encode_truth_layers() {
        let mut dataset = empty_dataset();
        assert!(!dataset.has_runtime_metric_defs());
        assert!(!dataset.uses_compiled_metric_snapshot_only());

        dataset.metrics.insert(
            "sales_total".to_string(),
            serde_json::from_value(serde_json::json!({
                "id": "sales_total",
                "shape": "scalar",
                "value": {"value": 1}
            }))
            .expect("metric snapshot"),
        );
        assert!(!dataset.has_runtime_metric_defs());
        assert!(dataset.uses_compiled_metric_snapshot_only());

        dataset.runtime_metric_defs.insert(
            "sales_total".to_string(),
            serde_json::json!({"id": "sales_total", "shape": "scalar_map", "values": {"value": 1}}),
        );
        assert!(dataset.has_runtime_metric_defs());
        assert!(
            !dataset.uses_compiled_metric_snapshot_only(),
            "runtime defs should become the runtime-authoritative source when present"
        );
    }

    #[test]
    fn analysis_node_and_edge_encode_semantic_categories() {
        let metric = AnalysisNode {
            node_kind: "metric".to_string(),
            semantic_kind: SemanticNodeKind::Metric,
            ..Default::default()
        };
        let narrative = AnalysisNode {
            node_kind: "narrative".to_string(),
            semantic_kind: SemanticNodeKind::NarrativeSupport,
            ..Default::default()
        };
        let tabular = AnalysisNode {
            node_kind: "tabular_source".to_string(),
            semantic_kind: SemanticNodeKind::TabularSource,
            tabular_source_dataset_id: Some("warning_list".to_string()),
            ..Default::default()
        };
        let support = AnalysisEdge {
            role: "detail".to_string(),
            semantic_kind: SemanticEdgeKind::Support,
            ..Default::default()
        };
        let scope_metric = AnalysisEdge {
            role: "scope_metric".to_string(),
            semantic_kind: SemanticEdgeKind::ScopeMetric,
            ..Default::default()
        };
        let association = AnalysisEdge {
            role: "association".to_string(),
            semantic_kind: SemanticEdgeKind::Association,
            ..Default::default()
        };

        assert_eq!(metric.semantic_kind(), SemanticNodeKind::Metric);
        assert!(metric.participates_in_metric_closure());
        assert_eq!(narrative.semantic_kind(), SemanticNodeKind::NarrativeSupport);
        assert!(!narrative.participates_in_metric_closure());
        assert_eq!(tabular.semantic_kind(), SemanticNodeKind::TabularSource);
        assert!(!tabular.participates_in_metric_closure());

        assert_eq!(support.semantic_kind(), SemanticEdgeKind::Support);
        assert!(support.participates_in_default_closure());
        assert_eq!(scope_metric.semantic_kind(), SemanticEdgeKind::ScopeMetric);
        assert!(scope_metric.participates_in_default_closure());
        assert_eq!(association.semantic_kind(), SemanticEdgeKind::Association);
        assert!(!association.participates_in_default_closure());
    }

    #[test]
    fn analysis_graph_validator_rejects_non_tabular_lineage_targets() {
        let graph = AnalysisGraph {
            nodes: BTreeMap::from([
                (
                    "sales_total".to_string(),
                    AnalysisNode {
                        id: "sales_total".to_string(),
                        canonical_metric_id: Some("sales_total".to_string()),
                        node_kind: "metric".to_string(),
                        semantic_kind: SemanticNodeKind::Metric,
                        ..Default::default()
                    },
                ),
                (
                    "detail_note".to_string(),
                    AnalysisNode {
                        id: "detail_note".to_string(),
                        node_kind: "narrative".to_string(),
                        semantic_kind: SemanticNodeKind::NarrativeSupport,
                        ..Default::default()
                    },
                ),
            ]),
            edges: vec![AnalysisEdge {
                from: "sales_total".to_string(),
                to: "detail_note".to_string(),
                role: "lineage".to_string(),
                semantic_kind: SemanticEdgeKind::Lineage,
            }],
        };
        let errors = graph.validate_invariants();
        assert!(errors.iter().any(|item| item.contains("must target a tabular source node")));
    }
}
