//! Compile lowered `analysis_expr` trees to controlled DataFusion SQL (0549).
//! Unsupported ops return `Ok(None)` so callers **fail-fast** (no whole-table JSON hydrate).

mod date_sql;
mod exec;
mod lower;

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{bail, Result};
use mei_lang_kernel::{DatasetView, MetricContract, MetricShape};
use serde_json::Value;

use super::{record_query_engine_ms, record_rows_materialized};
use crate::util::elapsed_ms;
use std::time::Instant;

pub const MAX_PIPELINE_SQL_ROWS: usize = 2000;
pub use exec::{MAX_COLUMN_FACET_VALUES, MAX_FACET_COLUMNS_PER_QUERY};

static PIPELINE_SQL_HIT: AtomicU64 = AtomicU64::new(0);
static PIPELINE_SQL_FALLBACK: AtomicU64 = AtomicU64::new(0);

pub fn record_pipeline_sql_hit() {
    PIPELINE_SQL_HIT.fetch_add(1, Ordering::Relaxed);
}

pub fn record_pipeline_sql_fallback() {
    PIPELINE_SQL_FALLBACK.fetch_add(1, Ordering::Relaxed);
}

pub fn take_pipeline_sql_stats() -> (u64, u64) {
    (
        PIPELINE_SQL_HIT.swap(0, Ordering::Relaxed),
        PIPELINE_SQL_FALLBACK.swap(0, Ordering::Relaxed),
    )
}

pub fn snapshot_pipeline_sql_stats() -> (u64, u64) {
    (
        PIPELINE_SQL_HIT.load(Ordering::Relaxed),
        PIPELINE_SQL_FALLBACK.load(Ordering::Relaxed),
    )
}

/// Try to evaluate a lowered analysis_expr rowset via DataFusion SQL.
pub fn try_eval_analysis_expr_via_sql(
    app_root: &Path,
    datasets: &BTreeMap<String, DatasetView>,
    expr: &Value,
) -> Result<Option<Vec<Value>>> {
    let started = Instant::now();
    let Some(plan) = lower::try_lower_expr(app_root, datasets, expr)? else {
        record_pipeline_sql_fallback();
        return Ok(None);
    };
    let rows = match exec::execute_sql_plan(app_root, &plan) {
        Ok(rows) => rows,
        Err(err) => {
            tracing::debug!(error = %err, "pipeline_sql DataFusion exec fallback");
            record_pipeline_sql_fallback();
            return Ok(None);
        }
    };
    if rows.len() > MAX_PIPELINE_SQL_ROWS {
        record_pipeline_sql_fallback();
        bail!(
            "pipeline_sql_row_limit: result has {} rows (max {}); whole-table JSON hydrate is disabled",
            rows.len(),
            MAX_PIPELINE_SQL_ROWS
        );
    }
    record_pipeline_sql_hit();
    record_query_engine_ms(elapsed_ms(started));
    record_rows_materialized(rows.len());
    Ok(Some(rows))
}

#[derive(Debug, Clone)]
pub struct PipelineSqlPage {
    pub total: usize,
    pub rows: Vec<Value>,
    pub has_more: bool,
    pub page: usize,
    pub page_size: usize,
    pub columns: Vec<String>,
    pub column_facets: BTreeMap<String, Vec<crate::types::TableColumnFacet>>,
}

