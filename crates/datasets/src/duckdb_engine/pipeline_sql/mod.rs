//! Compile lowered `analysis_expr` trees to controlled DuckDB SQL (0549).
//! Unsupported ops return `Ok(None)` so callers fall back to the row interpreter.

mod date_sql;
mod exec;
mod lower;

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::Result;
use mei_lang_kernel::{DatasetView, MetricContract, MetricShape};
use serde_json::Value;

use super::{record_duckdb_query_ms, record_rows_materialized};
use crate::util::elapsed_ms;
use std::time::Instant;

pub const MAX_PIPELINE_SQL_ROWS: usize = 2000;

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

/// Try to evaluate a lowered analysis_expr rowset via DuckDB SQL.
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
    let rows = exec::execute_sql_plan(app_root, &plan)?;
    if rows.len() > MAX_PIPELINE_SQL_ROWS {
        record_pipeline_sql_fallback();
        return Ok(None);
    }
    record_pipeline_sql_hit();
    record_duckdb_query_ms(elapsed_ms(started));
    record_rows_materialized(rows.len());
    Ok(Some(rows))
}

/// Evaluate dataframe/series metric defs via SQL.
///
/// Best-effort: each metric is attempted independently. Unsupported ops are
/// skipped (caller falls back to the row interpreter for those ids only).
pub fn try_eval_dataframe_metrics_via_sql(
    app_root: &Path,
    datasets: &BTreeMap<String, DatasetView>,
    metric_defs: &BTreeMap<String, Value>,
    metric_ids: &[String],
) -> Result<Option<BTreeMap<String, MetricContract>>> {
    let mut out = BTreeMap::new();
    for metric_id in metric_ids {
        let Some(raw) = metric_defs.get(metric_id) else {
            record_pipeline_sql_fallback();
            continue;
        };
        match try_eval_one_dataframe_metric(app_root, datasets, raw, metric_id)? {
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

fn try_eval_one_dataframe_metric(
    app_root: &Path,
    datasets: &BTreeMap<String, DatasetView>,
    raw: &Value,
    metric_id: &str,
) -> Result<Option<MetricContract>> {
    let Some(map) = raw.as_object() else {
        return Ok(None);
    };
    let shape_name = map.get("shape").and_then(Value::as_str).unwrap_or("dataframe");
    if matches!(shape_name, "scalar_map" | "scalar") {
        return Ok(None);
    }
    let expr = map
        .get("series")
        .or_else(|| map.get("list"))
        .or_else(|| map.get("value"))
        .unwrap_or(&Value::Null);
    let Some(rows) = try_eval_analysis_expr_via_sql(app_root, datasets, expr)? else {
        return Ok(None);
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

/// Best-effort SQL eval: returns contracts for every metric that lowered successfully.
pub fn try_eval_metrics_via_sql_partial(
    app_root: &Path,
    datasets: &BTreeMap<String, DatasetView>,
    metric_defs: &BTreeMap<String, Value>,
    metric_ids: &[String],
    global_filters: &BTreeMap<String, String>,
    search: Option<&str>,
) -> Result<BTreeMap<String, MetricContract>> {
    use super::metric_sql::{try_eval_metrics_via_sql, SqlMetricEvalInput};

    let mut out = BTreeMap::new();
    for id in metric_ids {
        let Some(raw) = metric_defs.get(id) else {
            continue;
        };
        let shape = raw
            .get("shape")
            .and_then(Value::as_str)
            .unwrap_or_else(|| {
                if raw.get("values").is_some() {
                    "scalar_map"
                } else {
                    "dataframe"
                }
            });
        if matches!(shape, "scalar_map" | "scalar") {
            if let Some(scalars) = try_eval_metrics_via_sql(SqlMetricEvalInput {
                app_root,
                datasets,
                metric_defs,
                metric_ids: std::slice::from_ref(id),
                global_filters,
                search,
            })? {
                out.extend(scalars);
            }
        } else if let Some(frames) =
            try_eval_dataframe_metrics_via_sql(app_root, datasets, metric_defs, std::slice::from_ref(id))?
        {
            out.extend(frames);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests;
