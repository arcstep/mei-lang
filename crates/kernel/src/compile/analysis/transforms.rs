use std::collections::{BTreeMap, BTreeSet};

use regex::Regex;
use serde_json::{json, Value};

use super::dates::{
    aggregate_month_value, format_month_label, latest_month_window, max_row_month, parse_row_date,
};
use super::schema::{parse_number, row_number, row_string, row_value, value_display_text};

fn scalar_text(value: &Value) -> String {
    value_display_text(value)
}

pub(super) fn trend_year_compare_rows(
    rows: &[Value],
    date_field: &str,
    value_field: Option<&str>,
    agg: &str,
    months: usize,
    years: &[i32],
    month_label_field: &str,
    year_label_field: &str,
) -> Vec<Value> {
    let Some(anchor) = max_row_month(rows, date_field) else {
        return Vec::new();
    };
    let window = latest_month_window(anchor, months);
    let month_nums = window
        .iter()
        .map(|(_, month)| *month)
        .collect::<BTreeSet<_>>();
    let mut out = Vec::new();
    for month in month_nums {
        for year in years {
            let value = aggregate_month_value(rows, date_field, value_field, agg, *year, month);
            let mut row = serde_json::Map::new();
            row.insert(
                month_label_field.to_string(),
                Value::String(format!("{month:02}")),
            );
            row.insert(
                year_label_field.to_string(),
                Value::String(format!("{year}")),
            );
            row.insert("value".to_string(), json!(value));
            out.push(Value::Object(row));
        }
    }
    out.sort_by(|left, right| {
        let left_month = row_string(left, month_label_field);
        let left_year = row_string(left, year_label_field);
        let right_month = row_string(right, month_label_field);
        let right_year = row_string(right, year_label_field);
        left_month
            .cmp(&right_month)
            .then_with(|| left_year.cmp(&right_year))
    });
    out
}

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

pub(super) fn party_year_aggregate_rows(
    rows: &[Value],
    party_field: &str,
    date_field: &str,
    value_field: &str,
    years: &[i32],
) -> Vec<Value> {
    let mut parties: BTreeMap<String, BTreeMap<i32, (f64, usize)>> = BTreeMap::new();
    for row in rows {
        let party = row_string(row, party_field);
        if party.is_empty() {
            continue;
        }
        let Some((year, _, _)) = parse_row_date(row, date_field) else {
            continue;
        };
        if !years.contains(&year) {
            continue;
        }
        let amount = row_number(row, value_field).unwrap_or(0.0);
        let entry = parties.entry(party).or_default().entry(year).or_insert((0.0, 0));
        entry.0 += amount;
        entry.1 += 1;
    }
    let mut out = Vec::new();
    for (party, year_stats) in parties {
        let mut object = serde_json::Map::new();
        object.insert(party_field.to_string(), Value::String(party));
        for year in years {
            let (sum, count) = year_stats.get(year).copied().unwrap_or((0.0, 0));
            object.insert(format!("罚没金额_{year}"), json!(sum));
            object.insert(format!("处罚次数_{year}"), json!(count as f64));
        }
        if years.len() >= 2 {
            let prev = years[years.len() - 2];
            let curr = years[years.len() - 1];
            let prev_sum = year_stats.get(&prev).map(|(sum, _)| *sum).unwrap_or(0.0);
            let curr_sum = year_stats.get(&curr).map(|(sum, _)| *sum).unwrap_or(0.0);
            object.insert(
                format!("同比降低额_{curr}"),
                json!((prev_sum - curr_sum).max(0.0)),
            );
        }
        out.push(Value::Object(object));
    }
    out
}