/// Paginate a dataframe metric via SQL (COUNT + LIMIT/OFFSET), bypassing the 2000-row
/// whole-table materialize gate. Used by drilldown `__scalar_rowset__` tables (0528).
pub fn try_page_dataframe_metric_via_sql(
    app_root: &Path,
    datasets: &BTreeMap<String, DatasetView>,
    metric_defs: &BTreeMap<String, Value>,
    metric_id: &str,
    filters: &BTreeMap<String, String>,
    search: Option<&str>,
    page: usize,
    page_size: usize,
    sort: &[crate::table_contract::TableSortSpec],
    facet_columns: &[String],
) -> Result<Option<PipelineSqlPage>> {
    let Some(raw) = lookup_metric_def_for_sql(metric_defs, metric_id) else {
        record_pipeline_sql_fallback();
        return Ok(None);
    };
    let Some(map) = raw.as_object() else {
        record_pipeline_sql_fallback();
        return Ok(None);
    };
    let shape_name = map
        .get("shape")
        .and_then(Value::as_str)
        .unwrap_or("dataframe");
    if matches!(shape_name, "scalar_map" | "scalar") {
        record_pipeline_sql_fallback();
        return Ok(None);
    }
    let expr = map
        .get("series")
        .or_else(|| map.get("list"))
        .or_else(|| map.get("value"))
        .unwrap_or(&Value::Null);
    let mut stack = Vec::new();
    let Some(inlined) = inline_metric_refs_for_sql(expr, metric_defs, 0, &mut stack) else {
        record_pipeline_sql_fallback();
        return Ok(None);
    };
    let started = Instant::now();
    let Some(plan) = lower::try_lower_expr(app_root, datasets, &inlined)? else {
        record_pipeline_sql_fallback();
        return Ok(None);
    };
    let columns = if plan.result_columns.is_empty() {
        map.get("schema")
            .and_then(|value| value.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| {
                        item.get("name")
                            .and_then(Value::as_str)
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    } else {
        plan.result_columns.clone()
    };
    let where_sql = super::sql::build_where_clause(filters, search, &columns)?;
    let order_by_sql = build_order_by_sql(sort)?;
    let page = page.max(1);
    let page_size = page_size.max(1);
    let paged = match exec::execute_sql_plan_page(
        app_root,
        &plan,
        &where_sql,
        &order_by_sql,
        page,
        page_size,
    ) {
        Ok(page) => page,
        Err(err) if !order_by_sql.is_empty() => {
            // 复杂序号 ORDER BY（regexp）与 WHERE 组合可能触发 DataFusion SanityCheck；
            // 降级为无 ORDER BY 重试，保留 filters，避免落到无过滤物化。
            tracing::debug!(
                error = %err,
                "pipeline_sql DataFusion page retry without ORDER BY"
            );
            match exec::execute_sql_plan_page(app_root, &plan, &where_sql, "", page, page_size) {
                Ok(page) => page,
                Err(err2) => {
                    tracing::debug!(error = %err2, "pipeline_sql DataFusion page fallback");
                    record_pipeline_sql_fallback();
                    return Ok(None);
                }
            }
        }
        Err(err) => {
            tracing::debug!(error = %err, "pipeline_sql DataFusion page fallback");
            record_pipeline_sql_fallback();
            return Ok(None);
        }
    };
    let mut column_facets = BTreeMap::new();
    for column in facet_columns.iter().take(exec::MAX_FACET_COLUMNS_PER_QUERY) {
        let name = column.trim();
        if name.is_empty() {
            continue;
        }
        match exec::execute_sql_plan_facets(
            app_root,
            &plan,
            name,
            &where_sql,
            exec::MAX_COLUMN_FACET_VALUES,
        ) {
            Ok(values) => {
                column_facets.insert(name.to_string(), values);
            }
            Err(err) => {
                tracing::debug!(
                    column = %name,
                    error = %err,
                    "pipeline_sql facet group skipped"
                );
            }
        }
    }
    record_pipeline_sql_hit();
    record_query_engine_ms(elapsed_ms(started));
    record_rows_materialized(paged.rows.len());
    Ok(Some(PipelineSqlPage {
        total: paged.total,
        rows: paged.rows,
        has_more: paged.has_more,
        page,
        page_size,
        columns,
        column_facets,
    }))
}

fn build_order_by_sql(sort: &[crate::table_contract::TableSortSpec]) -> Result<String> {
    if sort.is_empty() {
        return Ok(String::new());
    }
    let mut parts = Vec::new();
    for item in sort {
        let field = item.field.trim();
        if field.is_empty() {
            continue;
        }
        let col = super::sql::quote_ident(field)?;
        let dir = if item.direction.eq_ignore_ascii_case("desc") {
            "DESC"
        } else {
            "ASC"
        };
        if crate::paginate::is_serial_number_field(field) {
            // 对齐内存段排序：空值最后；无数字文本（如「待完善」）排在数字序号之后。
            parts.push(format!(
                "CASE WHEN trim(CAST({col} AS VARCHAR)) = '' OR {col} IS NULL THEN 1 ELSE 0 END {dir}"
            ));
            parts.push(format!(
                "CASE WHEN regexp_match(CAST({col} AS VARCHAR), '\\d') IS NULL THEN 1 ELSE 0 END {dir}"
            ));
            parts.push(format!(
                "concat(\
lpad(coalesce((regexp_match(CAST({col} AS VARCHAR), '^(\\d+)'))[1], '0'), 10, '0'), \
coalesce(concat('-', lpad((regexp_match(CAST({col} AS VARCHAR), '^\\d+-(\\d+)'))[1], 10, '0')), ''), \
coalesce(concat('-', lpad((regexp_match(CAST({col} AS VARCHAR), '^\\d+-\\d+-(\\d+)'))[1], 10, '0')), '')\
) {dir}"
            ));
            parts.push(format!("CAST({col} AS VARCHAR) {dir}"));
        } else {
            parts.push(format!("{col} {dir}"));
        }
    }
    if parts.is_empty() {
        Ok(String::new())
    } else {
        Ok(format!(" ORDER BY {}", parts.join(", ")))
    }
}

/// Count rows for a lowered analysis_expr without materializing the rowset.
pub fn try_count_analysis_expr_via_sql(
    app_root: &Path,
    datasets: &BTreeMap<String, DatasetView>,
    expr: &Value,
) -> Result<Option<i64>> {
    let started = Instant::now();
    let Some(plan) = lower::try_lower_expr(app_root, datasets, expr)? else {
        record_pipeline_sql_fallback();
        return Ok(None);
    };
    let count = match exec::execute_sql_plan_count(app_root, &plan) {
        Ok(n) => n,
        Err(err) => {
            tracing::debug!(error = %err, "pipeline_sql DataFusion count fallback");
            record_pipeline_sql_fallback();
            return Ok(None);
        }
    };
    record_pipeline_sql_hit();
    record_query_engine_ms(elapsed_ms(started));
    Ok(Some(count))
}

/// Aggregate a numeric field over a lowered analysis_expr rowset.
pub fn try_agg_analysis_expr_via_sql(
    app_root: &Path,
    datasets: &BTreeMap<String, DatasetView>,
    expr: &Value,
    field: &str,
    agg: &str,
) -> Result<Option<f64>> {
    let started = Instant::now();
    let Some(plan) = lower::try_lower_expr(app_root, datasets, expr)? else {
        record_pipeline_sql_fallback();
        return Ok(None);
    };
    let value = match exec::execute_sql_plan_agg_f64(app_root, &plan, field, agg) {
        Ok(n) => n,
        Err(err) => {
            tracing::debug!(error = %err, "pipeline_sql DataFusion agg fallback");
            record_pipeline_sql_fallback();
            return Ok(None);
        }
    };
    record_pipeline_sql_hit();
    record_query_engine_ms(elapsed_ms(started));
    Ok(Some(value))
}

/// Evaluate dataframe/series metric defs via SQL.
///
/// Best-effort: each metric is attempted independently. Unsupported ops are
/// skipped (caller must fail-fast — no whole-table JSON hydrate).
/// When `allow_paged` is true, results exceeding [`MAX_PIPELINE_SQL_ROWS`] are
/// materialized via SQL pagination (artifact / warmup Pack-First path).
pub fn try_eval_dataframe_metrics_via_sql(
    app_root: &Path,
    datasets: &BTreeMap<String, DatasetView>,
    metric_defs: &BTreeMap<String, Value>,
    metric_ids: &[String],
) -> Result<Option<BTreeMap<String, MetricContract>>> {
    try_eval_dataframe_metrics_via_sql_opts(app_root, datasets, metric_defs, metric_ids, false)
}

/// Like [`try_eval_dataframe_metrics_via_sql`], with optional paged oversize materialize.
pub fn try_eval_dataframe_metrics_via_sql_opts(
    app_root: &Path,
    datasets: &BTreeMap<String, DatasetView>,
    metric_defs: &BTreeMap<String, Value>,
    metric_ids: &[String],
    allow_paged: bool,
) -> Result<Option<BTreeMap<String, MetricContract>>> {
    let mut out = BTreeMap::new();
    for metric_id in metric_ids {
        let Some(raw) = metric_defs.get(metric_id) else {
            record_pipeline_sql_fallback();
            continue;
        };
        match try_eval_one_dataframe_metric(
            app_root,
            datasets,
            metric_defs,
            raw,
            metric_id,
            allow_paged,
        )? {
            Some(contract) => {
                out.insert(metric_id.clone(), contract);
            }
            None => {
                record_pipeline_sql_fallback();
            }
        }
    }
    if out.is_empty() {
        return Ok(None);
    }
    Ok(Some(out))
}

/// Inline `{ "__ref": "metric", "id": "…" }` to the referenced dataframe analysis_expr.
/// Board explain charts hoist `group_by` over `__scalar_rowset__` via metric refs; pipeline SQL
/// only understands analysis_expr / `__ref: data`, so we expand refs before lowering.
fn inline_metric_refs_for_sql(
    expr: &Value,
    metric_defs: &BTreeMap<String, Value>,
    depth: usize,
    stack: &mut Vec<String>,
) -> Option<Value> {
    if depth > 32 {
        return None;
    }
    match expr {
        Value::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(inline_metric_refs_for_sql(
                    item,
                    metric_defs,
                    depth + 1,
                    stack,
                )?);
            }
            Some(Value::Array(out))
        }
        Value::Object(map) => {
            if map.get("__ref").and_then(Value::as_str) == Some("metric") {
                let id = map
                    .get("id")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())?;
                if stack.iter().any(|seen| seen == id) {
                    return None;
                }
                let def = lookup_metric_def_for_sql(metric_defs, id)?;
                let inner = def
                    .get("value")
                    .or_else(|| def.get("series"))
                    .or_else(|| def.get("list"))?;
                stack.push(id.to_string());
                let inlined = inline_metric_refs_for_sql(inner, metric_defs, depth + 1, stack);
                stack.pop();
                return inlined;
            }
            let mut out = serde_json::Map::new();
            for (key, child) in map {
                out.insert(
                    key.clone(),
                    inline_metric_refs_for_sql(child, metric_defs, depth + 1, stack)?,
                );
            }
            Some(Value::Object(out))
        }
        other => Some(other.clone()),
    }
}

