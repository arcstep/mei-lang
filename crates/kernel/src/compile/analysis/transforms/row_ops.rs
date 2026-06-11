use std::collections::BTreeSet;

use regex::Regex;
use serde_json::{json, Value};

use super::super::schema::{parse_number, row_string, row_value, value_display_text};

fn scalar_text(value: &Value) -> String {
    value_display_text(value)
}

fn regex_capture_text(text: &str, pattern: &str) -> String {
    if pattern.is_empty() {
        return String::new();
    }
    Regex::new(pattern)
        .ok()
        .and_then(|regex| {
            regex
                .captures(text)
                .and_then(|captures| captures.get(1).or_else(|| captures.get(0)))
                .map(|matched| matched.as_str().to_string())
        })
        .unwrap_or_default()
}

pub fn select_fields(row: &Value, fields: &[String]) -> Value {
    let Some(object) = row.as_object() else {
        return row.clone();
    };
    let mut out = serde_json::Map::new();
    for field in fields {
        if let Some(value) = object.get(field) {
            out.insert(field.clone(), value.clone());
        }
    }
    Value::Object(out)
}

pub fn rename_fields(row: &Value, mapping: &serde_json::Map<String, Value>) -> Value {
    let Some(object) = row.as_object() else {
        return row.clone();
    };
    let mut out = serde_json::Map::new();
    for (key, value) in object {
        let renamed = mapping
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or(key)
            .to_string();
        out.insert(renamed, value.clone());
    }
    Value::Object(out)
}

pub fn reorder_fields(row: &Value, fields: &[String]) -> Value {
    let Some(object) = row.as_object() else {
        return row.clone();
    };
    let mut out = serde_json::Map::new();
    for field in fields {
        if let Some(value) = object.get(field) {
            out.insert(field.clone(), value.clone());
        }
    }
    for (key, value) in object {
        if !out.contains_key(key) {
            out.insert(key.clone(), value.clone());
        }
    }
    Value::Object(out)
}

pub fn sort_rows_by_field(rows: &mut [Value], field: &str, order: &str) {
    rows.sort_by(|left, right| {
        let l = row_value(left, field).cloned().unwrap_or(Value::Null);
        let r = row_value(right, field).cloned().unwrap_or(Value::Null);
        compare_json_values(&l, &r)
    });
    if order.eq_ignore_ascii_case("desc") {
        rows.reverse();
    }
}

fn compare_json_values(left: &Value, right: &Value) -> std::cmp::Ordering {
    if let (Some(l), Some(r)) = (parse_number(left), parse_number(right)) {
        return l.partial_cmp(&r).unwrap_or(std::cmp::Ordering::Equal);
    }
    left.to_string().cmp(&right.to_string())
}

pub fn first_rows_by_field(rows: &[Value], field: &str) -> Vec<Value> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for row in rows {
        let key = row_string(row, field);
        if seen.insert(key) {
            out.push(row.clone());
        }
    }
    out
}

pub fn distinct_rows_by_fields(rows: &[Value], fields: &[String]) -> Vec<Value> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for row in rows {
        let key = fields
            .iter()
            .map(|field| {
                row_value(row, field)
                    .cloned()
                    .unwrap_or(Value::Null)
                    .to_string()
            })
            .collect::<Vec<_>>()
            .join("\u{1f}");
        if seen.insert(key) {
            out.push(row.clone());
        }
    }
    out
}

pub fn mutate_row(row: &Value, updates: &serde_json::Map<String, Value>) -> Value {
    let mut out = row.as_object().cloned().unwrap_or_default();
    for (key, expr) in updates {
        out.insert(key.clone(), eval_row_value(expr, &out));
    }
    Value::Object(out)
}

pub fn eval_row_value(expr: &Value, row: &serde_json::Map<String, Value>) -> Value {
    if let Some(analysis) = expr.as_object() {
        if analysis.get("__kind").and_then(Value::as_str) == Some("analysis_expr") {
            let analysis_type = analysis.get("type").and_then(Value::as_str).unwrap_or("");
            return match analysis_type {
                "lit" => analysis.get("value").cloned().unwrap_or(Value::Null),
                "col" => analysis
                    .get("field")
                    .and_then(Value::as_str)
                    .and_then(|field| row.get(field).cloned())
                    .unwrap_or(Value::Null),
                "number" => {
                    let field = analysis.get("field").and_then(Value::as_str).unwrap_or("");
                    row.get(field)
                        .and_then(parse_number)
                        .map(|value| json!(value))
                        .unwrap_or(Value::Null)
                }
                "text" => {
                    let field = analysis.get("field").and_then(Value::as_str).unwrap_or("");
                    row.get(field)
                        .map(|value| {
                            Value::String(value.as_str().unwrap_or(&value.to_string()).to_string())
                        })
                        .unwrap_or(Value::Null)
                }
                "extract_number" => {
                    let field = analysis.get("field").and_then(Value::as_str).unwrap_or("");
                    let text = row.get(field).map(scalar_text).unwrap_or_default();
                    let pattern = analysis
                        .get("pattern")
                        .and_then(Value::as_str)
                        .unwrap_or("");
                    let extracted = if pattern.is_empty() {
                        text.chars()
                            .filter(|ch| ch.is_ascii_digit() || *ch == '.')
                            .collect::<String>()
                    } else {
                        regex_capture_text(&text, pattern)
                    };
                    extracted
                        .parse::<f64>()
                        .ok()
                        .map(|value| json!(value))
                        .unwrap_or(Value::Null)
                }
                "extract_match" => {
                    let field = analysis.get("field").and_then(Value::as_str).unwrap_or("");
                    let text = row.get(field).map(scalar_text).unwrap_or_default();
                    let pattern = analysis
                        .get("pattern")
                        .and_then(Value::as_str)
                        .unwrap_or("");
                    Value::String(regex_capture_text(&text, pattern))
                }
                "sub" => {
                    let left_field = analysis
                        .get("left_field")
                        .and_then(Value::as_str)
                        .unwrap_or("");
                    let right_field = analysis
                        .get("right_field")
                        .and_then(Value::as_str)
                        .unwrap_or("");
                    let left = row.get(left_field).and_then(parse_number).unwrap_or(0.0);
                    let right = row.get(right_field).and_then(parse_number).unwrap_or(0.0);
                    json!(left - right)
                }
                _ => expr.clone(),
            };
        }
    }
    expr.clone()
}
