use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

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
