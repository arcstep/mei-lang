use std::collections::BTreeMap;

use anyhow::{bail, Result};

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
            let (lower, upper) = match rest.split_once("..") {
                Some((l, u)) => (l.trim(), u.trim()),
                None => continue,
            };
            if lower.is_empty() || upper.is_empty() {
                continue;
            }
            let date_expr = sql_parse_date_expr(key)?;
            parts.push(format!(
                "{date_expr} BETWEEN CAST({} AS DATE) AND CAST({} AS DATE)",
                quote_string(lower),
                quote_string(upper)
            ));
        } else if let Some(rest) = expected.strip_prefix("in:") {
            let values: Vec<String> = rest
                .split(',')
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(quote_string)
                .collect();
            if !values.is_empty() {
                parts.push(format!(
                    "CAST({col} AS VARCHAR) IN ({})",
                    values.join(", ")
                ));
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
