//! Lower analysis_expr → SqlPlan (CTE chain).

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{bail, Result};
use mei_lang_kernel::DatasetView;
use serde_json::Value;

use super::super::connection::{block_on, with_app_session};
use super::super::register::{ensure_parquet_view, resolve_parquet_for_dataset_view};
use super::super::sql::{build_where_clause, quote_ident, quote_string};
use super::date_sql::sql_parse_date_expr;
use super::MAX_PIPELINE_SQL_ROWS;

#[derive(Debug, Clone)]
pub struct SqlPlan {
    pub setup_ddls: Vec<String>,
    pub final_sql: String,
    pub result_columns: Vec<String>,
}

#[derive(Debug, Clone)]
struct Rel {
    /// SQL subquery or view reference producing rows.
    sql: String,
    /// Known output column names (best-effort).
    columns: Vec<String>,
}

fn probe_ending_month(app_root: &Path, inner_sql: &str, date_expr: &str) -> Result<Option<u32>> {
    let sql = format!(
        "SELECT CAST(date_part('month', max({date_expr})) AS INT) FROM ({inner_sql}) AS _probe WHERE {date_expr} IS NOT NULL"
    );
    with_app_session(app_root, |ctx| {
        block_on(async {
            let batches = ctx.sql(&sql).await?.collect().await?;
            let Some(batch) = batches.first() else {
                return Ok(None);
            };
            if batch.num_rows() == 0 || batch.num_columns() == 0 {
                return Ok(None);
            }
            use datafusion::arrow::array::{Array, Int64Array};
            let col = batch.column(0);
            if let Some(arr) = col.as_any().downcast_ref::<Int64Array>() {
                if arr.is_null(0) {
                    return Ok(None);
                }
                return Ok(Some(arr.value(0).clamp(1, 12) as u32));
            }
            if let Some(arr) = col
                .as_any()
                .downcast_ref::<datafusion::arrow::array::Int32Array>()
            {
                if arr.is_null(0) {
                    return Ok(None);
                }
                return Ok(Some(arr.value(0).clamp(1, 12) as u32));
            }
            Ok(None)
        })
    })
}

pub fn try_lower_expr(
    app_root: &Path,
    datasets: &BTreeMap<String, DatasetView>,
    expr: &Value,
) -> Result<Option<SqlPlan>> {
    let mut setup = Vec::new();
    let Some(rel) = lower_rel(app_root, datasets, expr, &mut setup, 0)? else {
        return Ok(None);
    };
    let mut setup_ddls = Vec::new();
    let mut ctes = Vec::new();
    for item in setup {
        if let Some(rest) = item.strip_prefix("CTE:") {
            if let Some((name, sql)) = rest.split_once("|||") {
                if !ctes.iter().any(|(n, _)| n == name) {
                    ctes.push((name.to_string(), sql.to_string()));
                }
                continue;
            }
        }
        setup_ddls.push(item);
    }
    let final_sql = if ctes.is_empty() {
        rel.sql
    } else {
        let with = ctes
            .iter()
            .map(|(name, sql)| format!("{name} AS ({sql})"))
            .collect::<Vec<_>>()
            .join(", ");
        format!("WITH {with} {}", rel.sql)
    };
    Ok(Some(SqlPlan {
        setup_ddls,
        final_sql,
        result_columns: rel.columns,
    }))
}

fn lower_rel(
    app_root: &Path,
    datasets: &BTreeMap<String, DatasetView>,
    expr: &Value,
    setup: &mut Vec<String>,
    depth: usize,
) -> Result<Option<Rel>> {
    if depth > 32 {
        return Ok(None);
    }
    let Some(object) = expr.as_object() else {
        return Ok(None);
    };
    // Runtime metric defs often keep dataset bindings as `__ref: data`
    // instead of fully lowered `analysis_expr/rows`.
    if object.get("__ref").and_then(Value::as_str) == Some("data") {
        return lower_data_ref(app_root, datasets, object, setup);
    }
    if object.get("__kind").and_then(Value::as_str) != Some("analysis_expr") {
        return Ok(None);
    }
    let analysis_type = object.get("type").and_then(Value::as_str).unwrap_or("");
    match analysis_type {
        "rows" => lower_rows(app_root, datasets, object, setup),
        "where" => lower_where(app_root, datasets, object, setup, depth),
        "latest_days" => lower_latest_days(app_root, datasets, object, setup, depth),
        "latest_months" => lower_latest_months(app_root, datasets, object, setup, depth),
        "sort_by" => lower_sort_by(app_root, datasets, object, setup, depth),
        "limit" => lower_limit(app_root, datasets, object, setup, depth),
        "select" => lower_select(app_root, datasets, object, setup, depth),
        "rename" => lower_rename(app_root, datasets, object, setup, depth),
        "trend_year_compare" => lower_trend_year_compare(app_root, datasets, object, setup, depth),
        "party_year_aggregate" => {
            lower_party_year_aggregate(app_root, datasets, object, setup, depth)
        }
        "unpivot_columns" => lower_unpivot_columns(app_root, datasets, object, setup, depth),
        "lookup_value" => lower_lookup_value(app_root, datasets, object, setup, depth),
        "group_by" => lower_group_by(app_root, datasets, object, setup, depth),
        "bucket_date" => lower_bucket_date(app_root, datasets, object, setup, depth),
        "first_by" => lower_first_by(app_root, datasets, object, setup, depth),
        "mutate" => lower_mutate(app_root, datasets, object, setup, depth),
        "concat_rowsets" => lower_concat_rowsets(app_root, datasets, object, setup, depth),
        "split_text" => lower_split_text(app_root, datasets, object, setup, depth),
        "sql" => lower_raw_sql(object),
        _ => Ok(None),
    }
}

fn lower_data_ref(
    app_root: &Path,
    datasets: &BTreeMap<String, DatasetView>,
    object: &serde_json::Map<String, Value>,
    setup: &mut Vec<String>,
) -> Result<Option<Rel>> {
    let dataset_id = object
        .get("from_dataset")
        .or_else(|| object.get("id"))
        .or_else(|| object.get("dataset"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.strip_prefix("dataset.").unwrap_or(s).to_string());
    let Some(dataset_id) = dataset_id else {
        return Ok(None);
    };
    let mut rows_obj = serde_json::Map::new();
    rows_obj.insert("dataset".to_string(), Value::String(dataset_id));
    lower_rows(app_root, datasets, &rows_obj, setup)
}

fn lower_rows(
    app_root: &Path,
    datasets: &BTreeMap<String, DatasetView>,
    object: &serde_json::Map<String, Value>,
    setup: &mut Vec<String>,
) -> Result<Option<Rel>> {
    let dataset_id = object
        .get("dataset")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.strip_prefix("dataset.").unwrap_or(s).to_string());
    let Some(dataset_id) = dataset_id else {
        return Ok(None);
    };
    let Some(view) = lookup_dataset(datasets, &dataset_id) else {
        return Ok(None);
    };
    let Some((view_name, columns)) = ensure_dataset_view(app_root, view, setup)? else {
        return Ok(None);
    };
    let view_ident = quote_ident(&view_name)?;
    Ok(Some(Rel {
        sql: format!("SELECT * FROM {view_ident}"),
        columns,
    }))
}

fn ensure_dataset_view(
    app_root: &Path,
    view: &DatasetView,
    _setup: &mut Vec<String>,
) -> Result<Option<(String, Vec<String>)>> {
    // Tabular parquet snapshots, or GeoJSON attribute parquet (properties only).
    let Some(parquet) = resolve_parquet_for_dataset_view(app_root, view)? else {
        return Ok(None);
    };
    let (name, columns) = ensure_parquet_view(app_root, parquet.as_path(), &view.schema, None)?;
    let columns = if columns.is_empty() {
        view.columns.clone()
    } else {
        columns
    };
    Ok(Some((name, columns)))
}

fn lower_where(
    app_root: &Path,
    datasets: &BTreeMap<String, DatasetView>,
    object: &serde_json::Map<String, Value>,
    setup: &mut Vec<String>,
    depth: usize,
) -> Result<Option<Rel>> {
    let Some(inner_expr) = object.get("rowset") else {
        return Ok(None);
    };
    let Some(inner) = lower_rel(app_root, datasets, inner_expr, setup, depth + 1)? else {
        return Ok(None);
    };
    let Some(predicate) = object.get("predicate") else {
        return Ok(None);
    };
    let Some(pred_sql) = predicate_to_sql(predicate)? else {
        return Ok(None);
    };
    Ok(Some(Rel {
        sql: format!("SELECT * FROM ({}) AS _w WHERE {pred_sql}", inner.sql),
        columns: inner.columns,
    }))
}

