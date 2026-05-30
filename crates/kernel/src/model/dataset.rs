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
    #[serde(default)]
    pub metrics: BTreeMap<String, MetricContract>,
    #[serde(skip, default)]
    pub runtime_metric_defs: BTreeMap<String, Value>,
    #[serde(skip, default)]
    pub runtime_analysis_graph: AnalysisGraph,
    #[serde(skip, default)]
    pub runtime_analysis_contracts: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AnalysisGraph {
    #[serde(default)]
    pub nodes: BTreeMap<String, AnalysisNode>,
    #[serde(default)]
    pub edges: Vec<AnalysisEdge>,
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
    pub support_role: Option<String>,
    #[serde(default)]
    pub shape: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub lineage_dataset_id: Option<String>,
    #[serde(default)]
    pub can_explain: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AnalysisEdge {
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub role: String,
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
