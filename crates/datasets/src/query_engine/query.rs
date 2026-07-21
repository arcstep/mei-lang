use std::collections::BTreeMap;
use std::path::Path;
use std::time::Instant;

use anyhow::{Context, Result};
use mei_lang_kernel::ColumnSchema;
use serde_json::{Map, Value};

use super::arrow_json::{batches_to_json_rows, first_scalar_f64, first_scalar_i64};
use super::connection::{block_on, with_app_session};
use super::register::ensure_parquet_view;
use super::sql::{build_where_clause, quote_ident};
use super::{record_query_engine_ms, record_rows_materialized};
use crate::types::DatasetQueryOptions;
use crate::util::elapsed_ms;

#[derive(Debug, Clone)]
pub struct ParquetPageQuery<'a> {
    pub parquet_path: &'a Path,
    pub schema: &'a [ColumnSchema],
    pub physical_columns: Option<&'a [String]>,
    pub normalize: &'a BTreeMap<String, String>,
    pub options: &'a DatasetQueryOptions,
}

#[derive(Debug, Clone)]
pub struct ParquetPageResult {
    pub page: usize,
    pub page_size: usize,
    pub total: usize,
    pub has_more: bool,
    pub columns: Vec<String>,
    pub rows: Vec<Value>,
    pub query_engine_ms: u64,
    pub rows_materialized: usize,
    pub column_facets: BTreeMap<String, Vec<crate::types::TableColumnFacet>>,
}

pub fn query_parquet_page(app_root: &Path, req: ParquetPageQuery<'_>) -> Result<ParquetPageResult> {
    let started = Instant::now();
    let (view, columns) =
        ensure_parquet_view(app_root, req.parquet_path, req.schema, req.physical_columns)?;
    let logical_columns = apply_normalize_columns(&columns, req.normalize);
    let where_sql = build_where_clause(
        &req.options.filters,
        req.options.search.as_deref(),
        &logical_columns,
    )?;
    let view_ident = quote_ident(&view)?;
    let columns_for_select = columns.clone();

    let collect_all = req.options.collect_all;
    let page = if collect_all {
        1
    } else {
        req.options.page.max(1)
    };
    let page_size = if collect_all {
        0
    } else {
        req.options.page_size.max(1)
    };
    let offset = if collect_all {
        0
    } else {
        page.saturating_sub(1).saturating_mul(page_size)
    };

    let (total, rows) = with_app_session(app_root, |ctx| {
        let count_sql = format!("SELECT COUNT(*) FROM {view_ident}{where_sql}");
        let total = block_on(async {
            let batches = ctx
                .sql(&count_sql)
                .await
                .with_context(|| format!("query engine count: {count_sql}"))?
                .collect()
                .await
                .with_context(|| format!("collect count: {count_sql}"))?;
            first_scalar_i64(&batches)
        })?;

        let limit_sql = if collect_all {
            String::new()
        } else {
            format!(" LIMIT {} OFFSET {offset}", page_size.saturating_add(1))
        };
        let select_list = columns_for_select
            .iter()
            .map(|c| quote_ident(c))
            .collect::<Result<Vec<_>>>()?
            .join(", ");
        let select_sql = if select_list.is_empty() {
            format!("SELECT * FROM {view_ident}{where_sql}{limit_sql}")
        } else {
            format!("SELECT {select_list} FROM {view_ident}{where_sql}{limit_sql}")
        };
        let batches = block_on(async {
            ctx.sql(&select_sql)
                .await
                .with_context(|| format!("prepare page query: {select_sql}"))?
                .collect()
                .await
                .with_context(|| format!("collect page query: {select_sql}"))
        })?;
        let mut rows = batches_to_json_rows(&batches)?;
        if !req.normalize.is_empty() {
            rows = rows
                .into_iter()
                .map(|row| rename_row(row, req.normalize))
                .collect();
        }
        Ok((total.max(0) as usize, rows))
    })?;

    let mut has_more = false;
    let page_rows = if collect_all {
        rows
    } else if rows.len() > page_size {
        has_more = true;
        rows.into_iter().take(page_size).collect()
    } else {
        rows
    };
    let mut column_facets = BTreeMap::new();
    for column in req
        .options
        .facet_columns
        .iter()
        .take(super::pipeline_sql::MAX_FACET_COLUMNS_PER_QUERY)
    {
        let name = column.trim();
        if name.is_empty() {
            continue;
        }
        match query_parquet_facets(
            app_root,
            &view_ident,
            name,
            &where_sql,
            super::pipeline_sql::MAX_COLUMN_FACET_VALUES,
        ) {
            Ok(values) => {
                column_facets.insert(name.to_string(), values);
            }
            Err(err) => {
                tracing::debug!(column = %name, error = %err, "parquet facet group skipped");
            }
        }
    }
    let materialized = page_rows.len();
    let ms = elapsed_ms(started);
    record_query_engine_ms(ms);
    record_rows_materialized(materialized);

    Ok(ParquetPageResult {
        page,
        page_size: if collect_all { materialized } else { page_size },
        total,
        has_more: if collect_all {
            false
        } else {
            has_more || (offset + materialized) < total
        },
        columns: logical_columns,
        rows: page_rows,
        query_engine_ms: ms,
        rows_materialized: materialized,
        column_facets,
    })
}