/// Match kernel `filter_rows_in_latest_days`: inclusive window of `days` ending at
/// max parsed date in the rowset (not wall clock).
fn lower_latest_days(
    app_root: &Path,
    datasets: &BTreeMap<String, DatasetView>,
    object: &serde_json::Map<String, Value>,
    setup: &mut Vec<String>,
    depth: usize,
) -> Result<Option<Rel>> {
    let Some(inner_expr) = object.get("rowset") else {
        return Ok(None);
    };
    let Some(inner) = lower_rel(app_root, datasets, inner_expr, setup, depth + 1)? else {
        return Ok(None);
    };
    let field = object.get("field").and_then(Value::as_str).unwrap_or("");
    if field.is_empty() {
        return Ok(None);
    }
    let days = object
        .get("days")
        .and_then(Value::as_u64)
        .unwrap_or(7)
        .clamp(1, 366) as i64;
    let offset = days.saturating_sub(1);
    let date_expr = sql_parse_date_expr(field)?;
    let ord = |expr: &str| {
        format!(
            "(CAST(date_part('year', {expr}) AS BIGINT) * 10000 \
             + CAST(date_part('month', {expr}) AS BIGINT) * 100 \
             + CAST(date_part('day', {expr}) AS BIGINT))"
        )
    };
    // Unqualified date_expr resolves against each FROM alias scope.
    Ok(Some(Rel {
        sql: format!(
            "SELECT * FROM ({inner}) AS _ld \
             WHERE {date_expr} IS NOT NULL \
               AND {ord_ld} >= (\
                 SELECT COALESCE(MAX({ord_m}), 0) - {offset} \
                 FROM ({inner}) AS _m \
                 WHERE {date_expr} IS NOT NULL\
               )",
            inner = inner.sql,
            date_expr = date_expr,
            ord_ld = ord(&date_expr),
            ord_m = ord(&date_expr),
            offset = offset,
        ),
        columns: inner.columns,
    }))
}

/// Match kernel `filter_rows_in_latest_months`: keep rows whose (year, month)
/// falls in the last `months` calendar months ending at max row month.
fn lower_latest_months(
    app_root: &Path,
    datasets: &BTreeMap<String, DatasetView>,
    object: &serde_json::Map<String, Value>,
    setup: &mut Vec<String>,
    depth: usize,
) -> Result<Option<Rel>> {
    let Some(inner_expr) = object.get("rowset") else {
        return Ok(None);
    };
    let Some(inner) = lower_rel(app_root, datasets, inner_expr, setup, depth + 1)? else {
        return Ok(None);
    };
    let field = object.get("field").and_then(Value::as_str).unwrap_or("");
    if field.is_empty() {
        return Ok(None);
    }
    let months = object
        .get("months")
        .and_then(Value::as_u64)
        .unwrap_or(6)
        .clamp(1, 120) as i64;
    let offset = months.saturating_sub(1);
    let date_expr = sql_parse_date_expr(field)?;
    let month_idx = |expr: &str| {
        format!(
            "(CAST(date_part('year', {expr}) AS BIGINT) * 12 \
             + CAST(date_part('month', {expr}) AS BIGINT))"
        )
    };
    Ok(Some(Rel {
        sql: format!(
            "SELECT * FROM ({inner}) AS _lm \
             WHERE {date_expr} IS NOT NULL \
               AND {idx_lm} >= (\
                 SELECT COALESCE(MAX({idx_m}), 0) - {offset} \
                 FROM ({inner}) AS _m \
                 WHERE {date_expr} IS NOT NULL\
               )",
            inner = inner.sql,
            date_expr = date_expr,
            idx_lm = month_idx(&date_expr),
            idx_m = month_idx(&date_expr),
            offset = offset,
        ),
        columns: inner.columns,
    }))
}

fn predicate_to_sql(predicate: &Value) -> Result<Option<String>> {
    let Some(object) = predicate.as_object() else {
        return Ok(None);
    };
    if object.get("__kind").and_then(Value::as_str) != Some("analysis_expr") {
        return Ok(None);
    }
    match object.get("type").and_then(Value::as_str).unwrap_or("") {
        "eq" => {
            let field = object.get("field").and_then(Value::as_str).unwrap_or("");
            if field.is_empty() {
                return Ok(None);
            }
            let col = quote_ident(field)?;
            let value = literal_sql(object.get("value"))?;
            Ok(Some(format!("CAST({col} AS VARCHAR) = {value}")))
        }
        "ne" => {
            let field = object.get("field").and_then(Value::as_str).unwrap_or("");
            let col = quote_ident(field)?;
            let value = literal_sql(object.get("value"))?;
            Ok(Some(format!("CAST({col} AS VARCHAR) <> {value}")))
        }
        "gt" | "gte" | "lt" | "lte" => {
            let op = match object.get("type").and_then(Value::as_str).unwrap_or("") {
                "gt" => ">",
                "gte" => ">=",
                "lt" => "<",
                _ => "<=",
            };
            let field = object.get("field").and_then(Value::as_str).unwrap_or("");
            let col = quote_ident(field)?;
            let value = object
                .get("value")
                .and_then(|v| v.as_f64().or_else(|| v.as_i64().map(|n| n as f64)))
                .unwrap_or(0.0);
            Ok(Some(format!("try_cast({col} AS DOUBLE) {op} {value}")))
        }
        "field_gt" | "field_gte" | "field_lt" | "field_lte" => {
            let op = match object.get("type").and_then(Value::as_str).unwrap_or("") {
                "field_gt" => ">",
                "field_gte" => ">=",
                "field_lt" => "<",
                _ => "<=",
            };
            let left = object
                .get("left_field")
                .and_then(Value::as_str)
                .unwrap_or("");
            let right = object
                .get("right_field")
                .and_then(Value::as_str)
                .unwrap_or("");
            let l = quote_ident(left)?;
            let r = quote_ident(right)?;
            Ok(Some(format!(
                "try_cast({l} AS DOUBLE) {op} try_cast({r} AS DOUBLE)"
            )))
        }
        "not_empty" => {
            let field = object.get("field").and_then(Value::as_str).unwrap_or("");
            let col = quote_ident(field)?;
            Ok(Some(format!(
                "({col} IS NOT NULL AND TRIM(CAST({col} AS VARCHAR)) <> '')"
            )))
        }
        "between" => {
            let field = object.get("field").and_then(Value::as_str).unwrap_or("");
            if field.is_empty() {
                return Ok(None);
            }
            let lower = object.get("lower").and_then(Value::as_str).unwrap_or("");
            let upper = object.get("upper").and_then(Value::as_str).unwrap_or("");
            if lower.is_empty() || upper.is_empty() {
                return Ok(None);
            }
            let date_expr = sql_parse_date_expr(field)?;
            Ok(Some(format!(
                "{date_expr} BETWEEN CAST({} AS DATE) AND CAST({} AS DATE)",
                quote_string(lower),
                quote_string(upper)
            )))
        }
        "and" => {
            let Some(items) = object.get("predicates").and_then(Value::as_array) else {
                return Ok(None);
            };
            let mut parts = Vec::new();
            for item in items {
                let Some(p) = predicate_to_sql(item)? else {
                    return Ok(None);
                };
                parts.push(format!("({p})"));
            }
            if parts.is_empty() {
                return Ok(None);
            }
            Ok(Some(parts.join(" AND ")))
        }
        "or" => {
            let Some(items) = object.get("predicates").and_then(Value::as_array) else {
                return Ok(None);
            };
            let mut parts = Vec::new();
            for item in items {
                let Some(p) = predicate_to_sql(item)? else {
                    return Ok(None);
                };
                parts.push(format!("({p})"));
            }
            if parts.is_empty() {
                return Ok(None);
            }
            Ok(Some(parts.join(" OR ")))
        }
        "not" => {
            let Some(inner) = object.get("predicate") else {
                return Ok(None);
            };
            let Some(p) = predicate_to_sql(inner)? else {
                return Ok(None);
            };
            Ok(Some(format!("NOT ({p})")))
        }
        "present" => {
            let field = object.get("field").and_then(Value::as_str).unwrap_or("");
            if field.is_empty() {
                return Ok(None);
            }
            let blank = blank_field_sql(field)?;
            Ok(Some(format!("NOT ({blank})")))
        }
        "blank" => {
            let field = object.get("field").and_then(Value::as_str).unwrap_or("");
            if field.is_empty() {
                return Ok(None);
            }
            Ok(Some(blank_field_sql(field)?))
        }
        "in_values" => {
            let field = object.get("field").and_then(Value::as_str).unwrap_or("");
            if field.is_empty() {
                return Ok(None);
            }
            let Some(values) = object.get("values").and_then(Value::as_array) else {
                return Ok(None);
            };
            if values.is_empty() {
                return Ok(None);
            }
            let mut parts = Vec::with_capacity(values.len());
            for value in values {
                parts.push(literal_sql(Some(value))?);
            }
            let col = quote_ident(field)?;
            Ok(Some(format!(
                "CAST({col} AS VARCHAR) IN ({})",
                parts.join(", ")
            )))
        }
        "matches" => {
            let field = object.get("field").and_then(Value::as_str).unwrap_or("");
            let pattern = object.get("pattern").and_then(Value::as_str).unwrap_or("");
            if field.is_empty() || pattern.is_empty() {
                return Ok(None);
            }
            let col = quote_ident(field)?;
            // DataFusion: regexp_match returns List; non-null means a match.
            Ok(Some(format!(
                "regexp_match(CAST({col} AS VARCHAR), {}) IS NOT NULL",
                quote_string(pattern)
            )))
        }
        "contains" => {
            let field = object.get("field").and_then(Value::as_str).unwrap_or("");
            let value = object
                .get("value")
                .and_then(Value::as_str)
                .unwrap_or("");
            if field.is_empty() {
                return Ok(None);
            }
            let col = quote_ident(field)?;
            Ok(Some(format!(
                "strpos(CAST({col} AS VARCHAR), {}) > 0",
                quote_string(value)
            )))
        }
        "placeholder_only" => {
            let field = object.get("field").and_then(Value::as_str).unwrap_or("");
            if field.is_empty() {
                return Ok(None);
            }
            let col = quote_ident(field)?;
            // Match kernel: non-empty and every char is dash/space-like.
            Ok(Some(format!(
                "(TRIM(CAST({col} AS VARCHAR)) <> '' AND \
                 regexp_replace(CAST({col} AS VARCHAR), '[-—－― \\t\\n\\r]', '', 'g') = '')"
            )))
        }
        _ => Ok(None),
    }
}

