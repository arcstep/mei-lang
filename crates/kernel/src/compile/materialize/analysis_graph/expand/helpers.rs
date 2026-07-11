use serde_json::Value;

pub(crate) const INFERRED_SCALAR_ROWSET_LOCAL_ID: &str = "__scalar_rowset__";

pub(crate) fn support_role_for_item(map: &serde_json::Map<String, Value>) -> String {
    let raw = map
        .get("kind")
        .or_else(|| map.get("type"))
        .or_else(|| map.get("id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default();
    normalize_role_id(raw)
}

pub(crate) fn child_metric_local_id(map: &serde_json::Map<String, Value>) -> Option<String> {
    map.get("key")
        .or_else(|| map.get("id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(crate) fn scoped_child_metric_id(parent_metric_id: &str, local_id: &str) -> String {
    format!("{}::{}", parent_metric_id.trim(), local_id.trim())
}

pub(super) fn normalize_role_id(value: &str) -> String {
    let raw = value.trim().to_lowercase();
    match raw.as_str() {
        "definition" | "metric_definition" | "metric-definition" => "definition".to_string(),
        "detail" | "details" => "detail".to_string(),
        "trend" | "trend_compare" | "timeseries" | "time_series" | "time-series" => {
            "trend".to_string()
        }
        "composition" | "breakdown" | "group" | "group_by" | "groupby" => "composition".to_string(),
        "numerator_denominator" | "numerator-denominator" | "ratio" | "numerator" => {
            "numerator_denominator".to_string()
        }
        "note" | "text" | "md" | "markdown" => "note".to_string(),
        _ => raw.replace([' ', '-'], "_"),
    }
}