fn lookup_metric_def_for_sql<'a>(
    metric_defs: &'a BTreeMap<String, Value>,
    metric_id: &str,
) -> Option<&'a Value> {
    if let Some(def) = metric_defs.get(metric_id) {
        return Some(def);
    }
    let suffix = format!("::{metric_id}");
    metric_defs.iter().find_map(|(key, def)| {
        if key == metric_id || key.ends_with(&suffix) || key.rsplit("::").next() == Some(metric_id)
        {
            Some(def)
        } else {
            None
        }
    })
}

fn try_eval_one_dataframe_metric(
    app_root: &Path,
    datasets: &BTreeMap<String, DatasetView>,
    metric_defs: &BTreeMap<String, Value>,
    raw: &Value,
    metric_id: &str,
    allow_paged: bool,
) -> Result<Option<MetricContract>> {
    let Some(map) = raw.as_object() else {
        return Ok(None);
    };
    let shape_name = map
        .get("shape")
        .and_then(Value::as_str)
        .unwrap_or("dataframe");
    if matches!(shape_name, "scalar_map" | "scalar") {
        return Ok(None);
    }
    let expr = map
        .get("series")
        .or_else(|| map.get("list"))
        .or_else(|| map.get("value"))
        .unwrap_or(&Value::Null);
    let mut stack = Vec::new();
    let Some(inlined) = inline_metric_refs_for_sql(expr, metric_defs, 0, &mut stack) else {
        return Ok(None);
    };
    let rows = if allow_paged {
        match try_eval_analysis_expr_via_sql(app_root, datasets, &inlined) {
            Ok(Some(rows)) => rows,
            Ok(None) => return Ok(None),
            Err(err) if err.to_string().contains("pipeline_sql_row_limit") => {
                let Some(rows) = materialize_dataframe_metric_via_sql_pages(
                    app_root,
                    datasets,
                    metric_defs,
                    metric_id,
                )?
                else {
                    return Err(err);
                };
                rows
            }
            Err(err) => return Err(err),
        }
    } else {
        let Some(rows) = try_eval_analysis_expr_via_sql(app_root, datasets, &inlined)? else {
            return Ok(None);
        };
        rows
    };
    let schema = map
        .get("schema")
        .and_then(|value| serde_json::from_value(value.clone()).ok())
        .unwrap_or_default();
    let shape = match shape_name {
        "series" => MetricShape::Series,
        "table" => MetricShape::Table,
        _ => MetricShape::Dataframe,
    };
    Ok(Some(MetricContract {
        id: metric_id.to_string(),
        label: map
            .get("label")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        unit: map
            .get("unit")
            .and_then(Value::as_str)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        value_format: map.get("value_format").cloned(),
        purpose: None,
        shape,
        schema,
        dataset: map
            .get("dataset")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        transforms: Vec::new(),
        value: Value::Array(rows),
    }))
}

