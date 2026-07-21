use std::path::Path;

use anyhow::{Context, Result};
use serde_json::Value;

use super::super::arrow_json::batches_to_json_rows;
use super::super::connection::{block_on, with_app_session};
use super::lower::SqlPlan;
use super::MAX_PIPELINE_SQL_ROWS;

pub fn execute_sql_plan(app_root: &Path, plan: &SqlPlan) -> Result<Vec<Value>> {
    for ddl in &plan.setup_ddls {
        with_app_session(app_root, |ctx| {
            block_on(async {
                let _ = ctx
                    .sql(ddl)
                    .await
                    .with_context(|| format!("pipeline sql setup: {ddl}"))?
                    .collect()
                    .await
                    .with_context(|| format!("collect pipeline setup: {ddl}"))?;
                Ok::<(), anyhow::Error>(())
            })
        })?;
    }
    let inner = plan.final_sql.trim_end_matches(';');
    // WITH ... SELECT must stay at statement head; do not wrap in SELECT * FROM (...).
    let sql = if inner.trim_start().len() >= 4
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
    with_app_session(app_root, |ctx| {
        block_on(async {
            let batches = ctx
                .sql(&sql)
                .await
                .with_context(|| format!("prepare pipeline sql: {sql}"))?
                .collect()
                .await
                .with_context(|| format!("collect pipeline sql: {sql}"))?;
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
    let col = super::super::sql::quote_ident(field)?;
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
    for ddl in &plan.setup_ddls {
        with_app_session(app_root, |ctx| {
            block_on(async {
                let _ = ctx
                    .sql(ddl)
                    .await
                    .with_context(|| format!("pipeline sql setup: {ddl}"))?
                    .collect()
                    .await
                    .with_context(|| format!("collect pipeline setup: {ddl}"))?;
                Ok::<(), anyhow::Error>(())
            })
        })?;
    }
    let inner = plan.final_sql.trim_end_matches(';');
    let sql = sql_template.replace("{inner}", inner);
    with_app_session(app_root, |ctx| {
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