fn query_parquet_facets(
    app_root: &Path,
    view_ident: &str,
    column: &str,
    where_sql: &str,
    limit: usize,
) -> Result<Vec<crate::types::TableColumnFacet>> {
    let col = quote_ident(column)?;
    let col_expr = format!("TRIM(CAST({col} AS VARCHAR))");
    let facet_where = if where_sql.is_empty() {
        format!(" WHERE {col_expr} IS NOT NULL AND {col_expr} <> ''")
    } else {
        format!("{where_sql} AND {col_expr} IS NOT NULL AND {col_expr} <> ''")
    };
    let limit = limit.max(1);
    let sql = format!(
        "SELECT {col_expr} AS v, CAST(COUNT(*) AS BIGINT) AS c \
         FROM {view_ident}{facet_where} \
         GROUP BY {col_expr} \
         ORDER BY c DESC, v ASC \
         LIMIT {limit}"
    );
    let rows = with_app_session(app_root, |ctx| {
        block_on(async {
            let batches = ctx
                .sql(&sql)
                .await
                .with_context(|| format!("prepare parquet facet sql: {sql}"))?
                .collect()
                .await
                .with_context(|| format!("collect parquet facet sql: {sql}"))?;
            batches_to_json_rows(&batches)
        })
    })?;
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
    Ok(out)
}

pub fn query_parquet_scalar_i64(
    app_root: &Path,
    parquet_path: &Path,
    schema: &[ColumnSchema],
    sql_agg: &str,
    where_sql: &str,
) -> Result<i64> {
    let started = Instant::now();
    let (view, _) = ensure_parquet_view(app_root, parquet_path, schema, None)?;
    let view_ident = quote_ident(&view)?;
    let sql = format!("SELECT ({sql_agg}) FROM {view_ident}{where_sql}");
    let value = with_app_session(app_root, |ctx| {
        block_on(async {
            let batches = ctx
                .sql(&sql)
                .await
                .with_context(|| format!("query engine scalar i64: {sql}"))?
                .collect()
                .await
                .with_context(|| format!("collect scalar i64: {sql}"))?;
            first_scalar_i64(&batches)
        })
    })?;
    record_query_engine_ms(elapsed_ms(started));
    record_rows_materialized(0);
    Ok(value)
}

pub fn query_parquet_scalar_f64(
    app_root: &Path,
    parquet_path: &Path,
    schema: &[ColumnSchema],
    sql_agg: &str,
    where_sql: &str,
) -> Result<f64> {
    let started = Instant::now();
    let (view, _) = ensure_parquet_view(app_root, parquet_path, schema, None)?;
    let view_ident = quote_ident(&view)?;
    let sql = format!("SELECT ({sql_agg}) FROM {view_ident}{where_sql}");
    let value = with_app_session(app_root, |ctx| {
        block_on(async {
            let batches = ctx
                .sql(&sql)
                .await
                .with_context(|| format!("query engine scalar f64: {sql}"))?
                .collect()
                .await
                .with_context(|| format!("collect scalar f64: {sql}"))?;
            first_scalar_f64(&batches)
        })
    })?;
    record_query_engine_ms(elapsed_ms(started));
    record_rows_materialized(0);
    Ok(value)
}

fn apply_normalize_columns(
    columns: &[String],
    normalize: &BTreeMap<String, String>,
) -> Vec<String> {
    if normalize.is_empty() {
        return columns.to_vec();
    }
    columns
        .iter()
        .map(|c| normalize.get(c).cloned().unwrap_or_else(|| c.clone()))
        .collect()
}

fn rename_row(row: Value, normalize: &BTreeMap<String, String>) -> Value {
    let Value::Object(map) = row else {
        return row;
    };
    let mut out = Map::new();
    for (k, v) in map {
        let name = normalize.get(&k).cloned().unwrap_or(k);
        out.insert(name, v);
    }
    Value::Object(out)
}
