use std::collections::BTreeMap;

use regex::Regex;
use serde_json::Value;

use crate::model::DatasetView;

use super::{
    eval_context::EvalContext,
    rowset::eval_rowset_with_ctx,
    schema::{row_string, row_value},
};

fn resolve_value_list(
    values_expr: Option<&Value>,
    datasets: &BTreeMap<String, DatasetView>,
    ctx: &mut EvalContext,
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
    eval_rowset_with_ctx(source, datasets, ctx)
        .unwrap_or_default()
        .into_iter()
        .map(|row| Value::String(row_string(&row, field)))
        .collect()
}

fn field_text(row: &Value, field: &str) -> String {
    row_string(row, field).trim().to_string()
}

/// Aligns with Neverland `DrillFieldValue.blank?/present?` for承办部门、办结时间等字段。
fn blank_field(text: &str) -> bool {
    if text.is_empty() {
        return true;
    }
    const SENTINELS: &[&str] = &[
        "—",
        "-",
        "/",
        "无",
        "暂无",
        "待定",
        "未知",
        "n/a",
        "na",
        "null",
        "none",
        "无承办部门",
        "无部门",
    ];
    SENTINELS
        .iter()
        .any(|sentinel| sentinel.eq_ignore_ascii_case(text))
}

/// Dash-only placeholders such as `——` on 职务.
fn placeholder_only_text(text: &str) -> bool {
    if text.is_empty() {
        return false;
    }
    text.chars().all(|ch| matches!(ch, '-' | '—' | '－' | '―' | ' ' | '\t' | '\n' | '\r'))
}

#[cfg(test)]
pub(super) fn predicate_matches(
    row: &Value,
    predicate: &Value,
    datasets: &BTreeMap<String, DatasetView>,
) -> bool {
    let mut ctx = EvalContext::default();
    predicate_matches_with_ctx(row, predicate, datasets, &mut ctx)
}

pub(super) fn predicate_matches_with_ctx(
    row: &Value,
    predicate: &Value,
    datasets: &BTreeMap<String, DatasetView>,
    ctx: &mut EvalContext,
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
            resolve_value_list(object.get("values"), datasets, ctx)
                .iter()
                .filter_map(Value::as_str)
                .any(|item| item == actual)
        }
        "not_empty" => {
            let field = object.get("field").and_then(Value::as_str).unwrap_or("");
            !row_string(row, field).trim().is_empty()
        }
        "present" => {
            let field = object.get("field").and_then(Value::as_str).unwrap_or("");
            !blank_field(&field_text(row, field))
        }
        "blank" => {
            let field = object.get("field").and_then(Value::as_str).unwrap_or("");
            blank_field(&field_text(row, field))
        }
        "placeholder_only" => {
            let field = object.get("field").and_then(Value::as_str).unwrap_or("");
            placeholder_only_text(&field_text(row, field))
        }
        "contains" => {
            let field = object.get("field").and_then(Value::as_str).unwrap_or("");
            let expected = object.get("value").and_then(Value::as_str).unwrap_or("");
            row_string(row, field).contains(expected)
        }
        "matches" => {
            let field = object.get("field").and_then(Value::as_str).unwrap_or("");
            let expected = object.get("pattern").and_then(Value::as_str).unwrap_or("");
            Regex::new(expected)
                .map(|regex| regex.is_match(&row_string(row, field)))
                .unwrap_or(false)
        }
        "and" => object
            .get("predicates")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .all(|item| predicate_matches_with_ctx(row, item, datasets, ctx))
            })
            .unwrap_or(true),
        "or" => object
            .get("predicates")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .any(|item| predicate_matches_with_ctx(row, item, datasets, ctx))
            })
            .unwrap_or(true),
        "not" => !predicate_matches_with_ctx(
            row,
            object.get("predicate").unwrap_or(&Value::Null),
            datasets,
            ctx,
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

    #[test]
    fn present_treats_dash_sentinel_as_blank() {
        let row = json!({"承办部门": "—"});
        let predicate = json!({
            "__kind": "analysis_expr",
            "type": "present",
            "field": "承办部门"
        });
        assert!(!predicate_matches(&row, &predicate, &BTreeMap::new()));
    }

    #[test]
    fn placeholder_only_matches_dash_only_position() {
        let row = json!({"职务": "——"});
        let predicate = json!({
            "__kind": "analysis_expr",
            "type": "placeholder_only",
            "field": "职务"
        });
        assert!(predicate_matches(&row, &predicate, &BTreeMap::new()));
    }

    #[test]
    fn matches_uses_regex_pattern() {
        let row = json!({"序号": "1-2"});
        let predicate = json!({
            "__kind": "analysis_expr",
            "type": "matches",
            "field": "序号",
            "pattern": "^\\d+(?:-.*)?$"
        });
        assert!(predicate_matches(&row, &predicate, &BTreeMap::new()));
    }

    #[test]
    fn matches_treats_excel_float_serial_as_integer_text() {
        let row = json!({"序号": 10.0});
        let predicate = json!({
            "__kind": "analysis_expr",
            "type": "matches",
            "field": "序号",
            "pattern": "^\\s*\\d+(?:-.*)?\\s*$"
        });
        assert!(
            predicate_matches(&row, &predicate, &BTreeMap::new()),
            "10.0 must match serial pattern like 10"
        );
    }
}
