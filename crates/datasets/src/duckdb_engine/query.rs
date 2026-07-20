use std::collections::BTreeMap;
use std::path::Path;
use std::time::Instant;

use anyhow::{Context, Result};
use mei_lang_kernel::ColumnSchema;
use serde_json::{json, Map, Value};

use super::connection::with_app_connection;
use super::register::ensure_parquet_view;
use super::sql::{build_where_clause, quote_ident};
use super::{record_duckdb_query_ms, record_rows_materialized};
use crate::types::DatasetQueryOptions;
use crate::util::elapsed_ms;

#[derive(Debug, Clone)]
pub struct DuckdbPageQuery<'a> {
    pub parquet_path: &'a Path,
    pub schema: &'a [ColumnSchema],
    pub physical_columns: Option<&'a [String]>,
    pub normalize: &'a BTreeMap<String, String>,
    pub options: &'a DatasetQueryOptions,
}

#[derive(Debug, Clone)]
pub struct DuckdbPageResult {
    pub page: usize,
    pub page_size: usize,
    pub total: usize,
    pub has_more: bool,
    pub columns: Vec<String>,
    pub rows: Vec<Value>,
    pub duckdb_query_ms: u64,
    pub rows_materialized: usize,
}

pub fn query_parquet_page(app_root: &Path, req: DuckdbPageQuery<'_>) -> Result<DuckdbPageResult> {
    let started = Instant::now();
    let (view, columns) = ensure_parquet_view(
        app_root,
        req.parquet_path,
        req.schema,
        req.physical_columns,
    )?;
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

    let (total, rows) = with_app_connection(app_root, |conn| {
        let count_sql = format!("SELECT COUNT(*) FROM {view_ident}{where_sql}");
        let total: i64 = conn
            .query_row(&count_sql, [], |row| row.get(0))
            .with_context(|| format!("duckdb count: {count_sql}"))?;

        let limit_sql = if collect_all {
            String::new()
        } else {
            // Fetch one extra row to compute has_more without a second COUNT round-trip on page.
            format!(" LIMIT {} OFFSET {offset}", page_size.saturating_add(1))
        };
        // Project view columns by name (avoid Statement::column_name before execute).
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
        let mut stmt = conn
            .prepare(&select_sql)
            .with_context(|| format!("prepare page query: {select_sql}"))?;
        let col_names = columns_for_select;
        let mut rows = Vec::new();
        let mut rows_iter = stmt.query([])?;
        while let Some(row) = rows_iter.next()? {
            let mut map = Map::new();
            for (idx, name) in col_names.iter().enumerate() {
                let value = duck_value_to_json(row, idx)?;
                let out_name = req
                    .normalize
                    .get(name)
                    .cloned()
                    .unwrap_or_else(|| name.clone());
                map.insert(out_name, value);
            }
            rows.push(Value::Object(map));
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
    let materialized = page_rows.len();
    let ms = elapsed_ms(started);
    record_duckdb_query_ms(ms);
    record_rows_materialized(materialized);

    Ok(DuckdbPageResult {
        page,
        page_size: if collect_all {
            materialized
        } else {
            page_size
        },
        total,
        has_more: if collect_all {
            false
        } else {
            has_more || (offset + materialized) < total
        },
        columns: logical_columns,
        rows: page_rows,
        duckdb_query_ms: ms,
        rows_materialized: materialized,
    })
}

pub fn query_parquet_scalar_i64(
    app_root: &Path,
    parquet_path: &Path,
    schema: &[ColumnSchema],
    sql_agg: &str,
) -> Result<i64> {
    let started = Instant::now();
    let (view, _) = ensure_parquet_view(app_root, parquet_path, schema, None)?;
    let view_ident = quote_ident(&view)?;
    let sql = format!("SELECT ({sql_agg}) FROM {view_ident}");
    let value = with_app_connection(app_root, |conn| {
        conn.query_row(&sql, [], |row| row.get::<_, i64>(0))
            .with_context(|| format!("duckdb scalar i64: {sql}"))
    })?;
    record_duckdb_query_ms(elapsed_ms(started));
    record_rows_materialized(0);
    Ok(value)
}

pub fn query_parquet_scalar_f64(
    app_root: &Path,
    parquet_path: &Path,
    schema: &[ColumnSchema],
    sql_agg: &str,
) -> Result<f64> {
    let started = Instant::now();
    let (view, _) = ensure_parquet_view(app_root, parquet_path, schema, None)?;
    let view_ident = quote_ident(&view)?;
    let sql = format!("SELECT ({sql_agg}) FROM {view_ident}");
    let value = with_app_connection(app_root, |conn| {
        conn.query_row(&sql, [], |row| row.get::<_, f64>(0))
            .with_context(|| format!("duckdb scalar f64: {sql}"))
    })?;
    record_duckdb_query_ms(elapsed_ms(started));
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

fn duck_value_to_json(row: &duckdb::Row<'_>, idx: usize) -> Result<Value> {
    // Prefer text then numeric then null — keeps JSON shape stable for UI.
    if let Ok(v) = row.get::<_, Option<String>>(idx) {
        return Ok(match v {
            Some(s) => Value::String(s),
            None => Value::Null,
        });
    }
    if let Ok(v) = row.get::<_, Option<i64>>(idx) {
        return Ok(match v {
            Some(n) => json!(n),
            None => Value::Null,
        });
    }
    if let Ok(v) = row.get::<_, Option<f64>>(idx) {
        return Ok(match v {
            Some(n) => json!(n),
            None => Value::Null,
        });
    }
    if let Ok(v) = row.get::<_, Option<bool>>(idx) {
        return Ok(match v {
            Some(b) => Value::Bool(b),
            None => Value::Null,
        });
    }
    Ok(Value::Null)
}