/// Page through a dataframe metric until exhausted (artifact / warmup oversize path).
fn materialize_dataframe_metric_via_sql_pages(
    app_root: &Path,
    datasets: &BTreeMap<String, DatasetView>,
    metric_defs: &BTreeMap<String, Value>,
    metric_id: &str,
) -> Result<Option<Vec<Value>>> {
    let empty_filters = BTreeMap::new();
    let page_size = MAX_PIPELINE_SQL_ROWS.max(1);
    let mut page = 1usize;
    let mut all_rows = Vec::new();
    // Hard cap pages so a runaway total cannot loop forever (~2M rows).
    const MAX_PAGES: usize = 1024;
    for _ in 0..MAX_PAGES {
        let Some(paged) = try_page_dataframe_metric_via_sql(
            app_root,
            datasets,
            metric_defs,
            metric_id,
            &empty_filters,
            None,
            page,
            page_size,
            &[],
            &[],
        )?
        else {
            return Ok(None);
        };
        all_rows.extend(paged.rows);
        if !paged.has_more {
            tracing::info!(
                metric_id = %metric_id,
                rows = all_rows.len(),
                pages = page,
                "pipeline_sql_paged_artifact_materialize"
            );
            return Ok(Some(all_rows));
        }
        page = page.saturating_add(1);
    }
    bail!(
        "pipeline_sql_row_limit: paged materialize exceeded {} pages for metric_id={}",
        MAX_PAGES,
        metric_id
    );
}

