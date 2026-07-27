use std::collections::BTreeMap;

use anyhow::{bail, Result};
use chrono::Datelike;

pub fn quote_ident(name: &str) -> Result<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        bail!("empty SQL identifier");
    }
    if trimmed.contains('\0') {
        bail!("SQL identifier contains NUL");
    }
    Ok(format!("\"{}\"", trimmed.replace('"', "\"\"")))
}

pub fn quote_string(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

/// Map MeiLang column type_name to DataFusion / Arrow SQL cast type.
pub fn sql_cast_type(type_name: &str) -> &'static str {
    match type_name.trim().to_ascii_lowercase().as_str() {
        "int" | "integer" | "i64" | "i32" | "long" => "BIGINT",
        "float" | "double" | "number" | "f64" | "f32" => "DOUBLE",
        "bool" | "boolean" => "BOOLEAN",
        "date" => "DATE",
        "datetime" | "timestamp" => "TIMESTAMP",
        _ => "VARCHAR",
    }
}

/// Build a tolerant `try_cast` projection for parquet physical columns.
///
/// Excel-origin integers often land as Float64 / Utf8 `"1.0"`. Direct
/// `try_cast(x AS BIGINT)` then yields NULL; route through DOUBLE + round first.
pub fn sql_try_cast_expr(source_ident: &str, type_name: &str) -> String {
    let cast_ty = sql_cast_type(type_name);
    match cast_ty {
        "BIGINT" => format!(
            "try_cast(round(try_cast({source_ident} AS DOUBLE)) AS BIGINT)"
        ),
        _ => format!("try_cast({source_ident} AS {cast_ty})"),
    }
}

/// Historical name used by pipeline_sql lowerers.
#[allow(dead_code)]
pub fn duck_cast_type(type_name: &str) -> &'static str {
    sql_cast_type(type_name)
}

/// Normalize a column that may be DATE / ISO text / Excel serial into DATE.
///
/// DataFusion cannot `try_cast(Date32 AS DOUBLE)` (planning fails even inside
/// `CASE`). Excel-serial decoding therefore goes through `CAST(... AS VARCHAR)`
/// first; native DATE columns still resolve via the leading `try_cast(... AS DATE)`.
pub fn sql_parse_date_expr(column: &str) -> Result<String> {
    let col = quote_ident(column)?;
    Ok(format!(
        "COALESCE(\
           try_cast({col} AS DATE), \
           CASE \
             WHEN try_cast(CAST({col} AS VARCHAR) AS DOUBLE) IS NOT NULL \
              AND try_cast(CAST({col} AS VARCHAR) AS DOUBLE) > 0 \
              AND try_cast(CAST({col} AS VARCHAR) AS DOUBLE) < 2958465 \
             THEN CAST(to_timestamp((try_cast(CAST({col} AS VARCHAR) AS DOUBLE) - 25569.0) * 86400.0) AS DATE) \
             ELSE NULL \
           END\
         )"
    ))
}

/// Inclusive day range (`YYYY-MM-DD..YYYY-MM-DD`) used by `between:` / `drange:`.
///
/// Open-ended forms are allowed:
/// - `2024-01-15..` → `>= start`
/// - `..2024-06-30` → `<= end`
fn sql_date_range_predicate(column: &str, rest: &str) -> Result<Option<String>> {
    let (lower, upper) = match rest.split_once("..") {
        Some((l, u)) => (l.trim(), u.trim()),
        None => return Ok(None),
    };
    if lower.is_empty() && upper.is_empty() {
        return Ok(None);
    }
    let date_expr = sql_parse_date_expr(column)?;
    if lower.is_empty() {
        return Ok(Some(format!(
            "{date_expr} <= CAST({} AS DATE)",
            quote_string(upper)
        )));
    }
    if upper.is_empty() {
        return Ok(Some(format!(
            "{date_expr} >= CAST({} AS DATE)",
            quote_string(lower)
        )));
    }
    Ok(Some(format!(
        "{date_expr} BETWEEN CAST({} AS DATE) AND CAST({} AS DATE)",
        quote_string(lower),
        quote_string(upper)
    )))
}

/// Inclusive month range (`YYYY-MM..YYYY-MM`) used by `mrange:`.
///
/// Open-ended forms are allowed:
/// - `2024-01..` → from first day of start month
/// - `..2024-06` → through last day of end month
fn sql_month_range_predicate(column: &str, rest: &str) -> Result<Option<String>> {
    let (start, end) = match rest.split_once("..") {
        Some((l, u)) => (l.trim(), u.trim()),
        None => return Ok(None),
    };
    let has_start = start.len() >= 7;
    let has_end = end.len() >= 7;
    if !has_start && !has_end {
        return Ok(None);
    }
    let date_expr = sql_parse_date_expr(column)?;
    if has_start && has_end {
        let lower = format!("{}-01", &start[..7]);
        let upper = match month_range_end_day(&end[..7]) {
            Some(day) => day,
            None => return Ok(None),
        };
        return Ok(Some(format!(
            "{date_expr} BETWEEN CAST({} AS DATE) AND CAST({} AS DATE)",
            quote_string(&lower),
            quote_string(&upper)
        )));
    }
    if has_start {
        let lower = format!("{}-01", &start[..7]);
        return Ok(Some(format!(
            "{date_expr} >= CAST({} AS DATE)",
            quote_string(&lower)
        )));
    }
    let upper = match month_range_end_day(&end[..7]) {
        Some(day) => day,
        None => return Ok(None),
    };
    Ok(Some(format!(
        "{date_expr} <= CAST({} AS DATE)",
        quote_string(&upper)
    )))
}