/// Aligns with kernel `blank_field` sentinels (承办部门 / 办结时间 etc.).
fn blank_field_sql(field: &str) -> Result<String> {
    let col = quote_ident(field)?;
    let trimmed = format!("TRIM(CAST({col} AS VARCHAR))");
    let ascii_sentinels = ["n/a", "na", "null", "none"];
    let other_sentinels = [
        "—", "-", "/", "无", "暂无", "待定", "未知", "无承办部门", "无部门",
    ];
    let ascii_list = ascii_sentinels
        .iter()
        .map(|s| quote_string(s))
        .collect::<Vec<_>>()
        .join(", ");
    let mut parts = vec![
        format!("{col} IS NULL"),
        format!("{trimmed} = ''"),
        format!("LOWER({trimmed}) IN ({ascii_list})"),
    ];
    for s in other_sentinels {
        parts.push(format!("{trimmed} = {}", quote_string(s)));
    }
    Ok(format!("({})", parts.join(" OR ")))
}

fn literal_sql(value: Option<&Value>) -> Result<String> {
    Ok(match value {
        Some(Value::String(s)) => quote_string(s),
        Some(Value::Number(n)) => n.to_string(),
        Some(Value::Bool(b)) => {
            if *b {
                "TRUE".into()
            } else {
                "FALSE".into()
            }
        }
        Some(Value::Null) | None => "NULL".into(),
        _ => bail!("unsupported predicate literal"),
    })
}

fn lower_sort_by(
    app_root: &Path,
    datasets: &BTreeMap<String, DatasetView>,
    object: &serde_json::Map<String, Value>,
    setup: &mut Vec<String>,
    depth: usize,
) -> Result<Option<Rel>> {
    let Some(inner_expr) = object.get("rowset") else {
        return Ok(None);
    };
    let Some(inner) = lower_rel(app_root, datasets, inner_expr, setup, depth + 1)? else {
        return Ok(None);
    };
    let field = object
        .get("field")
        .or_else(|| object.get("by"))
        .and_then(Value::as_str)
        .unwrap_or("");
    if field.is_empty() {
        return Ok(None);
    }
    let col = quote_ident(field)?;
    let order = object.get("order").and_then(Value::as_str).unwrap_or("asc");
    let dir = if order.eq_ignore_ascii_case("desc") {
        "DESC"
    } else {
        "ASC"
    };
    let order_by = if crate::paginate::is_serial_number_field(field) {
        super::serial_number_order_by_parts(&col, dir).join(", ")
    } else {
        // Prefer VARCHAR order only: try_cast(... AS DOUBLE) can trip DataFusion
        // Utf8View range-query bugs on string/date columns.
        format!("CAST({col} AS VARCHAR) {dir}")
    };
    Ok(Some(Rel {
        sql: format!("SELECT * FROM ({}) AS _s ORDER BY {order_by}", inner.sql),
        columns: inner.columns,
    }))
}

fn lower_limit(
    app_root: &Path,
    datasets: &BTreeMap<String, DatasetView>,
    object: &serde_json::Map<String, Value>,
    setup: &mut Vec<String>,
    depth: usize,
) -> Result<Option<Rel>> {
    let Some(inner_expr) = object.get("rowset") else {
        return Ok(None);
    };
    let Some(inner) = lower_rel(app_root, datasets, inner_expr, setup, depth + 1)? else {
        return Ok(None);
    };
    // DataFusion may drop ORDER BY from a subquery unless LIMIT is on the same
    // SELECT. When the child already ends with ORDER BY, append LIMIT in-place.
    let n = object
        .get("n")
        .or_else(|| object.get("limit"))
        .and_then(Value::as_u64)
        .unwrap_or(10)
        .min(MAX_PIPELINE_SQL_ROWS as u64);
    let sql = if inner.sql.contains(" ORDER BY ") {
        format!("{} LIMIT {n}", inner.sql.trim_end_matches(';'))
    } else {
        format!("SELECT * FROM ({}) AS _l LIMIT {n}", inner.sql)
    };
    Ok(Some(Rel {
        sql,
        columns: inner.columns,
    }))
}

fn lower_select(
    app_root: &Path,
    datasets: &BTreeMap<String, DatasetView>,
    object: &serde_json::Map<String, Value>,
    setup: &mut Vec<String>,
    depth: usize,
) -> Result<Option<Rel>> {
    let Some(inner_expr) = object.get("rowset") else {
        return Ok(None);
    };
    let Some(inner) = lower_rel(app_root, datasets, inner_expr, setup, depth + 1)? else {
        return Ok(None);
    };
    let Some(fields) = object.get("fields").and_then(Value::as_array) else {
        return Ok(None);
    };
    let mut cols = Vec::new();
    let mut select_parts = Vec::new();
    for field in fields {
        let Some(name) = field.as_str().map(str::trim).filter(|s| !s.is_empty()) else {
            return Ok(None);
        };
        let q = quote_ident(name)?;
        select_parts.push(q);
        cols.push(name.to_string());
    }
    if select_parts.is_empty() {
        return Ok(None);
    }
    Ok(Some(Rel {
        sql: format!(
            "SELECT {} FROM ({}) AS _sel",
            select_parts.join(", "),
            inner.sql
        ),
        columns: cols,
    }))
}

fn lower_rename(
    app_root: &Path,
    datasets: &BTreeMap<String, DatasetView>,
    object: &serde_json::Map<String, Value>,
    setup: &mut Vec<String>,
    depth: usize,
) -> Result<Option<Rel>> {
    let Some(inner_expr) = object.get("rowset") else {
        return Ok(None);
    };
    let Some(inner) = lower_rel(app_root, datasets, inner_expr, setup, depth + 1)? else {
        return Ok(None);
    };
    let Some(mapping) = object.get("mapping").and_then(Value::as_object) else {
        return Ok(None);
    };
    if mapping.is_empty() {
        return Ok(None);
    }
    // Prefer explicit mapping order; fall back to projecting only renamed fields
    // when inner columns are unknown (common after aggregates).
    let mut select_parts = Vec::new();
    let mut cols = Vec::new();
    if inner.columns.is_empty() {
        for (from, to_val) in mapping {
            let Some(to) = to_val.as_str().map(str::trim).filter(|s| !s.is_empty()) else {
                return Ok(None);
            };
            let from_q = quote_ident(from)?;
            let to_q = quote_ident(to)?;
            select_parts.push(format!("{from_q} AS {to_q}"));
            cols.push(to.to_string());
        }
    } else {
        for col in &inner.columns {
            if let Some(to_val) = mapping.get(col) {
                let Some(to) = to_val.as_str().map(str::trim).filter(|s| !s.is_empty()) else {
                    return Ok(None);
                };
                let from_q = quote_ident(col)?;
                let to_q = quote_ident(to)?;
                select_parts.push(format!("{from_q} AS {to_q}"));
                cols.push(to.to_string());
            } else {
                let q = quote_ident(col)?;
                select_parts.push(q);
                cols.push(col.clone());
            }
        }
        // Include mapping keys that were not in known columns (e.g. after select).
        for (from, to_val) in mapping {
            if inner.columns.iter().any(|c| c == from) {
                continue;
            }
            let Some(to) = to_val.as_str().map(str::trim).filter(|s| !s.is_empty()) else {
                return Ok(None);
            };
            let from_q = quote_ident(from)?;
            let to_q = quote_ident(to)?;
            select_parts.push(format!("{from_q} AS {to_q}"));
            cols.push(to.to_string());
        }
    }
    Ok(Some(Rel {
        sql: format!(
            "SELECT {} FROM ({}) AS _ren",
            select_parts.join(", "),
            inner.sql
        ),
        columns: cols,
    }))
}

