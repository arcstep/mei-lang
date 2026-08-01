use std::path::Path;

use anyhow::{Context, Result};
use datafusion::prelude::SessionContext;
use serde_json::Value;

use super::super::arrow_json::batches_to_json_rows;
use super::super::connection::{block_on, with_app_session};
use super::super::sql::quote_ident;
use super::lower::SqlPlan;
use super::MAX_PIPELINE_SQL_ROWS;

pub const MAX_COLUMN_FACET_VALUES: usize = 256;
/// Hard cap on how many facet columns one query may compute (each is a full scan/group).
pub const MAX_FACET_COLUMNS_PER_QUERY: usize = 8;

#[derive(Debug, Clone)]
pub struct SqlPlanPage {
    pub total: usize,
    pub rows: Vec<Value>,
    pub has_more: bool,
}

pub fn execute_sql_plan(app_root: &Path, plan: &SqlPlan) -> Result<Vec<Value>> {
    let inner = plan.final_sql.trim_end_matches(';');
    // WITH ... SELECT must stay at statement head; do not wrap in SELECT * FROM (...).
    // composition top_n already emits `... LIMIT n` — do not append a second LIMIT
    // (DataFusion parser: "Expected end of statement, found: LIMIT").
    let sql = if sql_has_trailing_limit(inner) {
        inner.to_string()
    } else if inner.trim_start().len() >= 4
        && inner.trim_start()[..4].eq_ignore_ascii_case("with")
    {
        format!(
            "{inner} LIMIT {}",
            MAX_PIPELINE_SQL_ROWS.saturating_add(1)
        )
    } else {
        format!(
            "SELECT * FROM ({inner}) AS mei_pipeline_result LIMIT {}",
            MAX_PIPELINE_SQL_ROWS.saturating_add(1)
        )
    };
    // Setup + collect under one session lock so concurrent plans cannot interleave
    // catalog mutations between DDL and query.
    with_app_session(app_root, |ctx| {
        run_plan_setup_on_ctx(ctx, plan)?;
        block_on(async {
            let batches = ctx
                .sql(&sql)
                .await
                .with_context(|| {
                    format!(
                        "prepare pipeline sql failed (sql_chars={}): {}",
                        sql.chars().count(),
                        sql.chars().take(240).collect::<String>()
                    )
                })?
                .collect()
                .await
                .with_context(|| {
                    format!(
                        "collect pipeline sql failed (sql_chars={})",
                        sql.chars().count()
                    )
                })?;
            let mut rows = batches_to_json_rows(&batches)?;
            if !plan.result_columns.is_empty() {
                rows = rows
                    .into_iter()
                    .map(|row| project_columns(row, &plan.result_columns))
                    .collect();
            }
            Ok(rows)
        })
    })
}

fn sql_has_trailing_limit(sql: &str) -> bool {
    let trimmed = sql.trim().trim_end_matches(';').trim_end();
    let bytes = trimmed.as_bytes();
    // Match `(?i)\bLIMIT\s+\d+\s*$` without pulling in regex.
    let lower = trimmed.to_ascii_lowercase();
    let Some(idx) = lower.rfind("limit") else {
        return false;
    };
    if idx > 0 {
        let prev = bytes[idx - 1];
        if prev.is_ascii_alphanumeric() || prev == b'_' {
            return false;
        }
    }
    let after = lower[idx + 5..].trim_start();
    !after.is_empty() && after.bytes().all(|b| b.is_ascii_digit())
}

