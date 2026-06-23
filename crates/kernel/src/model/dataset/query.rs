use std::collections::BTreeMap;

use serde::{de::Error, Deserialize, Deserializer, Serialize};

fn json_value_to_string(value: serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::Null => None,
        serde_json::Value::String(text) => {
            let trimmed = text.trim().to_string();
            (!trimmed.is_empty()).then_some(trimmed)
        }
        serde_json::Value::Number(number) => Some(number.to_string()),
        serde_json::Value::Bool(flag) => Some(flag.to_string()),
        serde_json::Value::Array(items) => {
            let values = items
                .into_iter()
                .filter_map(json_value_to_string)
                .collect::<Vec<_>>();
            (!values.is_empty()).then(|| values.join(","))
        }
        serde_json::Value::Object(object) => serde_json::to_string(&object).ok(),
    }
}

pub fn deserialize_string_map<'de, D>(deserializer: D) -> Result<BTreeMap<String, String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    let Some(serde_json::Value::Object(object)) = value else {
        return Ok(BTreeMap::new());
    };
    Ok(object
        .into_iter()
        .filter_map(|(key, value)| {
            let normalized_key = key.trim().to_string();
            if normalized_key.is_empty() {
                return None;
            }
            json_value_to_string(value).map(|value| (normalized_key, value))
        })
        .collect())
}

fn deserialize_string_default<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(value.and_then(json_value_to_string).unwrap_or_default())
}

fn deserialize_opt_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(value.and_then(json_value_to_string))
}

fn deserialize_string_vec<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    match value {
        Some(serde_json::Value::Array(items)) => Ok(items
            .into_iter()
            .filter_map(json_value_to_string)
            .collect::<Vec<_>>()),
        Some(value) => Ok(json_value_to_string(value).into_iter().collect()),
        None => Ok(Vec::new()),
    }
}

fn deserialize_filter_operator<'de, D>(deserializer: D) -> Result<FilterOperator, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    match value.and_then(json_value_to_string).as_deref() {
        None | Some("") | Some("eq") => Ok(FilterOperator::Eq),
        Some(other) => Err(Error::custom(format!(
            "unsupported filter operator `{other}`; expected `eq`"
        ))),
    }
}

fn deserialize_filter_source<'de, D>(deserializer: D) -> Result<FilterIntentSource, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    let source = value
        .and_then(json_value_to_string)
        .unwrap_or_else(|| "query_state".to_string())
        .trim()
        .to_ascii_lowercase()
        .replace([' ', '-'], "_");
    Ok(match source.as_str() {
        "" | "query_state" => FilterIntentSource::QueryState,
        "filter_bar" => FilterIntentSource::FilterBar,
        "metric_click" => FilterIntentSource::MetricClick,
        "chart_selection" => FilterIntentSource::ChartSelection,
        "table_selection" => FilterIntentSource::TableSelection,
        "drilldown" => FilterIntentSource::Drilldown,
        _ => FilterIntentSource::Unknown,
    })
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
    #[serde(default, deserialize_with = "deserialize_string_default")]
    pub dimension: String,
    #[serde(default, deserialize_with = "deserialize_filter_operator")]
    pub operator: FilterOperator,
    /// Normalized filter literal under the current host/runtime conventions.
    #[serde(default, deserialize_with = "deserialize_string_default")]
    pub value: String,
    #[serde(default, deserialize_with = "deserialize_filter_source")]
    pub source: FilterIntentSource,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct QueryState {
    /// Shared runtime query-state filters before lowering into eval scope.
    #[serde(default, deserialize_with = "deserialize_string_map")]
    pub filters: BTreeMap<String, String>,
    /// Shared free-text search carried alongside filters in host/runtime state.
    #[serde(
        default,
        deserialize_with = "deserialize_opt_string",
        skip_serializing_if = "Option::is_none"
    )]
    pub search: Option<String>,
    /// Semantic grouping dimensions selected by the host/runtime state.
    #[serde(
        default,
        deserialize_with = "deserialize_string_vec",
        skip_serializing_if = "Vec::is_empty"
    )]
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
    #[serde(
        default,
        deserialize_with = "deserialize_opt_string",
        skip_serializing_if = "Option::is_none"
    )]
    pub dimension: Option<String>,
    #[serde(
        default,
        deserialize_with = "deserialize_opt_string",
        skip_serializing_if = "Option::is_none"
    )]
    pub start: Option<String>,
    #[serde(
        default,
        deserialize_with = "deserialize_opt_string",
        skip_serializing_if = "Option::is_none"
    )]
    pub end: Option<String>,
    #[serde(
        default,
        deserialize_with = "deserialize_opt_string",
        skip_serializing_if = "Option::is_none"
    )]
    pub preset: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct DimensionBinding {
    /// Semantic dimension name consumed by filter/eval layers.
    pub dimension: String,
    /// Concrete dataset field selected for the current evaluation pass.
    pub field: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_state_deserializes_browser_scalar_filter_values() {
        let state: QueryState = serde_json::from_value(serde_json::json!({
            "filters": {
                "year": 2025,
                "enabled": true,
                "districts": ["沙坪坝区", 500106]
            },
            "search": 123,
            "group": "street",
            "time_range": {
                "dimension": "date",
                "start": 202401,
                "end": 202412
            }
        }))
        .expect("query state");

        assert_eq!(state.filters.get("year"), Some(&"2025".to_string()));
        assert_eq!(state.filters.get("enabled"), Some(&"true".to_string()));
        assert_eq!(
            state.filters.get("districts"),
            Some(&"沙坪坝区,500106".to_string())
        );
        assert_eq!(state.search.as_deref(), Some("123"));
        assert_eq!(state.group, vec!["street".to_string()]);
        assert_eq!(
            state
                .time_range
                .as_ref()
                .and_then(|range| range.start.as_deref()),
            Some("202401")
        );
    }

    #[test]
    fn filter_intent_deserializes_browser_scalar_value_and_unknown_source() {
        let intent: FilterIntent = serde_json::from_value(serde_json::json!({
            "dimension": "year",
            "operator": "eq",
            "value": 2025,
            "source": "custom-widget"
        }))
        .expect("filter intent");

        assert_eq!(intent.dimension, "year");
        assert_eq!(intent.operator, FilterOperator::Eq);
        assert_eq!(intent.value, "2025");
        assert_eq!(intent.source, FilterIntentSource::Unknown);
    }
}