fn lower_trend_year_compare(
    app_root: &Path,
    datasets: &BTreeMap<String, DatasetView>,
    object: &serde_json::Map<String, Value>,
    setup: &mut Vec<String>,
    depth: usize,
) -> Result<Option<Rel>> {
    let Some(inner_expr) = object.get("rowset") else {
        return Ok(None);
    };
    let Some(inner) = lower_rel(app_root, datasets, inner_expr, setup, depth + 1)? else {
        return Ok(None);
    };
    let date_field = object
        .get("date_field")
        .and_then(Value::as_str)
        .unwrap_or("");
    if date_field.is_empty() {
        return Ok(None);
    }
    let value_field = object.get("value").and_then(Value::as_str);
    let agg = object.get("agg").and_then(Value::as_str).unwrap_or("count");
    let months = object
        .get("limit")
        .and_then(Value::as_u64)
        .map(|n| n as usize)
        .unwrap_or(6)
        .clamp(1, 12);
    let month_label = object
        .get("month_label_field")
        .and_then(Value::as_str)
        .unwrap_or("month");
    let year_label = object
        .get("year_label_field")
        .and_then(Value::as_str)
        .unwrap_or("year");
    let window_mode = object
        .get("window")
        .and_then(Value::as_str)
        .unwrap_or("rolling");
    let years = parse_years(object.get("years"));
    // 024008: omit years → auto from filtered rows; explicit → intersect with present.
    let date_expr = sql_parse_date_expr(date_field)?;
    let agg_expr = match (agg, value_field) {
        ("sum", Some(field)) => {
            let c = quote_ident(field)?;
            format!("COALESCE(SUM(try_cast({c} AS DOUBLE)), 0)")
        }
        ("avg", Some(field)) => {
            let c = quote_ident(field)?;
            format!("COALESCE(AVG(try_cast({c} AS DOUBLE)), 0)")
        }
        _ => "CAST(COUNT(*) AS DOUBLE)".to_string(),
    };
    let month_label_sql = quote_ident(month_label)?;
    let year_label_sql = quote_ident(year_label)?;

    // Optional row-level filters pushed from try_page_dataframe_metric_via_sql (024008).
    let row_filters = parse_row_filter_map(object.get("__mei_row_filters"));
    let row_search = object
        .get("__mei_row_search")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let inner_where = if row_filters.is_empty() && row_search.is_none() {
        String::new()
    } else {
        let cols = if inner.columns.is_empty() {
            // Best-effort: filter keys themselves (mapped dataset columns).
            row_filters.keys().cloned().collect::<Vec<_>>()
        } else {
            inner.columns.clone()
        };
        build_where_clause(&row_filters, row_search, &cols)?
    };
    let filtered_inner = if inner_where.is_empty() {
        format!("({})", inner.sql)
    } else {
        format!("(SELECT * FROM ({}) AS _mei_trend_src{inner_where})", inner.sql)
    };

    // Rolling months must match kernel `latest_month_window`: emit month numbers
    // in chronological ascending order ending at max(date), even when some months have no rows.
    // DataFusion cannot multiply Interval by a column, so resolve ending month in Rust.
    // IMPORTANT: ORDER BY window ordinal — not month_num (year-wrap would become 01,02,03,10…).
    let month_source = if window_mode.eq_ignore_ascii_case("calendar") {
        "(SELECT m, (m - 1) AS ord FROM generate_series(1, 12) AS t(m))".to_string()
    } else {
        let ending_month = probe_ending_month(app_root, &filtered_inner, &date_expr)?.unwrap_or(12);
        let values = (0..months)
            .map(|i| {
                // Match kernel latest_month_window: oldest → newest.
                let delta = -(months as i32 - 1) + i as i32;
                let mut m = ending_month as i32 + delta;
                while m <= 0 {
                    m += 12;
                }
                while m > 12 {
                    m -= 12;
                }
                format!("({m}, {i})")
            })
            .collect::<Vec<_>>()
            .join(", ");
        format!("(SELECT m, ord FROM (VALUES {values}) AS t(m, ord))")
    };

    const MAX_YEARS: usize = 5; // 024008 TREND_YEAR_COMPARE_MAX_YEARS
    let years_cte = if years.is_empty() {
        format!(
            "years AS (\
               SELECT y FROM (\
                 SELECT CAST(date_part('year', d) AS INT) AS y \
                 FROM parsed WHERE d IS NOT NULL \
                 GROUP BY 1 \
                 ORDER BY y DESC \
                 LIMIT {MAX_YEARS}\
               ) AS recent ORDER BY y\
             )"
        )
    } else {
        let years_values = years
            .iter()
            .map(|y| format!("({y})"))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "years AS (\
               SELECT y FROM (VALUES {years_values}) AS t(y) \
               WHERE y IN (\
                 SELECT DISTINCT CAST(date_part('year', d) AS INT) FROM parsed WHERE d IS NOT NULL\
               )\
             )"
        )
    };

    let sql = format!(
        "WITH parsed AS (\
           SELECT {date_expr} AS d, t.* FROM {filtered_inner} AS t\
         ), \
         months AS {month_source}, \
         {years_cte}, \
         grid AS (\
           SELECT m.m AS month_num, m.ord AS month_ord, y.y AS year_num \
           FROM months m CROSS JOIN years y\
         ), \
         agg AS (\
           SELECT CAST(date_part('year', d) AS INT) AS year_num, CAST(date_part('month', d) AS INT) AS month_num, {agg_expr} AS value \
           FROM parsed \
           WHERE d IS NOT NULL AND CAST(date_part('year', d) AS INT) IN (SELECT y FROM years) \
           GROUP BY 1, 2\
         ) \
         SELECT \
           lpad(CAST(g.month_num AS VARCHAR), 2, '0') AS {month_label_sql}, \
           CAST(g.year_num AS VARCHAR) AS {year_label_sql}, \
           COALESCE(a.value, 0) AS value \
         FROM grid g \
         LEFT JOIN agg a ON a.month_num = g.month_num AND a.year_num = g.year_num \
         ORDER BY g.month_ord, g.year_num",
        filtered_inner = filtered_inner,
        month_source = month_source,
        years_cte = years_cte,
        agg_expr = agg_expr,
        date_expr = date_expr,
        month_label_sql = month_label_sql,
        year_label_sql = year_label_sql,
    );

    Ok(Some(Rel {
        sql,
        columns: vec![
            month_label.to_string(),
            year_label.to_string(),
            "value".to_string(),
        ],
    }))
}

fn parse_row_filter_map(value: Option<&Value>) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let Some(map) = value.and_then(Value::as_object) else {
        return out;
    };
    for (key, raw) in map {
        let text = match raw {
            Value::String(s) => s.trim().to_string(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            _ => continue,
        };
        if key.trim().is_empty() || text.is_empty() {
            continue;
        }
        out.insert(key.trim().to_string(), text);
    }
    out
}

fn lower_party_year_aggregate(
    app_root: &Path,
    datasets: &BTreeMap<String, DatasetView>,
    object: &serde_json::Map<String, Value>,
    setup: &mut Vec<String>,
    depth: usize,
) -> Result<Option<Rel>> {
    let Some(inner_expr) = object.get("rowset") else {
        return Ok(None);
    };
    let Some(inner) = lower_rel(app_root, datasets, inner_expr, setup, depth + 1)? else {
        return Ok(None);
    };
    let party_field = object
        .get("party_field")
        .and_then(Value::as_str)
        .unwrap_or("");
    let date_field = object
        .get("date_field")
        .and_then(Value::as_str)
        .unwrap_or("");
    let value_field = object
        .get("value_field")
        .and_then(Value::as_str)
        .unwrap_or("");
    if party_field.is_empty() || date_field.is_empty() || value_field.is_empty() {
        return Ok(None);
    }
    let years = parse_years(object.get("years"));
    if years.is_empty() {
        return Ok(None);
    }
    let party = quote_ident(party_field)?;
    let value = quote_ident(value_field)?;
    let date_expr = sql_parse_date_expr(date_field)?;
    let years_list = years
        .iter()
        .map(|y| y.to_string())
        .collect::<Vec<_>>()
        .join(", ");

    let mut select_cols = vec![party_field.to_string()];
    let mut select_sql_parts = vec![format!("{party} AS {party}")];
    for year in &years {
        let sum_alias = format!("罚没金额_{year}");
        let count_alias = format!("处罚次数_{year}");
        let sum_q = quote_ident(&sum_alias)?;
        let count_q = quote_ident(&count_alias)?;
        // DataFusion SQL parser does not accept FILTER (WHERE …) aggregates; use CASE.
        select_sql_parts.push(format!(
            "COALESCE(SUM(CASE WHEN CAST(date_part('year', d) AS INT) = {year} THEN try_cast({value} AS DOUBLE) END), 0) AS {sum_q}"
        ));
        select_sql_parts.push(format!(
            "CAST(COUNT(CASE WHEN CAST(date_part('year', d) AS INT) = {year} THEN 1 END) AS DOUBLE) AS {count_q}"
        ));
        select_cols.push(sum_alias);
        select_cols.push(count_alias);
    }
    if years.len() >= 2 {
        let prev = years[years.len() - 2];
        let curr = years[years.len() - 1];
        let alias = format!("同比降低额_{curr}");
        let alias_q = quote_ident(&alias)?;
        let prev_q = quote_ident(&format!("罚没金额_{prev}"))?;
        let curr_q = quote_ident(&format!("罚没金额_{curr}"))?;
        // Computed in outer select after aggregation.
        let sql = format!(
            "SELECT inner_agg.*, \
               GREATEST(inner_agg.{prev_q} - inner_agg.{curr_q}, 0) AS {alias_q} \
             FROM (\
               SELECT {} \
               FROM (SELECT {date_expr} AS d, t.* FROM ({}) AS t) AS src \
               WHERE d IS NOT NULL AND CAST(date_part('year', d) AS INT) IN ({years_list}) \
                 AND {party} IS NOT NULL AND TRIM(CAST({party} AS VARCHAR)) <> '' \
               GROUP BY {party}\
             ) AS inner_agg",
            select_sql_parts.join(", "),
            inner.sql,
        );
        select_cols.push(alias);
        return Ok(Some(Rel {
            sql,
            columns: select_cols,
        }));
    }

    let sql = format!(
        "SELECT {} \
         FROM (SELECT {date_expr} AS d, t.* FROM ({}) AS t) AS src \
         WHERE d IS NOT NULL AND CAST(date_part('year', d) AS INT) IN ({years_list}) \
           AND {party} IS NOT NULL AND TRIM(CAST({party} AS VARCHAR)) <> '' \
         GROUP BY {party}",
        select_sql_parts.join(", "),
        inner.sql,
    );
    Ok(Some(Rel {
        sql,
        columns: select_cols,
    }))
}

