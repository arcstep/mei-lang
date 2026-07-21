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
    let sql = format!(
        "SELECT * FROM ({inner}) AS mei_pipeline_result LIMIT {}",
        MAX_PIPELINE_SQL_ROWS.saturating_add(1)
    );
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

fn project_columns(row: Value, columns: &[String]) -> Value {
    let Value::Object(map) = row else {
        return row;
    };
    let mut out = serde_json::Map::new();
    for name in columns {
        out.insert(
            name.clone(),
            map.get(name).cloned().unwrap_or(Value::Null),
        );
    }
    Value::Object(out)
}