/// Outcome of best-effort SQL KPI/dataframe eval (partial hits + miss diagnostics).
#[derive(Debug, Default)]
pub struct PipelineSqlPartialResult {
    pub metrics: BTreeMap<String, MetricContract>,
    /// metric_id → short reason (`pipeline_sql_lower_failed`, `pipeline_sql_row_limit`, …)
    pub miss_reasons: BTreeMap<String, String>,
}

/// Best-effort SQL eval: returns contracts for every metric that lowered successfully.
/// Dataframe metrics use paged materialize when the whole-table row gate trips
/// (warmup / lite-artifact cold path).
pub fn try_eval_metrics_via_sql_partial(
    app_root: &Path,
    datasets: &BTreeMap<String, DatasetView>,
    metric_defs: &BTreeMap<String, Value>,
    metric_ids: &[String],
    global_filters: &BTreeMap<String, String>,
    search: Option<&str>,
) -> Result<PipelineSqlPartialResult> {
    use super::metric_sql::{try_eval_metrics_via_sql, SqlMetricEvalInput};

    let mut out = PipelineSqlPartialResult::default();
    for id in metric_ids {
        let Some(raw) = metric_defs.get(id) else {
            out.miss_reasons
                .insert(id.clone(), "metric_def_missing".to_string());
            continue;
        };
        let shape = raw.get("shape").and_then(Value::as_str).unwrap_or_else(|| {
            if raw.get("values").is_some() {
                "scalar_map"
            } else {
                "dataframe"
            }
        });
        if matches!(shape, "scalar_map" | "scalar") {
            match try_eval_metrics_via_sql(SqlMetricEvalInput {
                app_root,
                datasets,
                metric_defs,
                metric_ids: std::slice::from_ref(id),
                global_filters,
                search,
            }) {
                Ok(Some(scalars)) if scalars.contains_key(id) => {
                    out.metrics.extend(scalars);
                }
                Ok(_) => {
                    out.miss_reasons
                        .insert(id.clone(), "pipeline_sql_lower_failed".to_string());
                }
                Err(err) => {
                    let reason = if err.to_string().contains("pipeline_sql_row_limit") {
                        "pipeline_sql_row_limit"
                    } else {
                        "pipeline_sql_eval_failed"
                    };
                    out.miss_reasons.insert(id.clone(), reason.to_string());
                }
            }
        } else {
            match try_eval_one_dataframe_metric(
                app_root,
                datasets,
                metric_defs,
                raw,
                id,
                true, // KPI/artifact path: page oversize instead of aborting the batch
            ) {
                Ok(Some(contract)) => {
                    out.metrics.insert(id.clone(), contract);
                }
                Ok(None) => {
                    out.miss_reasons
                        .insert(id.clone(), "pipeline_sql_lower_failed".to_string());
                    record_pipeline_sql_fallback();
                }
                Err(err) => {
                    let message = err.to_string();
                    let reason = if message.contains("pipeline_sql_row_limit") {
                        "pipeline_sql_row_limit"
                    } else {
                        "pipeline_sql_eval_failed"
                    };
                    out.miss_reasons.insert(id.clone(), reason.to_string());
                    // Do not fail the whole partial batch — caller fail-fasts uncovered ids.
                    tracing::warn!(
                        metric_id = %id,
                        error = %message,
                        "pipeline_sql dataframe metric failed during partial eval"
                    );
                }
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests;
