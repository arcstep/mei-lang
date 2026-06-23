use serde_json::Value;

use crate::model::ColumnSchema;

use super::schema::{row_number, row_value};

/// 从行字段解析日历日期；支持常见字符串与 Excel 序列日。
pub(super) fn parse_row_date(row: &Value, field: &str) -> Option<(i32, u32, u32)> {
    let value = row_value(row, field)?;
    parse_date_value(value)
}

pub(super) fn parse_date_value(value: &Value) -> Option<(i32, u32, u32)> {
    match value {
        Value::String(raw) => parse_date_text(raw).or_else(|| {
            raw.trim()
                .parse::<f64>()
                .ok()
                .and_then(parse_excel_serial_date)
        }),
        Value::Number(number) => number.as_f64().and_then(parse_excel_serial_date),
        _ => None,
    }
}

fn parse_date_text(raw: &str) -> Option<(i32, u32, u32)> {
    let text = raw.trim();
    if text.is_empty() {
        return None;
    }
    // `YYYY-MM-DD HH:MM:SS` / ISO datetime：只取日历日部分。
    let date_token = text.split(['T', 't', ' ']).next().unwrap_or(text).trim();
    let normalized = date_token
        .replace('年', "-")
        .replace('月', "-")
        .replace('日', "")
        .replace('/', "-")
        .replace('.', "-");
    let parts = normalized
        .split('-')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if parts.len() >= 3 {
        let year = parts[0].parse::<i32>().ok()?;
        let month = parts[1].parse::<u32>().ok()?;
        let day = parts[2].parse::<u32>().ok()?;
        if (1..=12).contains(&month) && (1..=31).contains(&day) {
            return Some((year, month, day));
        }
    }
    None
}

/// Excel 序列日：0 = 1899-12-30（与 calamine / Excel 1900 date system 一致）。
fn parse_excel_serial_date(serial: f64) -> Option<(i32, u32, u32)> {
    if !serial.is_finite() || serial <= 0.0 {
        return None;
    }
    let days = serial.floor() as i32;
    Some(civil_ymd_from_days(
        civil_days_from_ymd(1899, 12, 30) + days,
    ))
}