fn month_range_end_day(yyyy_mm: &str) -> Option<String> {
    let (year_s, month_s) = yyyy_mm.split_once('-')?;
    let year: i32 = year_s.parse().ok()?;
    let month: u32 = month_s.parse().ok()?;
    if !(1..=12).contains(&month) {
        return None;
    }
    let (end_year, end_month) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    // Last day = first day of next month minus 1 day (civil calendar).
    let next = chrono::NaiveDate::from_ymd_opt(end_year, end_month, 1)?;
    let last = next.pred_opt()?;
    Some(format!(
        "{:04}-{:02}-{:02}",
        last.year(),
        last.month(),
        last.day()
    ))
}

pub fn build_where_clause(
    filters: &BTreeMap<String, String>,
    search: Option<&str>,
    columns: &[String],
) -> Result<String> {
    let mut parts = Vec::new();
    for (key, expected) in filters {
        let expected = expected.trim();
        if expected.is_empty() {
            continue;
        }
        let col = quote_ident(key)?;
        if let Some(rest) = expected.strip_prefix("gte:") {
            parts.push(format!(
                "try_cast({col} AS DOUBLE) >= {}",
                rest.trim().parse::<f64>().unwrap_or(0.0)
            ));
        } else if let Some(rest) = expected.strip_prefix("lte:") {
            parts.push(format!(
                "try_cast({col} AS DOUBLE) <= {}",
                rest.trim().parse::<f64>().unwrap_or(0.0)
            ));
        } else if let Some(rest) = expected.strip_prefix("gt:") {
            parts.push(format!(
                "try_cast({col} AS DOUBLE) > {}",
                rest.trim().parse::<f64>().unwrap_or(0.0)
            ));
        } else if let Some(rest) = expected.strip_prefix("lt:") {
            parts.push(format!(
                "try_cast({col} AS DOUBLE) < {}",
                rest.trim().parse::<f64>().unwrap_or(0.0)
            ));
        } else if let Some(rest) = expected.strip_prefix("between:") {
            // Encoded as between:YYYY-MM-DD..YYYY-MM-DD (inclusive date range).
            if let Some(pred) = sql_date_range_predicate(key, rest)? {
                parts.push(pred);
            }
        } else if let Some(rest) = expected.strip_prefix("drange:") {
            // filter-bar / query_state contract: drange:YYYY-MM-DD..YYYY-MM-DD
            if let Some(pred) = sql_date_range_predicate(key, rest)? {
                parts.push(pred);
            }
        } else if let Some(rest) = expected.strip_prefix("mrange:") {
            // filter-bar contract: mrange:YYYY-MM..YYYY-MM → inclusive calendar months
            if let Some(pred) = sql_month_range_predicate(key, rest)? {
                parts.push(pred);
            }
        } else if let Some(rest) = expected.strip_prefix("in:") {
            let values: Vec<String> = rest
                .split(',')
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(quote_string)
                .collect();
            if !values.is_empty() {
                parts.push(format!("CAST({col} AS VARCHAR) IN ({})", values.join(", ")));
            }
        } else if let Some(rest) = expected.strip_prefix("not:in:") {
            let values: Vec<String> = rest
                .split(',')
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(quote_string)
                .collect();
            if !values.is_empty() {
                parts.push(format!(
                    "CAST({col} AS VARCHAR) NOT IN ({})",
                    values.join(", ")
                ));
            }
        } else if let Some(rest) = expected.strip_prefix("contains_any:") {
            let needles: Vec<&str> = rest
                .split(',')
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .collect();
            if !needles.is_empty() {
                let ors: Vec<String> = needles
                    .into_iter()
                    .map(|needle| {
                        format!(
                            "strpos(CAST({col} AS VARCHAR), {}) > 0",
                            quote_string(needle)
                        )
                    })
                    .collect();
                parts.push(format!("({})", ors.join(" OR ")));
            }
        } else if let Some(rest) = expected.strip_prefix("contains:") {
            parts.push(format!(
                "strpos(CAST({col} AS VARCHAR), {}) > 0",
                quote_string(rest)
            ));
        } else {
            parts.push(format!(
                "CAST({col} AS VARCHAR) = {}",
                quote_string(expected)
            ));
        }
    }
    if let Some(keyword) = search.map(str::trim).filter(|s| !s.is_empty()) {
        let like = quote_string(&format!("%{}%", keyword.to_lowercase()));
        let mut ors = Vec::new();
        for col_name in columns {
            let col = quote_ident(col_name)?;
            ors.push(format!("lower(CAST({col} AS VARCHAR)) LIKE {like}"));
        }
        if !ors.is_empty() {
            parts.push(format!("({})", ors.join(" OR ")));
        }
    }
    if parts.is_empty() {
        Ok(String::new())
    } else {
        Ok(format!(" WHERE {}", parts.join(" AND ")))
    }
}
