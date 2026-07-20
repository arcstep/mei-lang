//! DuckDB date expression helpers (Excel serial + ISO strings).

use anyhow::Result;

use super::super::sql::quote_ident;

/// Parse a column that may be ISO date text or Excel serial day into DATE.
pub fn sql_parse_date_expr(column: &str) -> Result<String> {
    let col = quote_ident(column)?;
    Ok(format!(
        "COALESCE(\
           TRY_CAST({col} AS DATE), \
           CASE \
             WHEN TRY_CAST({col} AS DOUBLE) IS NOT NULL \
              AND TRY_CAST({col} AS DOUBLE) > 0 \
              AND TRY_CAST({col} AS DOUBLE) < 2958465 \
             THEN DATE '1899-12-30' + CAST(FLOOR(TRY_CAST({col} AS DOUBLE)) AS INTEGER) \
             ELSE NULL \
           END\
         )"
    ))
}