fn lower_unpivot_columns(
    app_root: &Path,
    datasets: &BTreeMap<String, DatasetView>,
    object: &serde_json::Map<String, Value>,
    setup: &mut Vec<String>,
    depth: usize,
) -> Result<Option<Rel>> {
    let Some(inner_expr) = object.get("rowset") else {
        return Ok(None);
    };
    let Some(inner) = lower_rel(app_root, datasets, inner_expr, setup, depth + 1)? else {
        return Ok(None);
    };
    let id_field = object.get("id_field").and_then(Value::as_str).unwrap_or("");
    if id_field.is_empty() {
        return Ok(None);
    }
    let year_field = object
        .get("year_field")
        .and_then(Value::as_str)
        .unwrap_or("year");
    let value_field = object
        .get("value_field")
        .and_then(Value::as_str)
        .unwrap_or("value");
    let Some(columns) = object.get("columns").and_then(Value::as_array) else {
        return Ok(None);
    };
    let mut unions = Vec::new();
    let id_q = quote_ident(id_field)?;
    let year_q = quote_ident(year_field)?;
    let value_q = quote_ident(value_field)?;
    for item in columns {
        let Some(obj) = item.as_object() else {
            continue;
        };
        let Some(year) = obj.get("year").and_then(Value::as_str) else {
            continue;
        };
        let Some(field) = obj.get("field").and_then(Value::as_str) else {
            continue;
        };
        let field_q = quote_ident(field)?;
        let year_lit = quote_string(year);
        unions.push(format!(
            "SELECT {id_q} AS {id_q}, {year_lit} AS {year_q}, COALESCE(try_cast({field_q} AS DOUBLE), 0) AS {value_q} \
             FROM ({}) AS _u",
            inner.sql
        ));
    }
    if unions.is_empty() {
        return Ok(None);
    }
    Ok(Some(Rel {
        sql: unions.join(" UNION ALL "),
        columns: vec![
            id_field.to_string(),
            year_field.to_string(),
            value_field.to_string(),
        ],
    }))
}

fn lower_bucket_date(
    app_root: &Path,
    datasets: &BTreeMap<String, DatasetView>,
    object: &serde_json::Map<String, Value>,
    setup: &mut Vec<String>,
    depth: usize,
) -> Result<Option<Rel>> {
    let Some(inner_expr) = object.get("rowset") else {
        return Ok(None);
    };
    let Some(inner) = lower_rel(app_root, datasets, inner_expr, setup, depth + 1)? else {
        return Ok(None);
    };
    let field = object.get("field").and_then(Value::as_str).unwrap_or("");
    if field.is_empty() {
        return Ok(None);
    }
    let label_field = object
        .get("label_field")
        .and_then(Value::as_str)
        .or_else(|| object.get("by").and_then(Value::as_str))
        .unwrap_or("month");
    let date_expr = sql_parse_date_expr(field)?;
    let label_q = quote_ident(label_field)?;
    // Match kernel format_month_label → "YYYY-MM"; unparseable dates become "".
    let month_label_expr = format!(
        "CASE WHEN ({date_expr}) IS NOT NULL THEN \
           concat(\
             CAST(CAST(date_part('year', {date_expr}) AS INT) AS VARCHAR), '-', \
             lpad(CAST(CAST(date_part('month', {date_expr}) AS INT) AS VARCHAR), 2, '0')\
           ) \
         ELSE '' END"
    );
    if inner.columns.is_empty() {
        // Unknown projection: overlay label (may duplicate name; group_by still reads alias).
        return Ok(Some(Rel {
            sql: format!(
                "SELECT _bd.*, ({month_label_expr}) AS {label_q} FROM ({}) AS _bd",
                inner.sql
            ),
            columns: Vec::new(),
        }));
    }
    let mut select_parts = Vec::new();
    let mut cols = Vec::new();
    let mut replaced = false;
    for col in &inner.columns {
        if col == label_field {
            select_parts.push(format!("({month_label_expr}) AS {label_q}"));
            cols.push(label_field.to_string());
            replaced = true;
        } else {
            select_parts.push(quote_ident(col)?);
            cols.push(col.clone());
        }
    }
    if !replaced {
        select_parts.push(format!("({month_label_expr}) AS {label_q}"));
        cols.push(label_field.to_string());
    }
    Ok(Some(Rel {
        sql: format!(
            "SELECT {} FROM ({}) AS _bd",
            select_parts.join(", "),
            inner.sql
        ),
        columns: cols,
    }))
}

fn parse_group_by_fields(object: &serde_json::Map<String, Value>) -> Option<Vec<String>> {
    object
        .get("fields")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .filter(|items| !items.is_empty())
        .or_else(|| {
            object
                .get("by")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(|field| vec![field.to_string()])
        })
        .or_else(|| {
            object
                .get("fields")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(|field| vec![field.to_string()])
        })
        .or_else(|| {
            object
                .get("field")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(|field| vec![field.to_string()])
        })
}

