use serde_json::{json, Value};

use super::super::dates::{
    aggregate_month_value, format_month_label, latest_month_window, max_row_month, parse_row_date,
};
use super::super::schema::row_string;

pub fn trend_year_compare_rows(
    rows: &[Value],
    date_field: &str,
    value_field: Option<&str>,
    agg: &str,
    months: usize,
    years: &[i32],
    month_label_field: &str,
    year_label_field: &str,
    window_mode: &str,
) -> Vec<Value> {
    let month_nums: Vec<u32> = if window_mode.eq_ignore_ascii_case("calendar") {
        (1..=12).collect()
    } else {
        let Some(anchor) = max_row_month(rows, date_field) else {
            return Vec::new();
        };
        latest_month_window(anchor, months)
            .into_iter()
            .map(|(_, month)| month)
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect()
    };
    if month_nums.is_empty() {
        return Vec::new();
    }
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

pub fn trend_rows_by_month(
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

pub fn bucket_rows_by_month(rows: &[Value], field: &str, label_field: &str) -> Vec<Value> {
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
