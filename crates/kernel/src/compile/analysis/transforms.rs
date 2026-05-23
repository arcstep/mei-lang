use std::collections::BTreeSet;

use serde_json::{json, Value};

use super::dates::{
    aggregate_month_value, format_month_label, latest_month_window, max_row_month, parse_row_date,
};
use super::schema::{parse_number, row_number, row_string, row_value};

pub(super) fn trend_rows_by_month(
    rows: &[Value],
    date_field: &str,
    value_field: Option<&str>,
    agg: &str,
    months: usize,
    label_field: &str,
) -> Vec<Value> {
    let Some(anchor) = max_row_month(rows, date_field) else {
        return Vec::new();
    };
    latest_month_window(anchor, months)
        .into_iter()
        .map(|(year, month)| {
            let value = aggregate_month_value(rows, date_field, value_field, agg, year, month);
            let mut row = serde_json::Map::new();
            row.insert(
                label_field.to_string(),
                Value::String(format_month_label(year, month)),
            );
            row.insert("value".to_string(), json!(value));
            Value::Object(row)
        })
        .collect()
}

pub(super) fn bucket_rows_by_month(rows: &[Value], field: &str, label_field: &str) -> Vec<Value> {
    rows.iter()
        .map(|row| {
            let mut object = row.as_object().cloned().unwrap_or_default();
            let label = parse_row_date(row, field)
                .map(|(year, month, _)| format_month_label(year, month))
                .unwrap_or_default();
            object.insert(label_field.to_string(), Value::String(label));
            Value::Object(object)
        })
        .collect()
}

pub(super) fn aggregate_group_rows(
    rows: &[Value],
    group_field: &str,
    value_field: Option<&str>,
    agg: &str,
    limit: Option<usize>,
) -> Vec<Value> {
    let mut grouped = std::collections::BTreeMap::<String, Vec<f64>>::new();
    let mut counts = std::collections::BTreeMap::<String, usize>::new();
    for row in rows {
        let label = row_string(row, group_field);
        if label.is_empty() {
            continue;
        }
        *counts.entry(label.clone()).or_insert(0) += 1;
        if let Some(field) = value_field {
            if let Some(number) = row_number(row, field) {
                grouped.entry(label).or_default().push(number);
            } else {
                grouped.entry(label).or_default();
            }
        } else {
            grouped.entry(label).or_default();
        }
    }
    let mut out = Vec::new();
    for (label, numbers) in grouped {
        let value = match agg {
            "sum" => numbers.iter().sum::<f64>(),
            "avg" => {
                if numbers.is_empty() {
                    0.0
                } else {
                    numbers.iter().sum::<f64>() / numbers.len() as f64
                }
            }
            "min" => numbers.into_iter().reduce(f64::min).unwrap_or(0.0),
            "max" => numbers.into_iter().reduce(f64::max).unwrap_or(0.0),
            _ => counts.get(&label).copied().unwrap_or(0) as f64,
        };
        let mut row = serde_json::Map::new();
        row.insert(group_field.to_string(), Value::String(label));
        row.insert("value".to_string(), json!(value));
        out.push(Value::Object(row));
    }
    if let Some(limit) = limit {
        out.truncate(limit);
    }
    out
}

pub(super) fn summarize_rows(rows: &[Value], agg: &str, value_field: &str) -> Vec<Value> {
    if agg == "count" {
        return vec![json!({ "value": rows.len() })];
    }
    let values = rows
        .iter()
        .filter_map(|row| row_number(row, value_field))
        .collect::<Vec<_>>();
    let value = match agg {
        "sum" => values.iter().sum::<f64>(),
        "avg" => {
            if values.is_empty() {
                0.0
            } else {
                values.iter().sum::<f64>() / values.len() as f64
            }
        }
        "min" => values.into_iter().reduce(f64::min).unwrap_or(0.0),
        "max" => values.into_iter().reduce(f64::max).unwrap_or(0.0),
        _ => 0.0,
    };
    vec![json!({ "value": value })]
}

pub(super) fn select_fields(row: &Value, fields: &[String]) -> Value {
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

pub(super) fn rename_fields(row: &Value, mapping: &serde_json::Map<String, Value>) -> Value {
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

pub(super) fn reorder_fields(row: &Value, fields: &[String]) -> Value {
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

#[cfg(test)]
mod tests {
    use super::trend_rows_by_month;
    use serde_json::json;

    #[test]
    fn trend_by_month_fills_missing_buckets_with_zero() {
        let rows = vec![
            json!({"做出处罚日期": "2024-05-10", "罚款金额": 100}),
            json!({"做出处罚日期": "2024-06-10", "罚款金额": 200}),
        ];
        let trend = trend_rows_by_month(&rows, "做出处罚日期", Some("罚款金额"), "sum", 6, "month");
        assert_eq!(trend.len(), 6);
        assert_eq!(
            trend[0].get("month").and_then(|v| v.as_str()),
            Some("2024-01")
        );
        assert_eq!(trend[0].get("value").and_then(|v| v.as_f64()), Some(0.0));
        assert_eq!(trend[4].get("value").and_then(|v| v.as_f64()), Some(100.0));
        assert_eq!(trend[5].get("value").and_then(|v| v.as_f64()), Some(200.0));
    }
}

pub(super) fn sort_rows_by_field(rows: &mut [Value], field: &str, order: &str) {
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

pub(super) fn first_rows_by_field(rows: &[Value], field: &str) -> Vec<Value> {
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

pub(super) fn distinct_rows_by_fields(rows: &[Value], fields: &[String]) -> Vec<Value> {
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

pub(super) fn mutate_row(row: &Value, updates: &serde_json::Map<String, Value>) -> Value {
    let mut out = row.as_object().cloned().unwrap_or_default();
    for (key, expr) in updates {
        out.insert(key.clone(), eval_row_value(expr, &out));
    }
    Value::Object(out)
}

fn eval_row_value(expr: &Value, row: &serde_json::Map<String, Value>) -> Value {
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
                    let text = row.get(field).and_then(Value::as_str).unwrap_or_default();
                    let extracted = text
                        .chars()
                        .filter(|ch| ch.is_ascii_digit() || *ch == '.')
                        .collect::<String>();
                    extracted
                        .parse::<f64>()
                        .ok()
                        .map(|value| json!(value))
                        .unwrap_or(Value::Null)
                }
                _ => expr.clone(),
            };
        }
    }
    expr.clone()
}
