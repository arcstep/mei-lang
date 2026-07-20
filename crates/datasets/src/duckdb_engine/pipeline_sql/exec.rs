use std::path::Path;

use anyhow::{Context, Result};
use serde_json::{json, Map, Value};

use super::super::connection::with_app_connection;
use super::lower::SqlPlan;
use super::MAX_PIPELINE_SQL_ROWS;

pub fn execute_sql_plan(app_root: &Path, plan: &SqlPlan) -> Result<Vec<Value>> {
    for ddl in &plan.setup_ddls {
        with_app_connection(app_root, |conn| {
            conn.execute_batch(ddl)
                .with_context(|| format!("pipeline sql setup: {ddl}"))?;
            Ok(())
        })?;
    }
    let inner = plan.final_sql.trim_end_matches(';');
    let col_names = if plan.result_columns.is_empty() {
        let describe = format!("SELECT * FROM ({inner}) AS _mei_desc LIMIT 0");
        with_app_connection(app_root, |conn| {
            let stmt = conn
                .prepare(&describe)
                .with_context(|| format!("describe pipeline sql: {describe}"))?;
            Ok(stmt.column_names())
        })?
    } else {
        plan.result_columns.clone()
    };
    let sql = format!("SELECT * FROM ({inner}) AS mei_pipeline_result LIMIT {}", MAX_PIPELINE_SQL_ROWS.saturating_add(1));
    with_app_connection(app_root, |conn| {
        let mut stmt = conn
            .prepare(&sql)
            .with_context(|| format!("prepare pipeline sql: {sql}"))?;
        let mut rows = Vec::new();
        let mut rows_iter = stmt.query([])?;
        while let Some(row) = rows_iter.next()? {
            let mut map = Map::new();
            for (idx, name) in col_names.iter().enumerate() {
                map.insert(name.clone(), duck_value_to_json(row, idx)?);
            }
            rows.push(Value::Object(map));
        }
        Ok(rows)
    })
}

fn duck_value_to_json(row: &duckdb::Row<'_>, idx: usize) -> Result<Value> {
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
