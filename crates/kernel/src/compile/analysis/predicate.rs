use std::collections::BTreeMap;

use serde_json::Value;

use crate::model::DatasetView;

use super::{
    rowset::eval_rowset,
    schema::{row_string, row_value},
};

fn resolve_value_list(
    values_expr: Option<&Value>,
    datasets: &BTreeMap<String, DatasetView>,
) -> Vec<Value> {
    let Some(expr) = values_expr else {
        return Vec::new();
    };
    if let Some(items) = expr.as_array() {
        return items.clone();
    }
    let Some(map) = expr.as_object() else {
        return Vec::new();
    };
    if map.get("__kind").and_then(Value::as_str) != Some("analysis_expr") {
        return Vec::new();
    }
    let analysis_type = map.get("type").and_then(Value::as_str).unwrap_or("");
    if analysis_type != "text" {
        return Vec::new();
    }
    let source = map
        .get("source")
        .or_else(|| map.get("rowset"))
        .unwrap_or(&Value::Null);
    let field = map.get("field").and_then(Value::as_str).unwrap_or("");
    eval_rowset(source, datasets)
        .unwrap_or_default()
        .into_iter()
        .map(|row| Value::String(row_string(&row, field)))
        .collect()
}

pub(super) fn predicate_matches(
    row: &Value,
    predicate: &Value,
    datasets: &BTreeMap<String, DatasetView>,
) -> bool {
    let Some(object) = predicate.as_object() else {
        return true;
    };
    if object.get("__kind").and_then(Value::as_str) != Some("analysis_expr") {
        return true;
    }
    let analysis_type = object.get("type").and_then(Value::as_str).unwrap_or("");
    match analysis_type {
        "eq" => {
            let field = object.get("field").and_then(Value::as_str).unwrap_or("");
            let expected = object.get("value").cloned().unwrap_or(Value::Null);
            row_value(row, field).cloned().unwrap_or(Value::Null) == expected
        }
        "ne" => {
            let field = object.get("field").and_then(Value::as_str).unwrap_or("");
            let expected = object.get("value").cloned().unwrap_or(Value::Null);
            row_value(row, field).cloned().unwrap_or(Value::Null) != expected
        }
        "gt" | "gte" | "lt" | "lte" => {
            let field = object.get("field").and_then(Value::as_str).unwrap_or("");
            let expected = object
                .get("value")
                .and_then(super::schema::parse_number)
                .unwrap_or(f64::NAN);
            let actual = row_value(row, field)
                .and_then(super::schema::parse_number)
                .unwrap_or(f64::NAN);
            match analysis_type {
                "gt" => actual > expected,
                "gte" => actual >= expected,
                "lt" => actual < expected,
                _ => actual <= expected,
            }
        }
        "between" => {
            let field = object.get("field").and_then(Value::as_str).unwrap_or("");
            let lower = object
                .get("lower")
                .and_then(super::schema::parse_number)
                .unwrap_or(f64::MIN);
            let upper = object
                .get("upper")
                .and_then(super::schema::parse_number)
                .unwrap_or(f64::MAX);
            let actual = row_value(row, field)
                .and_then(super::schema::parse_number)
                .unwrap_or(f64::NAN);
            actual >= lower && actual <= upper
        }
        "in_values" => {
            let field = object.get("field").and_then(Value::as_str).unwrap_or("");
            let actual = row_string(row, field);
            resolve_value_list(object.get("values"), datasets)
                .iter()
                .filter_map(Value::as_str)
                .any(|item| item == actual)
        }
        "not_empty" => {
            let field = object.get("field").and_then(Value::as_str).unwrap_or("");
            !row_string(row, field).trim().is_empty()
        }
        "contains" => {
            let field = object.get("field").and_then(Value::as_str).unwrap_or("");
            let expected = object.get("value").and_then(Value::as_str).unwrap_or("");
            row_string(row, field).contains(expected)
        }
        "matches" => {
            let field = object.get("field").and_then(Value::as_str).unwrap_or("");
            let expected = object.get("pattern").and_then(Value::as_str).unwrap_or("");
            row_string(row, field).contains(expected)
        }
        "and" => object
            .get("predicates")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .all(|item| predicate_matches(row, item, datasets))
            })
            .unwrap_or(true),
        "or" => object
            .get("predicates")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .any(|item| predicate_matches(row, item, datasets))
            })
            .unwrap_or(true),
        "not" => !predicate_matches(
            row,
            object.get("predicate").unwrap_or(&Value::Null),
            datasets,
        ),
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn not_empty_treats_json_null_as_empty() {
        let row = json!({"园区名称": null});
        let predicate = json!({
            "__kind": "analysis_expr",
            "type": "not_empty",
            "field": "园区名称"
        });
        assert!(!predicate_matches(&row, &predicate, &BTreeMap::new()));
    }
}
