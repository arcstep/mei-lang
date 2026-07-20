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

pub fn quote_path(path: &str) -> String {
    quote_string(path)
}

/// Map MeiLang column type_name to DuckDB cast type.
pub fn duck_cast_type(type_name: &str) -> &'static str {
    match type_name.trim().to_ascii_lowercase().as_str() {
        "int" | "integer" | "i64" | "i32" | "long" => "BIGINT",
        "float" | "double" | "number" | "f64" | "f32" => "DOUBLE",
        "bool" | "boolean" => "BOOLEAN",
        "date" => "DATE",
        "datetime" | "timestamp" => "TIMESTAMP",
        _ => "VARCHAR",
    }
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
        // Keep parity with simple equality / text contains used by most cockpit filters.
        // Advanced filter specs (gte:/in:) fall back to CAST-as-text equality / LIKE.
        if let Some(rest) = expected.strip_prefix("gte:") {
            parts.push(format!(
                "TRY_CAST({col} AS DOUBLE) >= {}",
                rest.trim().parse::<f64>().unwrap_or(0.0)
            ));
        } else if let Some(rest) = expected.strip_prefix("lte:") {
            parts.push(format!(
                "TRY_CAST({col} AS DOUBLE) <= {}",
                rest.trim().parse::<f64>().unwrap_or(0.0)
            ));
        } else if let Some(rest) = expected.strip_prefix("gt:") {
            parts.push(format!(
                "TRY_CAST({col} AS DOUBLE) > {}",
                rest.trim().parse::<f64>().unwrap_or(0.0)
            ));
        } else if let Some(rest) = expected.strip_prefix("lt:") {
            parts.push(format!(
                "TRY_CAST({col} AS DOUBLE) < {}",
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
            let date_expr = format!(
                "COALESCE(TRY_CAST({col} AS DATE), \
                 CASE WHEN TRY_CAST({col} AS DOUBLE) IS NOT NULL \
                   AND TRY_CAST({col} AS DOUBLE) > 0 \
                   AND TRY_CAST({col} AS DOUBLE) < 2958465 \
                 THEN DATE '1899-12-30' + CAST(FLOOR(TRY_CAST({col} AS DOUBLE)) AS INTEGER) \
                 ELSE NULL END)"
            );
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
