use serde_json::{json, Value};

use super::super::dates::{
    aggregate_month_value, format_month_label, latest_month_window, max_row_month, parse_row_date,
};

/// Auto-mode cap: keep the most recent N distinct years (024008).
pub const TREND_YEAR_COMPARE_MAX_YEARS: usize = 5;

/// Resolve years for `trend_year_compare`.
///
/// - `requested` empty → distinct years from filtered rows (asc), capped to most recent 5
/// - `requested` non-empty → intersection with years present in rows (preserve requested order)
pub fn resolve_trend_compare_years(
    rows: &[Value],
    date_field: &str,
    requested: Option<&[i32]>,
) -> Vec<i32> {
    let mut present = std::collections::BTreeSet::new();
    for row in rows {
        if let Some((year, _, _)) = parse_row_date(row, date_field) {
            present.insert(year);
        }
    }
    if present.is_empty() {
        return Vec::new();
    }
    match requested {
        Some(wanted) if !wanted.is_empty() => wanted
            .iter()
            .copied()
            .filter(|year| present.contains(year))
            .collect(),
        _ => {
            let mut years: Vec<i32> = present.into_iter().collect();
            if years.len() > TREND_YEAR_COMPARE_MAX_YEARS {
                let drop_n = years.len() - TREND_YEAR_COMPARE_MAX_YEARS;
                years.drain(0..drop_n);
            }
            years
        }
    }
}

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
    if years.is_empty() {
        return Vec::new();
    }
    let month_nums: Vec<u32> = if window_mode.eq_ignore_ascii_case("calendar") {
        (1..=12).collect()
    } else {
        let Some(anchor) = max_row_month(rows, date_field) else {
            return Vec::new();
        };
        // 保持 latest_month_window 的时间升序（勿 BTreeSet：跨年窗口会变成 01..03,10..12）。
        let mut seen = std::collections::HashSet::new();
        latest_month_window(anchor, months)
            .into_iter()
            .map(|(_, month)| month)
            .filter(|month| seen.insert(*month))
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
