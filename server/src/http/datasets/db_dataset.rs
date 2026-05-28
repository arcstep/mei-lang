use std::{collections::BTreeMap, path::Path, time::Instant};

use anyhow::{anyhow, Context, Result};
use mei_lang_kernel::SourceDecl;
use rusqlite::{types::ValueRef, Connection};
use serde_json::Value;

use super::paginate::{apply_normalize, row_matches, QueryWindow};
use super::paths::resolve_db_path;
use super::types::{DatasetQueryOptions, DatasetQueryResult, SourceMeta};
use super::util::elapsed_ms;

pub(crate) fn query_db_rows(
    app_root: &Path,
    source: &SourceDecl,
    meta: &SourceMeta,
    options: &DatasetQueryOptions,
) -> Result<DatasetQueryResult> {
    let query_started = Instant::now();
    let dsn = meta
        .connection
        .clone()
        .or_else(|| {
            if source.path.is_empty() {
                None
            } else {
                Some(source.path.clone())
            }
        })
        .ok_or_else(|| anyhow!("db source missing connection/path"))?;
    let db_path = resolve_db_path(app_root, dsn.as_str());
    let conn = Connection::open(&db_path)
        .with_context(|| format!("failed to open sqlite db {}", db_path.display()))?;
    let base_sql = if let Some(query) = meta.query.as_deref().filter(|v| !v.trim().is_empty()) {
        format!("SELECT * FROM ({query})")
    } else if let Some(table) = meta.table.as_deref().filter(|v| !v.trim().is_empty()) {
        format!("SELECT * FROM \"{}\"", table.replace('"', "\"\""))
    } else {
        return Err(anyhow!("db source needs table or query"));
    };
    let offset = options.page.saturating_sub(1) * options.page_size;
    let no_filters = options.filters.is_empty()
        && options
            .search
            .as_deref()
            .map(str::trim)
            .map(|value| value.is_empty())
            .unwrap_or(true);
    if no_filters {
        let sql = if options.collect_all {
            base_sql.clone()
        } else {
            format!(
                "{base_sql} LIMIT {} OFFSET {}",
                options.page_size.saturating_add(1),
                offset
            )
        };
        let mut stmt = conn.prepare(&sql)?;
        let columns = stmt
            .column_names()
            .iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>();
        let mut rows = stmt
            .query_map([], |row| db_row_to_value(row, &columns))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        for row in &mut rows {
            *row = apply_normalize(std::mem::take(row), &meta.normalize);
        }
        let has_more = !options.collect_all && rows.len() > options.page_size;
        if has_more {
            rows.truncate(options.page_size);
        }
        let total = if options.collect_all {
            rows.len()
        } else {
            offset + rows.len() + usize::from(has_more)
        };
        let mut result = DatasetQueryResult {
            page: if options.collect_all { 1 } else { options.page },
            page_size: if options.collect_all {
                rows.len()
            } else {
                options.page_size
            },
            total,
            has_more,
            columns,
            rows,
            lazy: true,
            perf: BTreeMap::new(),
            column_meta: Vec::new(),
            summary: None,
            query_state_echo: None,
        };
        result
            .perf
            .insert("db_query_window_ms".to_string(), elapsed_ms(query_started));
        return Ok(result);
    }
    let mut stmt = conn.prepare(&base_sql)?;
    let columns = stmt
        .column_names()
        .iter()
        .map(|value| value.to_string())
        .collect::<Vec<_>>();
    let mapped = stmt.query_map([], |row| db_row_to_value(row, &columns))?;
    let mut window = QueryWindow::new(options);
    for row in mapped {
        if window.should_stop() {
            break;
        }
        let normalized = apply_normalize(row?, &meta.normalize);
        if row_matches(&normalized, &options.filters, options.search.as_deref()) {
            window.push(normalized);
        }
    }
    let mut result = window.finish(columns, true);
    result
        .perf
        .insert("db_query_filter_ms".to_string(), elapsed_ms(query_started));
    Ok(result)
}

fn db_row_to_value(row: &rusqlite::Row<'_>, columns: &[String]) -> Result<Value, rusqlite::Error> {
    let mut map = serde_json::Map::new();
    for (index, column) in columns.iter().enumerate() {
        let value = match row.get_ref(index)? {
            ValueRef::Null => Value::Null,
            ValueRef::Integer(v) => serde_json::json!(v),
            ValueRef::Real(v) => serde_json::json!(v),
            ValueRef::Text(v) => Value::String(String::from_utf8_lossy(v).to_string()),
            ValueRef::Blob(v) => Value::String(format!("<blob:{} bytes>", v.len())),
        };
        map.insert(column.clone(), value);
    }
    Ok(Value::Object(map))
}