/// Paginated pipeline SQL: COUNT(*) + LIMIT/OFFSET over lowered plan (0528).
/// Does **not** apply `MAX_PIPELINE_SQL_ROWS` fail-fast — page size is the cap.
pub fn execute_sql_plan_page(
    app_root: &Path,
    plan: &SqlPlan,
    where_sql: &str,
    order_by_sql: &str,
    page: usize,
    page_size: usize,
) -> Result<SqlPlanPage> {
    let inner = plan.final_sql.trim_end_matches(';');
    let page = page.max(1);
    let page_size = page_size.max(1);
    let offset = page.saturating_sub(1).saturating_mul(page_size);
    let count_sql = format!(
        "SELECT CAST(COUNT(*) AS BIGINT) AS c FROM ({inner}) AS mei_pipeline_count{where_sql}"
    );
    let limit = page_size.saturating_add(1);
    let page_sql = format!(
        "SELECT * FROM ({inner}) AS mei_pipeline_page{where_sql}{order_by_sql} LIMIT {limit} OFFSET {offset}"
    );
    let (total, mut rows) = with_app_session(app_root, |ctx| {
        run_plan_setup_on_ctx(ctx, plan)?;
        let total = block_on(async {
            let batches = ctx
                .sql(&count_sql)
                .await
                .with_context(|| format!("prepare pipeline count sql: {count_sql}"))?
                .collect()
                .await
                .with_context(|| format!("collect pipeline count sql: {count_sql}"))?;
            Ok::<i64, anyhow::Error>(first_i64(&batches).unwrap_or(0))
        })?
        .max(0) as usize;
        let rows = block_on(async {
            let batches = ctx
                .sql(&page_sql)
                .await
                .with_context(|| format!("prepare pipeline page sql: {page_sql}"))?
                .collect()
                .await
                .with_context(|| format!("collect pipeline page sql: {page_sql}"))?;
            batches_to_json_rows(&batches)
        })?;
        Ok((total, rows))
    })?;
    if !plan.result_columns.is_empty() {
        rows = rows
            .into_iter()
            .map(|row| project_columns(row, &plan.result_columns))
            .collect();
    }
    let taken = rows.len().min(page_size);
    let has_more = rows.len() > page_size || (offset + taken) < total;
    if rows.len() > page_size {
        rows.truncate(page_size);
    }
    Ok(SqlPlanPage {
        total,
        rows,
        has_more,
    })
}

/// Facet buckets over lowered plan: `GROUP BY` + `COUNT(*)`, ordered by count desc.
/// Returns at most `limit` values (top-N by frequency) to bound payload/CPU.
pub fn execute_sql_plan_facets(
    app_root: &Path,
    plan: &SqlPlan,
    column: &str,
    where_sql: &str,
    limit: usize,
) -> Result<Vec<crate::types::TableColumnFacet>> {
    let inner = plan.final_sql.trim_end_matches(';');
    let col = quote_ident(column)?;
    let col_expr = format!("TRIM(CAST({col} AS VARCHAR))");
    let facet_where = if where_sql.is_empty() {
        format!(" WHERE {col_expr} IS NOT NULL AND {col_expr} <> ''")
    } else {
        format!("{where_sql} AND {col_expr} IS NOT NULL AND {col_expr} <> ''")
    };
    let limit = limit.max(1).min(MAX_COLUMN_FACET_VALUES);
    let sql = format!(
        "SELECT {col_expr} AS v, CAST(COUNT(*) AS BIGINT) AS c \
         FROM ({inner}) AS mei_pipeline_facet{facet_where} \
         GROUP BY {col_expr} \
         ORDER BY c DESC, v ASC \
         LIMIT {limit}"
    );
    let rows = with_app_session(app_root, |ctx| {
        run_plan_setup_on_ctx(ctx, plan)?;
        block_on(async {
            let batches = ctx
                .sql(&sql)
                .await
                .with_context(|| format!("prepare pipeline facet sql: {sql}"))?
                .collect()
                .await
                .with_context(|| format!("collect pipeline facet sql: {sql}"))?;
            batches_to_json_rows(&batches)
        })
    })?;
    Ok(parse_facet_rows(rows))
}

fn parse_facet_rows(rows: Vec<Value>) -> Vec<crate::types::TableColumnFacet> {
    let mut out = Vec::new();
    for row in rows {
        let text = row
            .get("v")
            .and_then(|value| match value {
                Value::String(s) => Some(s.trim().to_string()),
                Value::Number(n) => Some(n.to_string()),
                Value::Bool(b) => Some(b.to_string()),
                _ => None,
            })
            .filter(|s| !s.is_empty());
        let Some(text) = text else {
            continue;
        };
        let count = row
            .get("c")
            .and_then(|value| match value {
                Value::Number(n) => n.as_u64().or_else(|| n.as_i64().map(|v| v.max(0) as u64)),
                Value::String(s) => s.trim().parse::<u64>().ok(),
                _ => None,
            })
            .unwrap_or(0);
        out.push(crate::types::TableColumnFacet {
            value: text,
            count,
        });
    }
    out
}