fn lower_group_by_pivot(
    inner: &Rel,
    group_fields: &[String],
    object: &serde_json::Map<String, Value>,
) -> Result<Option<Rel>> {
    // Pivot + universe (year grid / first-dim pad) stays uncovered this round.
    if object.get("universe").is_some() {
        return Ok(None);
    }
    let pivot_field = object
        .get("pivot_field")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("");
    let Some(pivot_columns) = object.get("pivot_columns").and_then(Value::as_array) else {
        return Ok(None);
    };
    let pivot_cols: Vec<String> = pivot_columns
        .iter()
        .filter_map(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
        .collect();
    if pivot_field.is_empty() || pivot_cols.is_empty() || group_fields.is_empty() {
        return Ok(None);
    }
    // Kernel year-universe expansion (group_fields contains "年份") is not lowered here.
    if group_fields.iter().any(|f| f == "年份") {
        return Ok(None);
    }
    let pivot_q = quote_ident(pivot_field)?;
    let mut select_parts = Vec::new();
    let mut where_parts = Vec::new();
    let mut columns = Vec::new();
    for (idx, field) in group_fields.iter().enumerate() {
        let q = quote_ident(field)?;
        select_parts.push(format!("CAST({q} AS VARCHAR) AS {q}"));
        where_parts.push(format!(
            "{q} IS NOT NULL AND TRIM(CAST({q} AS VARCHAR)) <> ''"
        ));
        columns.push(field.clone());
        let _ = idx;
    }
    for col in &pivot_cols {
        let col_q = quote_ident(col)?;
        let lit = quote_string(col);
        // Match kernel aggregate_group_rows_pivot: count rows whose pivot equals the column.
        select_parts.push(format!(
            "CAST(SUM(CASE WHEN CAST({pivot_q} AS VARCHAR) = {lit} THEN 1 ELSE 0 END) AS BIGINT) AS {col_q}"
        ));
        columns.push(col.clone());
    }
    let group_by_ordinals = (1..=group_fields.len())
        .map(|n| n.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    let mut sql = format!(
        "SELECT {select} FROM ({inner}) AS _gbp WHERE {where} GROUP BY {group_by}",
        select = select_parts.join(", "),
        inner = inner.sql,
        where = where_parts.join(" AND "),
        group_by = group_by_ordinals,
    );
    if let Some(n) = object
        .get("limit")
        .and_then(Value::as_u64)
        .map(|n| n.min(MAX_PIPELINE_SQL_ROWS as u64))
    {
        sql = format!("SELECT * FROM ({sql}) AS _gbplimit LIMIT {n}");
    }
    Ok(Some(Rel { sql, columns }))
}

fn lower_group_by(
    app_root: &Path,
    datasets: &BTreeMap<String, DatasetView>,
    object: &serde_json::Map<String, Value>,
    setup: &mut Vec<String>,
    depth: usize,
) -> Result<Option<Rel>> {
    let Some(inner_expr) = object.get("rowset") else {
        return Ok(None);
    };
    let Some(mut inner) = lower_rel(app_root, datasets, inner_expr, setup, depth + 1)? else {
        return Ok(None);
    };
    // Chart cross-filter: apply pushed filters on the pre-agg rowset (e.g. 预警类型
    // while grouping by 数源单位). Outer page WHERE cannot see those columns.
    let row_filters = parse_row_filter_map(object.get("__mei_row_filters"));
    let row_search = object
        .get("__mei_row_search")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty());
    if !row_filters.is_empty() || row_search.is_some() {
        let cols = if inner.columns.is_empty() {
            row_filters.keys().cloned().collect::<Vec<_>>()
        } else {
            inner.columns.clone()
        };
        let inner_where = build_where_clause(&row_filters, row_search, &cols)?;
        if !inner_where.is_empty() {
            inner = Rel {
                sql: format!(
                    "SELECT * FROM ({}) AS _mei_gb_src{inner_where}",
                    inner.sql
                ),
                columns: inner.columns,
            };
        }
    }
    let Some(group_fields) = parse_group_by_fields(object) else {
        return Ok(None);
    };
    if object.get("pivot_field").is_some() || object.get("pivot_columns").is_some() {
        return lower_group_by_pivot(&inner, &group_fields, object);
    }
    let value_field = object.get("value").and_then(Value::as_str);
    let agg = object.get("agg").and_then(Value::as_str).unwrap_or("count");
    let agg_expr = match (agg, value_field) {
        ("sum", Some(field)) => {
            let c = quote_ident(field)?;
            format!("COALESCE(SUM(try_cast({c} AS DOUBLE)), 0)")
        }
        ("avg", Some(field)) => {
            let c = quote_ident(field)?;
            format!("COALESCE(AVG(try_cast({c} AS DOUBLE)), 0)")
        }
        ("min", Some(field)) => {
            let c = quote_ident(field)?;
            format!("MIN(try_cast({c} AS DOUBLE))")
        }
        ("max", Some(field)) => {
            let c = quote_ident(field)?;
            format!("MAX(try_cast({c} AS DOUBLE))")
        }
        _ => "CAST(COUNT(*) AS DOUBLE)".to_string(),
    };
    let limit = object
        .get("limit")
        .and_then(Value::as_u64)
        .map(|n| n.min(MAX_PIPELINE_SQL_ROWS as u64));
    let mut select_parts = Vec::new();
    let mut where_parts = Vec::new();
    let mut columns = Vec::new();
    for field in &group_fields {
        let q = quote_ident(field)?;
        select_parts.push(format!("CAST({q} AS VARCHAR) AS {q}"));
        where_parts.push(format!(
            "{q} IS NOT NULL AND TRIM(CAST({q} AS VARCHAR)) <> ''"
        ));
        columns.push(field.clone());
    }
    select_parts.push(format!("{agg_expr} AS value"));
    columns.push("value".to_string());
    let group_by_ordinals = (1..=group_fields.len())
        .map(|n| n.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    let grouped_sql = format!(
        "SELECT {select} FROM ({inner}) AS _gb WHERE {where} GROUP BY {group_by}",
        select = select_parts.join(", "),
        inner = inner.sql,
        where = where_parts.join(" AND "),
        group_by = group_by_ordinals,
    );
    let sql = if let Some(universe_expr) = object.get("universe") {
        // Universe pad only for single-key group_by (kernel apply_universe shape).
        if group_fields.len() != 1 {
            return Ok(None);
        }
        let group_field = group_fields[0].as_str();
        let group_q = quote_ident(group_field)?;
        let Some(universe) = lower_rel(app_root, datasets, universe_expr, setup, depth + 1)? else {
            return Ok(None);
        };
        // Match kernel apply_universe: pad missing keys with value 0, keep universe label order.
        format!(
            "WITH grouped AS ({grouped_sql}), \
             universe AS (\
               SELECT CAST({group_q} AS VARCHAR) AS {group_q} \
               FROM ({universe_sql}) AS _u \
               WHERE {group_q} IS NOT NULL AND TRIM(CAST({group_q} AS VARCHAR)) <> ''\
             ) \
             SELECT u.{group_q} AS {group_q}, COALESCE(g.value, 0) AS value \
             FROM universe u \
             LEFT JOIN grouped g ON u.{group_q} = g.{group_q}",
            universe_sql = universe.sql,
        )
    } else {
        grouped_sql
    };
    let sql = if let Some(n) = limit {
        format!("SELECT * FROM ({sql}) AS _gblimit LIMIT {n}")
    } else {
        sql
    };
    Ok(Some(Rel { sql, columns }))
}

fn lower_lookup_value(
    app_root: &Path,
    datasets: &BTreeMap<String, DatasetView>,
    object: &serde_json::Map<String, Value>,
    setup: &mut Vec<String>,
    depth: usize,
) -> Result<Option<Rel>> {
    let Some(inner_expr) = object.get("rowset") else {
        return Ok(None);
    };
    let Some(inner) = lower_rel(app_root, datasets, inner_expr, setup, depth + 1)? else {
        return Ok(None);
    };
    let left_field = object.get("field").and_then(Value::as_str).unwrap_or("");
    let right_key = object
        .get("lookup_field")
        .and_then(Value::as_str)
        .unwrap_or("");
    let right_value = object
        .get("value_field")
        .and_then(Value::as_str)
        .unwrap_or("");
    let as_field = object
        .get("as_field")
        .and_then(Value::as_str)
        .unwrap_or(right_value);
    let Some(lookup_rowset) = object.get("lookup_rowset") else {
        return Ok(None);
    };
    if left_field.is_empty()
        || right_key.is_empty()
        || right_value.is_empty()
        || as_field.is_empty()
    {
        return Ok(None);
    }
    let Some(lookup) = lower_rel(app_root, datasets, lookup_rowset, setup, depth + 1)? else {
        return Ok(None);
    };
    let l = quote_ident(left_field)?;
    let rk = quote_ident(right_key)?;
    let rv = quote_ident(right_value)?;
    let alias = quote_ident(as_field)?;
    let mut columns = inner.columns.clone();
    if !columns.iter().any(|c| c == as_field) {
        columns.push(as_field.to_string());
    }
    // Multi-value / hyphen-range object IDs need a first-token OR branch.
    // Only inspect join keys (left/right). Never use `as_field` (output alias):
    // map POI looks up 所属园区→园区名称 AS 园区ID — alias ends with ID but keys are names.
    let on_sql = if field_looks_like_object_key(left_field)
        || field_looks_like_object_key(right_key)
    {
        let first_token = format!(
            "regexp_replace(\
               regexp_replace(CAST(a.{l} AS VARCHAR), '[、，,;；\\n\\r\\t ]+.*$', ''), \
               '-.*$', \
               ''\
             )"
        );
        format!(
            "CAST(a.{l} AS VARCHAR) = CAST(b.{rk} AS VARCHAR) \
               OR ({first_token} <> '' AND {first_token} = CAST(b.{rk} AS VARCHAR))"
        )
    } else {
        format!("CAST(a.{l} AS VARCHAR) = CAST(b.{rk} AS VARCHAR)")
    };
    Ok(Some(Rel {
        sql: format!(
            "SELECT a.*, b.{rv} AS {alias} \
             FROM ({}) AS a \
             LEFT JOIN ({}) AS b ON {on_sql}",
            inner.sql, lookup.sql
        ),
        columns,
    }))
}

fn field_looks_like_object_key(field: &str) -> bool {
    let text = field.trim();
    if text.is_empty() {
        return false;
    }
    text.ends_with("ID")
        || text.ends_with("Id")
        || text.contains("预警ID")
        || text.contains("处理结果")
}

fn materialize_rel_as_cte(rel: Rel, setup: &mut Vec<String>) -> Result<Rel> {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    use std::hash::{Hash, Hasher};
    rel.sql.hash(&mut hasher);
    for col in &rel.columns {
        col.hash(&mut hasher);
    }
    let name = format!("mei_pipe_{:016x}", hasher.finish());
    let marker = format!("CTE:{name}|||{}", rel.sql);
    if !setup.iter().any(|item| item == &marker) {
        setup.push(marker);
    }
    let select = if rel.columns.is_empty() {
        format!("SELECT * FROM {name}")
    } else {
        let list = rel
            .columns
            .iter()
            .map(|c| quote_ident(c))
            .collect::<Result<Vec<_>>>()?
            .join(", ");
        format!("SELECT {list} FROM {name}")
    };
    Ok(Rel {
        sql: select,
        columns: rel.columns,
    })
}

fn lower_first_by(
    app_root: &Path,
    datasets: &BTreeMap<String, DatasetView>,
    object: &serde_json::Map<String, Value>,
    setup: &mut Vec<String>,
    depth: usize,
) -> Result<Option<Rel>> {
    let Some(inner_expr) = object.get("rowset") else {
        return Ok(None);
    };
    let Some(mut inner) = lower_rel(app_root, datasets, inner_expr, setup, depth + 1)? else {
        return Ok(None);
    };
    // Category / cross-filter: apply pushed filters on pre-dedup rowset.
    let row_filters = parse_row_filter_map(object.get("__mei_row_filters"));
    let row_search = object
        .get("__mei_row_search")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty());
    if !row_filters.is_empty() || row_search.is_some() {
        let cols = if inner.columns.is_empty() {
            row_filters.keys().cloned().collect::<Vec<_>>()
        } else {
            inner.columns.clone()
        };
        let inner_where = build_where_clause(&row_filters, row_search, &cols)?;
        if !inner_where.is_empty() {
            inner = Rel {
                sql: format!(
                    "SELECT * FROM ({}) AS _fb_src{inner_where}",
                    inner.sql
                ),
                columns: inner.columns,
            };
        }
    }
    let field = object.get("field").and_then(Value::as_str).unwrap_or("");
    if field.is_empty() {
        return Ok(None);
    }
    let col = quote_ident(field)?;
    // DataFusion rejects `ORDER BY (SELECT 1)` (ScalarSubquery); order by partition key.
    let numbered = if inner.columns.is_empty() {
        Rel {
            sql: format!(
                "SELECT * FROM (\
                   SELECT *, ROW_NUMBER() OVER (\
                     PARTITION BY CAST({col} AS VARCHAR) ORDER BY CAST({col} AS VARCHAR)\
                   ) AS _mei_rn \
                   FROM ({}) AS _fb_src\
                 ) AS _fb WHERE _mei_rn = 1",
                inner.sql
            ),
            columns: Vec::new(),
        }
    } else {
        let select_list = inner
            .columns
            .iter()
            .map(|c| quote_ident(c))
            .collect::<Result<Vec<_>>>()?
            .join(", ");
        Rel {
            sql: format!(
                "SELECT {select_list} FROM (\
                   SELECT *, ROW_NUMBER() OVER (\
                     PARTITION BY CAST({col} AS VARCHAR) ORDER BY CAST({col} AS VARCHAR)\
                   ) AS _mei_rn \
                   FROM ({}) AS _fb_src\
                 ) AS _fb WHERE _mei_rn = 1",
                inner.sql
            ),
            columns: inner.columns.clone(),
        }
    };
    // CTE-share identical first_by subtrees (label_status embeds it thrice).
    Ok(Some(materialize_rel_as_cte(numbered, setup)?))
}

fn lower_mutate(
    app_root: &Path,
    datasets: &BTreeMap<String, DatasetView>,
    object: &serde_json::Map<String, Value>,
    setup: &mut Vec<String>,
    depth: usize,
) -> Result<Option<Rel>> {
    let Some(inner_expr) = object.get("rowset") else {
        return Ok(None);
    };
    let Some(inner) = lower_rel(app_root, datasets, inner_expr, setup, depth + 1)? else {
        return Ok(None);
    };
    let Some(updates) = object.get("updates").and_then(Value::as_object) else {
        return Ok(None);
    };
    if updates.is_empty() {
        return Ok(None);
    }
    // Pass 1: lit / extract_* / div / sub (coalesce needs sibling exprs).
    let mut all_updates: BTreeMap<String, String> = BTreeMap::new();
    let mut coalesce_pending: Vec<(String, Vec<String>)> = Vec::new();
    for (key, expr) in updates {
        let Some(map) = expr.as_object() else {
            return Ok(None);
        };
        if map.get("__kind").and_then(Value::as_str) != Some("analysis_expr") {
            return Ok(None);
        }
        match map.get("type").and_then(Value::as_str).unwrap_or("") {
            "lit" => {
                all_updates.insert(key.clone(), literal_sql(map.get("value"))?);
            }
            "extract_number" => {
                let field = map.get("field").and_then(Value::as_str).unwrap_or("");
                if field.is_empty() {
                    return Ok(None);
                }
                let col = quote_ident(field)?;
                let pattern = map.get("pattern").and_then(Value::as_str).unwrap_or("");
                let sql = if pattern.is_empty() {
                    // Strip non-digit (and non-dot) chars, then cast.
                    format!(
                        "try_cast(regexp_replace(CAST({col} AS VARCHAR), '[^0-9.]', '', 'g') AS DOUBLE)"
                    )
                } else {
                    // First capture group via regexp_match(...[1]).
                    format!(
                        "try_cast((regexp_match(CAST({col} AS VARCHAR), {}))[1] AS DOUBLE)",
                        quote_string(pattern)
                    )
                };
                all_updates.insert(key.clone(), sql);
            }
            "extract_match" => {
                let field = map.get("field").and_then(Value::as_str).unwrap_or("");
                let pattern = map.get("pattern").and_then(Value::as_str).unwrap_or("");
                if field.is_empty() || pattern.is_empty() {
                    return Ok(None);
                }
                let col = quote_ident(field)?;
                let pat = quote_string(pattern);
                // Match kernel `regex_capture_text`: prefer group 1, else group 0.
                all_updates.insert(
                    key.clone(),
                    format!(
                        "COALESCE(\
                           (regexp_match(CAST({col} AS VARCHAR), {pat}))[1], \
                           (regexp_match(CAST({col} AS VARCHAR), {pat}))[0], \
                           ''\
                         )"
                    ),
                );
            }
            "div" => {
                let field = map.get("field").and_then(Value::as_str).unwrap_or("");
                if field.is_empty() {
                    return Ok(None);
                }
                let col = quote_ident(field)?;
                let by = map
                    .get("by")
                    .and_then(|value| {
                        value
                            .as_f64()
                            .or_else(|| value.as_i64().map(|n| n as f64))
                            .or_else(|| value.as_u64().map(|n| n as f64))
                            .or_else(|| value.as_str().and_then(|s| s.parse::<f64>().ok()))
                    })
                    .unwrap_or(1.0);
                if !by.is_finite() || by.abs() < f64::EPSILON {
                    all_updates.insert(key.clone(), "CAST(NULL AS DOUBLE)".into());
                } else {
                    all_updates.insert(
                        key.clone(),
                        format!("(try_cast({col} AS DOUBLE) / CAST({by} AS DOUBLE))"),
                    );
                }
            }
            "sub" => {
                let left_field = map.get("left_field").and_then(Value::as_str).unwrap_or("");
                let right_field = map.get("right_field").and_then(Value::as_str).unwrap_or("");
                if left_field.is_empty() || right_field.is_empty() {
                    return Ok(None);
                }
                let left = quote_ident(left_field)?;
                let right = quote_ident(right_field)?;
                all_updates.insert(
                    key.clone(),
                    format!("(try_cast({left} AS DOUBLE) - try_cast({right} AS DOUBLE))"),
                );
            }
            "coalesce" => {
                let fields = map
                    .get("fields")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                let names = fields
                    .iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>();
                if names.is_empty() {
                    return Ok(None);
                }
                coalesce_pending.push((key.clone(), names));
            }
            _ => return Ok(None),
        }
    }
    // Pass 2: coalesce may reference sibling mutate columns (e.g. street_u/street_p).
    for (key, names) in coalesce_pending {
        let mut parts = Vec::with_capacity(names.len());
        for name in &names {
            let expr = if let Some(sql) = all_updates.get(name) {
                sql.clone()
            } else {
                let col = quote_ident(name)?;
                format!("CAST({col} AS VARCHAR)")
            };
            parts.push(format!("NULLIF(TRIM(CAST(({expr}) AS VARCHAR)), '')"));
        }
        all_updates.insert(
            key,
            format!("COALESCE({}, '')", parts.join(", ")),
        );
    }
    if all_updates.is_empty() {
        return Ok(None);
    }
    let mut select_parts = Vec::new();
    let mut cols = Vec::new();
    if inner.columns.is_empty() {
        // Unknown schema: keep source columns and append updates.
        for (key, expr_sql) in &all_updates {
            let alias = quote_ident(key)?;
            select_parts.push(format!("{expr_sql} AS {alias}"));
            cols.push(key.clone());
        }
        if select_parts.is_empty() {
            return Ok(None);
        }
        return Ok(Some(Rel {
            sql: format!(
                "SELECT *, {} FROM ({}) AS _mut",
                select_parts.join(", "),
                inner.sql
            ),
            columns: cols,
        }));
    }
    for col in &inner.columns {
        let q = quote_ident(col)?;
        if let Some(expr_sql) = all_updates.get(col) {
            select_parts.push(format!("{expr_sql} AS {q}"));
        } else {
            select_parts.push(q);
        }
        cols.push(col.clone());
    }
    for (key, expr_sql) in &all_updates {
        if cols.iter().any(|c| c == key) {
            continue;
        }
        let alias = quote_ident(key)?;
        select_parts.push(format!("{expr_sql} AS {alias}"));
        cols.push(key.clone());
    }
    Ok(Some(Rel {
        sql: format!(
            "SELECT {} FROM ({}) AS _mut",
            select_parts.join(", "),
            inner.sql
        ),
        columns: cols,
    }))
}

/// Match kernel `eval_split_text_rowset`: explode field by delimiter; if no
/// non-empty parts remain, keep the original row once.
fn lower_split_text(
    app_root: &Path,
    datasets: &BTreeMap<String, DatasetView>,
    object: &serde_json::Map<String, Value>,
    setup: &mut Vec<String>,
    depth: usize,
) -> Result<Option<Rel>> {
    let Some(inner_expr) = object.get("rowset") else {
        return Ok(None);
    };
    let Some(inner) = lower_rel(app_root, datasets, inner_expr, setup, depth + 1)? else {
        return Ok(None);
    };
    let field = object.get("field").and_then(Value::as_str).unwrap_or("");
    if field.is_empty() {
        return Ok(None);
    }
    let delimiter = object
        .get("delimiter")
        .and_then(Value::as_str)
        .unwrap_or("、");
    let drop_empty = object
        .get("on_empty")
        .and_then(Value::as_str)
        .map(str::trim)
        .is_some_and(|v| v.eq_ignore_ascii_case("drop"));
    let col = quote_ident(field)?;
    let delim = quote_string(delimiter);
    let order_key = if inner.columns.is_empty() {
        "1".to_string()
    } else {
        quote_ident(&inner.columns[0])?
    };
    // Project to ASCII aliases in intermediate CTEs — DataFusion UNNEST +
    // non-ASCII qualified names can hit OuterReferenceColumn planner bugs.
    let (src_proj, kept_proj, empty_proj, outer_proj, columns) = if inner.columns.is_empty() {
        (
            format!("*, CAST({col} AS VARCHAR) AS __mei_text"),
            format!("* EXCLUDE (__mei_part, __mei_rid, __mei_text, {col}), __mei_part AS {col}"),
            format!("* EXCLUDE (__mei_rid, __mei_text)"),
            "*".to_string(),
            {
                let mut c = Vec::new();
                c.push(field.to_string());
                c
            },
        )
    } else {
        let mut src_parts = Vec::new();
        let mut kept_parts = Vec::new();
        let mut empty_parts = Vec::new();
        let mut outer_parts = Vec::new();
        for (idx, name) in inner.columns.iter().enumerate() {
            let alias = format!("__c{idx}");
            let q = quote_ident(name)?;
            src_parts.push(format!("{q} AS {alias}"));
            if name == field {
                kept_parts.push(format!("__mei_part AS {alias}"));
            } else {
                kept_parts.push(alias.clone());
            }
            empty_parts.push(alias.clone());
            outer_parts.push(format!("{alias} AS {q}"));
        }
        if !inner.columns.iter().any(|c| c == field) {
            let alias = format!("__c{}", inner.columns.len());
            src_parts.push(format!("CAST({col} AS VARCHAR) AS {alias}"));
            kept_parts.push(format!("__mei_part AS {alias}"));
            empty_parts.push(alias.clone());
            outer_parts.push(format!("{alias} AS {col}"));
        }
        src_parts.push(format!("CAST({col} AS VARCHAR) AS __mei_text"));
        (
            src_parts.join(", "),
            kept_parts.join(", "),
            empty_parts.join(", "),
            outer_parts.join(", "),
            {
                let mut c = inner.columns.clone();
                if !c.iter().any(|x| x == field) {
                    c.push(field.to_string());
                }
                c
            },
        )
    };
    let sql = if drop_empty {
        format!(
            "WITH src AS (\
               SELECT {src_proj}, ROW_NUMBER() OVER (ORDER BY {order_key}) AS __mei_rid \
               FROM ({inner}) AS _st\
             ), \
             src_arr AS (\
               SELECT *, string_to_array(COALESCE(__mei_text, ''), {delim}) AS __mei_arr FROM src\
             ), \
             exploded AS (\
               SELECT * EXCLUDE (__mei_arr, __mei_text), trim(unnest(__mei_arr)) AS __mei_part \
               FROM src_arr\
             ), \
             kept AS (\
               SELECT {kept_proj} FROM exploded WHERE __mei_part <> ''\
             ) \
             SELECT {outer_proj} FROM kept",
            inner = inner.sql,
            src_proj = src_proj,
            kept_proj = kept_proj,
            outer_proj = outer_proj,
            delim = delim,
            order_key = order_key,
        )
    } else {
        format!(
            "WITH src AS (\
               SELECT {src_proj}, ROW_NUMBER() OVER (ORDER BY {order_key}) AS __mei_rid \
               FROM ({inner}) AS _st\
             ), \
             src_arr AS (\
               SELECT *, string_to_array(COALESCE(__mei_text, ''), {delim}) AS __mei_arr FROM src\
             ), \
             exploded AS (\
               SELECT * EXCLUDE (__mei_arr, __mei_text), trim(unnest(__mei_arr)) AS __mei_part \
               FROM src_arr\
             ), \
             has_part AS (\
               SELECT DISTINCT __mei_rid FROM exploded WHERE __mei_part <> ''\
             ), \
             kept AS (\
               SELECT {kept_proj} FROM exploded WHERE __mei_part <> ''\
             ), \
             empty_rows AS (\
               SELECT {empty_proj} FROM src s \
               LEFT JOIN has_part h ON h.__mei_rid = s.__mei_rid \
               WHERE h.__mei_rid IS NULL\
             ) \
             SELECT {outer_proj} FROM kept \
             UNION ALL \
             SELECT {outer_proj} FROM empty_rows",
            inner = inner.sql,
            src_proj = src_proj,
            kept_proj = kept_proj,
            empty_proj = empty_proj,
            outer_proj = outer_proj,
            delim = delim,
            order_key = order_key,
        )
    };
    Ok(Some(Rel { sql, columns }))
}

fn lower_concat_rowsets(
    app_root: &Path,
    datasets: &BTreeMap<String, DatasetView>,
    object: &serde_json::Map<String, Value>,
    setup: &mut Vec<String>,
    depth: usize,
) -> Result<Option<Rel>> {
    let Some(items) = object.get("rowsets").and_then(Value::as_array) else {
        return Ok(None);
    };
    if items.is_empty() {
        return Ok(None);
    }
    let mut rels = Vec::new();
    for item in items {
        let Some(rel) = lower_rel(app_root, datasets, item, setup, depth + 1)? else {
            return Ok(None);
        };
        rels.push(rel);
    }
    let columns = rels
        .iter()
        .find(|r| !r.columns.is_empty())
        .map(|r| r.columns.clone())
        .unwrap_or_default();
    if !columns.is_empty() {
        for rel in &rels {
            if !rel.columns.is_empty() && rel.columns != columns {
                // Mismatched schemas — not covered.
                return Ok(None);
            }
        }
        let select_list = columns
            .iter()
            .map(|c| quote_ident(c))
            .collect::<Result<Vec<_>>>()?
            .join(", ");
        let parts = rels
            .iter()
            .map(|r| format!("SELECT {select_list} FROM ({}) AS _c", r.sql))
            .collect::<Vec<_>>();
        return Ok(Some(Rel {
            sql: parts.join(" UNION ALL "),
            columns,
        }));
    }
    let parts = rels
        .iter()
        .map(|r| format!("SELECT * FROM ({}) AS _c", r.sql))
        .collect::<Vec<_>>();
    Ok(Some(Rel {
        sql: parts.join(" UNION ALL "),
        columns: Vec::new(),
    }))
}

fn lower_raw_sql(object: &serde_json::Map<String, Value>) -> Result<Option<Rel>> {
    let query = object
        .get("query")
        .or_else(|| object.get("sql"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let Some(query) = query else {
        return Ok(None);
    };
    if !is_safe_readonly_sql(query) {
        return Ok(None);
    }
    let limit = object
        .get("row_limit")
        .and_then(Value::as_u64)
        .unwrap_or(MAX_PIPELINE_SQL_ROWS as u64)
        .min(MAX_PIPELINE_SQL_ROWS as u64);
    Ok(Some(Rel {
        sql: format!("SELECT * FROM ({query}) AS _raw LIMIT {limit}"),
        columns: Vec::new(),
    }))
}

fn is_safe_readonly_sql(query: &str) -> bool {
    let upper = query.to_ascii_uppercase();
    let forbidden = [
        "INSERT ",
        "UPDATE ",
        "DELETE ",
        "DROP ",
        "ALTER ",
        "CREATE ",
        "ATTACH ",
        "COPY ",
        "EXPORT ",
        "PRAGMA ",
        "INSTALL ",
        "LOAD ",
        "CALL ",
        "EXECUTE ",
        "READ_CSV",
        "READ_PARQUET",
        "READ_JSON",
    ];
    if forbidden.iter().any(|kw| upper.contains(kw)) {
        return false;
    }
    let trimmed = upper.trim_start();
    trimmed.starts_with("SELECT") || trimmed.starts_with("WITH")
}

fn parse_years(value: Option<&Value>) -> Vec<i32> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    item.as_i64()
                        .map(|v| v as i32)
                        .or_else(|| item.as_str().and_then(|t| t.parse().ok()))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn lookup_dataset<'a>(
    datasets: &'a BTreeMap<String, DatasetView>,
    dataset_id: &str,
) -> Option<&'a DatasetView> {
    datasets.get(dataset_id).or_else(|| {
        let local = dataset_id.rsplit("::").next().unwrap_or(dataset_id);
        datasets.get(local).or_else(|| {
            datasets
                .values()
                .find(|view| view.id == dataset_id || view.id.ends_with(dataset_id))
        })
    })
}