/// Proleptic Gregorian civil days (Howard Hinnant).
fn civil_days_from_ymd(year: i32, month: u32, day: u32) -> i32 {
    let month = month as i32;
    let day = day as i32;
    let y = year - if month <= 2 { 1 } else { 0 };
    let era = if y >= 0 { y / 400 } else { (y - 399) / 400 };
    let yoe = y - era * 400;
    let doy = (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + yoe / 400 + doy;
    era * 146097 + doe - 719468
}

fn civil_ymd_from_days(days: i32) -> (i32, u32, u32) {
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if m <= 2 { 1 } else { 0 };
    (year, m as u32, d as u32)
}

pub(super) fn format_month_label(year: i32, month: u32) -> String {
    format!("{year:04}-{month:02}")
}

pub(super) fn format_iso_date((year, month, day): (i32, u32, u32)) -> String {
    format!("{year:04}-{month:02}-{day:02}")
}

/// 将可解析的日历日（含 `YYYY-MM-DD HH:MM:SS`、Excel 序列日）规范为 `YYYY-MM-DD` 字符串；无法解析则原样返回。
pub fn format_calendar_date_value(value: &Value) -> Value {
    parse_date_value(value)
        .map(|ymd| Value::String(format_iso_date(ymd)))
        .unwrap_or_else(|| value.clone())
}

/// 按列名（`*时间` / `*日期`）与可选 schema 把行内日历列规范为 `YYYY-MM-DD`。
pub fn coerce_calendar_columns_in_rows(
    rows: Vec<Value>,
    columns: &[String],
    schema: &[ColumnSchema],
) -> Vec<Value> {
    let mut effective_schema: Vec<ColumnSchema> = schema
        .iter()
        .filter(|column| {
            let type_name = column.type_name.as_str();
            type_name == "date" || type_name == "datetime"
        })
        .cloned()
        .collect();
    let known: std::collections::BTreeSet<String> = effective_schema
        .iter()
        .map(|column| column.name.clone())
        .collect();
    for name in columns {
        if known.contains(name) {
            continue;
        }
        if name.ends_with("时间") || name.ends_with("日期") {
            effective_schema.push(ColumnSchema {
                name: name.clone(),
                type_name: "date".to_string(),
                source: None,
                optional: true,
                unit: None,
            });
        }
    }
    if effective_schema.is_empty() {
        return rows;
    }
    coerce_rows_to_schema(rows, &effective_schema)
}

/// 按 dataset schema 的 `date` / `datetime` 列把 Excel 序列日等值规范为 `YYYY-MM-DD` 字符串。
pub fn coerce_row_to_schema(row: &Value, schema: &[ColumnSchema]) -> Value {
    if schema.is_empty() {
        return row.clone();
    }
    let Some(obj) = row.as_object() else {
        return row.clone();
    };
    let mut out = obj.clone();
    for column in schema {
        let type_name = column.type_name.as_str();
        if type_name == "integer" {
            if let Some(value) = out.get(&column.name) {
                out.insert(column.name.clone(), coerce_value_to_integer(value));
            }
            continue;
        }
        if type_name != "date" && type_name != "datetime" {
            continue;
        }
        if let Some(value) = out.get(&column.name) {
            out.insert(column.name.clone(), coerce_value_to_date_string(value));
        }
    }
    Value::Object(out)
}

/// 按 dataset schema 的 `date` / `datetime` 列把 Excel 序列日等值规范为 `YYYY-MM-DD` 字符串。
pub fn coerce_rows_to_schema(rows: Vec<Value>, schema: &[ColumnSchema]) -> Vec<Value> {
    if schema.is_empty() {
        return rows;
    }
    rows.into_iter()
        .map(|row| coerce_row_to_schema(&row, schema))
        .collect()
}

fn coerce_value_to_integer(value: &Value) -> Value {
    if let Some(integer) = value.as_i64() {
        return serde_json::json!(integer);
    }
    if let Some(number) = value.as_f64() {
        if number.fract() == 0.0 {
            let rounded = number.round();
            if rounded >= i64::MIN as f64 && rounded <= i64::MAX as f64 {
                return serde_json::json!(rounded as i64);
            }
        }
    }
    value.clone()
}

fn coerce_value_to_date_string(value: &Value) -> Value {
    parse_date_value(value)
        .map(|ymd| Value::String(format_iso_date(ymd)))
        .unwrap_or_else(|| value.clone())
}

pub(super) fn add_months(year: i32, month: u32, delta: i32) -> (i32, u32) {
    let total = year * 12 + month as i32 - 1 + delta;
    let next_year = total.div_euclid(12);
    let next_month = total.rem_euclid(12) + 1;
    (next_year, next_month as u32)
}

pub(super) fn latest_month_window(anchor: (i32, u32), months: usize) -> Vec<(i32, u32)> {
    let count = months.max(1);
    let mut out = Vec::with_capacity(count);
    for offset in 0..count {
        let delta = -(count as i32 - 1) + offset as i32;
        out.push(add_months(anchor.0, anchor.1, delta));
    }
    out
}

pub(super) fn max_row_month(rows: &[Value], field: &str) -> Option<(i32, u32)> {
    rows.iter()
        .filter_map(|row| parse_row_date(row, field))
        .map(|(year, month, _)| (year, month))
        .max()
}

pub(super) fn filter_rows_in_latest_months(
    rows: &[Value],
    field: &str,
    months: usize,
) -> Vec<Value> {
    let Some(anchor) = max_row_month(rows, field) else {
        return Vec::new();
    };
    let window = latest_month_window(anchor, months);
    rows.iter()
        .filter(|row| {
            parse_row_date(row, field)
                .map(|(year, month, _)| window.contains(&(year, month)))
                .unwrap_or(false)
        })
        .cloned()
        .collect()
}

pub(super) fn filter_rows_in_latest_days(rows: &[Value], field: &str, days: usize) -> Vec<Value> {
    let mut parsed = Vec::new();
    for row in rows {
        if let Some(date) = parse_row_date(row, field) {
            parsed.push((row, date_ord(date)));
        }
    }
    if parsed.is_empty() {
        return Vec::new();
    }
    let max_ord = parsed.iter().map(|(_, ord)| *ord).max().unwrap_or(0);
    let min_ord = max_ord.saturating_sub(days.saturating_sub(1) as i64);
    parsed
        .into_iter()
        .filter(|(_, ord)| *ord >= min_ord)
        .map(|(row, _)| row.clone())
        .collect()
}

fn date_ord((year, month, day): (i32, u32, u32)) -> i64 {
    year as i64 * 10_000 + month as i64 * 100 + day as i64
}

pub(super) fn row_date_in_inclusive_range(
    row: &Value,
    field: &str,
    lower: &Value,
    upper: &Value,
) -> bool {
    let Some(actual) = parse_row_date(row, field).map(date_ord) else {
        return false;
    };
    let Some(lo) = parse_date_value(lower).map(date_ord) else {
        return false;
    };
    let Some(hi) = parse_date_value(upper).map(date_ord) else {
        return false;
    };
    actual >= lo && actual <= hi
}

pub(super) fn row_in_month(row: &Value, field: &str, year: i32, month: u32) -> bool {
    parse_row_date(row, field)
        .map(|(y, m, _)| y == year && m == month)
        .unwrap_or(false)
}

pub(super) fn aggregate_month_value(
    rows: &[Value],
    date_field: &str,
    value_field: Option<&str>,
    agg: &str,
    year: i32,
    month: u32,
) -> f64 {
    let mut numbers = Vec::new();
    let mut count = 0usize;
    for row in rows {
        if !row_in_month(row, date_field, year, month) {
            continue;
        }
        count += 1;
        if let Some(field) = value_field {
            if let Some(number) = row_number(row, field) {
                numbers.push(number);
            }
        }
    }
    match agg {
        "sum" => numbers.iter().sum(),
        "avg" => {
            if numbers.is_empty() {
                0.0
            } else {
                numbers.iter().sum::<f64>() / numbers.len() as f64
            }
        }
        "min" => numbers.into_iter().reduce(f64::min).unwrap_or(0.0),
        "max" => numbers.into_iter().reduce(f64::max).unwrap_or(0.0),
        _ => count as f64,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        filter_rows_in_latest_days, format_month_label, latest_month_window, parse_date_text,
        parse_date_value,
    };
    use serde_json::json;

    #[test]
    fn parse_excel_serial_matches_excel_epoch() {
        assert_eq!(parse_date_value(&json!(45960)), Some((2025, 10, 30)));
        assert_eq!(parse_date_value(&json!(46020)), Some((2025, 12, 29)));
        assert_eq!(parse_date_value(&json!("45960")), Some((2025, 10, 30)));
    }

    #[test]
    fn latest_days_anchors_on_max_row_date_not_wall_clock() {
        let rows = vec![
            json!({"检查日期": 45954}),
            json!({"检查日期": 45960}),
            json!({"检查日期": 45950}),
        ];
        let filtered = filter_rows_in_latest_days(&rows, "检查日期", 7);
        assert_eq!(filtered.len(), 2);
        assert!(filtered
            .iter()
            .all(|row| row.get("检查日期").and_then(|v| v.as_f64()) != Some(45950.0)));
    }

    #[test]
    fn latest_month_window_ends_at_anchor() {
        let window = latest_month_window((2024, 6), 6);
        assert_eq!(window.len(), 6);
        assert_eq!(window.first().copied(), Some((2024, 1)));
        assert_eq!(window.last().copied(), Some((2024, 6)));
        assert_eq!(format_month_label(2024, 6), "2024-06");
    }

    #[test]
    fn row_date_in_inclusive_range_matches_iso_bounds() {
        use super::row_date_in_inclusive_range;
        let row = json!({"检查日期": "2024-06-15"});
        assert!(row_date_in_inclusive_range(
            &row,
            "检查日期",
            &json!("2024-01-01"),
            &json!("2024-12-31"),
        ));
        assert!(!row_date_in_inclusive_range(
            &row,
            "检查日期",
            &json!("2025-01-01"),
            &json!("2025-12-31"),
        ));
    }

    #[test]
    fn parse_common_date_strings() {
        assert_eq!(parse_date_text("2024-06-15"), Some((2024, 6, 15)));
        assert_eq!(parse_date_text("2024/6/15"), Some((2024, 6, 15)));
        assert_eq!(
            parse_date_value(&json!("2024年6月15日")),
            Some((2024, 6, 15))
        );
        assert_eq!(parse_date_text("2025-06-06 00:00:00"), Some((2025, 6, 6)));
        assert_eq!(
            parse_date_value(&json!("2025-06-06T08:30:00")),
            Some((2025, 6, 6))
        );
    }

    #[test]
    fn coerce_calendar_columns_in_rows_uses_time_suffix_columns() {
        use super::{coerce_calendar_columns_in_rows, format_iso_date};

        let rows = vec![json!({
            "预警ID": "YJ1",
            "预警时间": "2025-10-01 00:00:00",
            "分办时间": 46023
        })];
        let columns = vec![
            "预警ID".to_string(),
            "预警时间".to_string(),
            "分办时间".to_string(),
        ];
        let out = coerce_calendar_columns_in_rows(rows, &columns, &[]);
        assert_eq!(
            out[0].get("预警时间").and_then(|v| v.as_str()),
            Some("2025-10-01")
        );
        let parsed = parse_date_value(&json!(46023)).expect("excel serial");
        assert_eq!(
            out[0].get("分办时间").and_then(|v| v.as_str()),
            Some(format_iso_date(parsed).as_str())
        );
    }

    #[test]
    fn coerce_rows_to_schema_converts_excel_serial_datetime_columns() {
        use super::{coerce_rows_to_schema, format_iso_date};
        use crate::model::ColumnSchema;

        let rows = vec![json!({"预警时间": 46023, "预警ID": "w1"})];
        let schema = vec![ColumnSchema {
            name: "预警时间".to_string(),
            type_name: "datetime".to_string(),
            source: None,
            optional: false,
            unit: None,
        }];
        let out = coerce_rows_to_schema(rows, &schema);
        let parsed = parse_date_value(&json!(46023)).expect("46023 serial");
        assert_eq!(
            out[0].get("预警时间").and_then(|v| v.as_str()),
            Some(format_iso_date(parsed).as_str())
        );
    }
}
