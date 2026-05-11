use serde_json::Value;

use super::schema::{parse_number, row_string, row_value};

pub(super) fn predicate_matches(row: &Value, predicate: &Value) -> bool {
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
                .and_then(parse_number)
                .unwrap_or(f64::NAN);
            let actual = row_value(row, field).and_then(parse_number).unwrap_or(f64::NAN);
            match analysis_type {
                "gt" => actual > expected,
                "gte" => actual >= expected,
                "lt" => actual < expected,
                _ => actual <= expected,
            }
        }
        "between" => {
            let field = object.get("field").and_then(Value::as_str).unwrap_or("");
            let lower = object.get("lower").and_then(parse_number).unwrap_or(f64::MIN);
            let upper = object.get("upper").and_then(parse_number).unwrap_or(f64::MAX);
            let actual = row_value(row, field).and_then(parse_number).unwrap_or(f64::NAN);
            actual >= lower && actual <= upper
        }
        "in_values" => {
            let field = object.get("field").and_then(Value::as_str).unwrap_or("");
            let actual = row_value(row, field).cloned().unwrap_or(Value::Null);
            object
                .get("values")
                .and_then(Value::as_array)
                .map(|items| items.iter().any(|item| item == &actual))
                .unwrap_or(false)
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
            .map(|items| items.iter().all(|item| predicate_matches(row, item)))
            .unwrap_or(true),
        "or" => object
            .get("predicates")
            .and_then(Value::as_array)
            .map(|items| items.iter().any(|item| predicate_matches(row, item)))
            .unwrap_or(true),
        "not" => !predicate_matches(row, object.get("predicate").unwrap_or(&Value::Null)),
        _ => true,
    }
}