pub(super) fn unpivot_columns_rows(
    rows: &[Value],
    id_field: &str,
    columns: &[(String, String)],
    year_field: &str,
    value_field: &str,
) -> Vec<Value> {
    let mut out = Vec::new();
    for row in rows {
        let id = row_string(row, id_field);
        if id.is_empty() {
            continue;
        }
        for (year_label, source_field) in columns {
            let value = row_number(row, source_field).unwrap_or(0.0);
            let mut object = serde_json::Map::new();
            object.insert(id_field.to_string(), Value::String(id.clone()));
            object.insert(year_field.to_string(), Value::String(year_label.clone()));
            object.insert(value_field.to_string(), json!(value));
            out.push(Value::Object(object));
        }
    }
    out
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
    use super::{eval_row_value, trend_rows_by_month, trend_year_compare_rows};
    use serde_json::json;

    #[test]
    fn trend_year_compare_aligns_months_across_years() {
        let rows = vec![
            json!({"检查日期": "2024-03-10"}),
            json!({"检查日期": "2024-03-12"}),
            json!({"检查日期": "2025-03-15"}),
            json!({"检查日期": "2025-06-01"}),
        ];
        let trend = trend_year_compare_rows(
            &rows,
            "检查日期",
            None,
            "count",
            6,
            &[2024, 2025],
            "month",
            "year",
        );
        let march_2024 = trend
            .iter()
            .find(|row| {
                row.get("month").and_then(|v| v.as_str()) == Some("03")
                    && row.get("year").and_then(|v| v.as_str()) == Some("2024")
            })
            .and_then(|row| row.get("value").and_then(|v| v.as_f64()));
        let march_2025 = trend
            .iter()
            .find(|row| {
                row.get("month").and_then(|v| v.as_str()) == Some("03")
                    && row.get("year").and_then(|v| v.as_str()) == Some("2025")
            })
            .and_then(|row| row.get("value").and_then(|v| v.as_f64()));
        assert_eq!(march_2024, Some(2.0));
        assert_eq!(march_2025, Some(1.0));
    }

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

    #[test]
    fn party_year_aggregate_sums_by_execution_year_and_party() {
        use super::party_year_aggregate_rows;

        let rows = vec![
            json!({"当事人": "甲公司", "执行日期": "2024-06-01", "罚款金额": 20000}),
            json!({"当事人": "甲公司", "执行日期": "2024-08-01", "罚款金额": 5000}),
            json!({"当事人": "甲公司", "执行日期": "2025-03-01", "罚款金额": 30000}),
            json!({"当事人": "乙公司", "执行日期": "2025-01-01", "罚款金额": 12000}),
        ];
        let stats = party_year_aggregate_rows(
            &rows,
            "当事人",
            "执行日期",
            "罚款金额",
            &[2024, 2025],
        );
        let a = stats
            .iter()
            .find(|row| row.get("当事人").and_then(|v| v.as_str()) == Some("甲公司"))
            .expect("甲公司");
        assert_eq!(a.get("罚没金额_2024").and_then(|v| v.as_f64()), Some(25000.0));
        assert_eq!(a.get("处罚次数_2024").and_then(|v| v.as_f64()), Some(2.0));
        assert_eq!(a.get("罚没金额_2025").and_then(|v| v.as_f64()), Some(30000.0));
        assert_eq!(a.get("同比降低额_2025").and_then(|v| v.as_f64()), Some(0.0));
    }

    #[test]
    fn unpivot_columns_expands_year_metrics_for_chart() {
        use super::unpivot_columns_rows;

        let rows = vec![json!({
            "当事人": "甲公司",
            "罚没金额_2024": 25000,
            "罚没金额_2025": 30000,
        })];
        let bars = unpivot_columns_rows(
            &rows,
            "当事人",
            &[
                ("2024".to_string(), "罚没金额_2024".to_string()),
                ("2025".to_string(), "罚没金额_2025".to_string()),
            ],
            "year",
            "value",
        );
        assert_eq!(bars.len(), 2);
        assert_eq!(
            bars[0].get("year").and_then(|v| v.as_str()),
            Some("2024")
        );
        assert_eq!(bars[0].get("value").and_then(|v| v.as_f64()), Some(25000.0));
    }

    #[test]
    fn extract_number_supports_regex_prefix_on_string_and_numeric_cells() {
        let expr = json!({
            "__kind": "analysis_expr",
            "type": "extract_number",
            "field": "序号",
            "pattern": "^\\s*(\\d+)"
        });
        let row_text = serde_json::Map::from_iter([(String::from("序号"), json!("1-2"))]);
        let row_number = serde_json::Map::from_iter([(String::from("序号"), json!(10))]);
        assert_eq!(eval_row_value(&expr, &row_text).as_f64(), Some(1.0));
        assert_eq!(eval_row_value(&expr, &row_number).as_f64(), Some(10.0));
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
                        Regex::new(pattern)
                            .ok()
                            .and_then(|regex| {
                                regex
                                    .captures(&text)
                                    .and_then(|captures| {
                                        captures.get(1).or_else(|| captures.get(0))
                                    })
                                    .map(|matched| matched.as_str().to_string())
                            })
                            .unwrap_or_default()
                    };
                    extracted
                        .parse::<f64>()
                        .ok()
                        .map(|value| json!(value))
                        .unwrap_or(Value::Null)
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