fn run_plan_setup_on_ctx(ctx: &SessionContext, plan: &SqlPlan) -> Result<()> {
    for ddl in &plan.setup_ddls {
        block_on(async {
            let _ = ctx
                .sql(ddl)
                .await
                .with_context(|| format!("pipeline sql setup: {ddl}"))?
                .collect()
                .await
                .with_context(|| format!("collect pipeline setup: {ddl}"))?;
            Ok::<(), anyhow::Error>(())
        })?;
    }
    Ok(())
}

fn first_i64(batches: &[datafusion::arrow::record_batch::RecordBatch]) -> Option<i64> {
    let batch = batches.first()?;
    if batch.num_rows() == 0 || batch.num_columns() == 0 {
        return None;
    }
    use datafusion::arrow::array::{Array, Float64Array, Int32Array, Int64Array};
    let col = batch.column(0);
    if let Some(arr) = col.as_any().downcast_ref::<Int64Array>() {
        if arr.is_null(0) {
            return None;
        }
        return Some(arr.value(0));
    }
    if let Some(arr) = col.as_any().downcast_ref::<Int32Array>() {
        if arr.is_null(0) {
            return None;
        }
        return Some(i64::from(arr.value(0)));
    }
    if let Some(arr) = col.as_any().downcast_ref::<Float64Array>() {
        if arr.is_null(0) {
            return None;
        }
        return Some(arr.value(0) as i64);
    }
    None
}

/// Count rows for a lowered plan without materializing the full rowset.
pub fn execute_sql_plan_count(app_root: &Path, plan: &SqlPlan) -> Result<i64> {
    execute_sql_plan_scalar_f64(
        app_root,
        plan,
        "SELECT CAST(COUNT(*) AS DOUBLE) AS c FROM ({inner}) AS mei_pipeline_count",
    )
    .map(|n| n as i64)
}

/// Aggregate a numeric column over a lowered plan (`SUM`/`AVG`/`MIN`/`MAX`).
pub fn execute_sql_plan_agg_f64(
    app_root: &Path,
    plan: &SqlPlan,
    field: &str,
    agg: &str,
) -> Result<f64> {
    let col = quote_ident(field)?;
    let agg = match agg {
        "SUM" | "AVG" | "MIN" | "MAX" => agg,
        _ => anyhow::bail!("unsupported pipeline agg {agg}"),
    };
    let template = format!(
        "SELECT CAST(COALESCE({agg}(try_cast({col} AS DOUBLE)), 0) AS DOUBLE) AS c \
         FROM ({{inner}}) AS mei_pipeline_agg"
    );
    execute_sql_plan_scalar_f64(app_root, plan, &template)
}

fn execute_sql_plan_scalar_f64(
    app_root: &Path,
    plan: &SqlPlan,
    sql_template: &str,
) -> Result<f64> {
    let inner = plan.final_sql.trim_end_matches(';');
    let sql = sql_template.replace("{inner}", inner);
    with_app_session(app_root, |ctx| {
        run_plan_setup_on_ctx(ctx, plan)?;
        block_on(async {
            let batches = ctx
                .sql(&sql)
                .await
                .with_context(|| format!("prepare pipeline scalar sql: {sql}"))?
                .collect()
                .await
                .with_context(|| format!("collect pipeline scalar sql: {sql}"))?;
            let Some(batch) = batches.first() else {
                return Ok(0.0);
            };
            if batch.num_rows() == 0 || batch.num_columns() == 0 {
                return Ok(0.0);
            }
            use datafusion::arrow::array::{Array, Float64Array, Int64Array};
            let col = batch.column(0);
            if let Some(arr) = col.as_any().downcast_ref::<Float64Array>() {
                if arr.is_null(0) {
                    return Ok(0.0);
                }
                return Ok(arr.value(0));
            }
            if let Some(arr) = col.as_any().downcast_ref::<Int64Array>() {
                if arr.is_null(0) {
                    return Ok(0.0);
                }
                return Ok(arr.value(0) as f64);
            }
            if let Some(arr) = col
                .as_any()
                .downcast_ref::<datafusion::arrow::array::Int32Array>()
            {
                if arr.is_null(0) {
                    return Ok(0.0);
                }
                return Ok(f64::from(arr.value(0)));
            }
            Ok(0.0)
        })
    })
}

fn project_columns(row: Value, columns: &[String]) -> Value {
    let Value::Object(map) = row else {
        return row;
    };
    let mut out = serde_json::Map::new();
    for name in columns {
        out.insert(name.clone(), map.get(name).cloned().unwrap_or(Value::Null));
    }
    Value::Object(out)
}
