//! PostgreSQL / TimescaleDB dataset query via `tokio-postgres`.
//! Connection comes from per-app `ops.sources.*.connection` (`env:NAME` or DSN).
//! Uses the query-engine `block_on` helper so Host/warmup Tokio contexts never nest
//! a sync `postgres` runtime.
//!
//! Performance notes:
//! - Pool key = DSN (one live client per DSN); avoid connect-per-query.
//! - Short in-memory result cache for identical collect_all queries (KPI / bridge).

use std::{
    collections::{BTreeMap, HashMap},
    env,
    path::Path,
    sync::{Arc, Mutex as StdMutex, OnceLock},
    time::{Duration, Instant},
};

use anyhow::{anyhow, Context, Result};
use mei_lang_kernel::SourceDecl;
use moka::sync::Cache;
use serde_json::{json, Value};
use tokio::sync::Mutex as AsyncMutex;
use tokio_postgres::types::Type;
use tokio_postgres::{Client, NoTls, Row};

use super::paginate::{apply_normalize, paginate_rows, row_matches, QueryWindow};
use super::query_engine::block_on;
use super::types::{DatasetQueryOptions, DatasetQueryResult, SourceMeta};
use super::util::elapsed_ms;

type ClientSlot = Arc<AsyncMutex<Client>>;

const RESULT_CACHE_TTL_SECS: u64 = 15;
const RESULT_CACHE_MAX: u64 = 64;

pub fn is_postgres_kind(kind: &str) -> bool {
    matches!(
        kind.trim().to_ascii_lowercase().as_str(),
        "postgres" | "postgresql" | "timescale" | "timescaledb"
    )
}

/// Resolve `env:VAR` or return the raw DSN string.
pub fn resolve_connection_dsn(raw: &str) -> Result<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("postgres connection is empty"));
    }
    if let Some(name) = trimmed.strip_prefix("env:") {
        let key = name.trim();
        if key.is_empty() {
            return Err(anyhow!("postgres connection env: name is empty"));
        }
        return env::var(key).with_context(|| {
            format!("environment variable `{key}` not set for postgres connection")
        });
    }
    Ok(trimmed.to_string())
}

fn redact_dsn(dsn: &str) -> String {
    if let Some(at) = dsn.find('@') {
        if let Some(scheme_end) = dsn.find("://") {
            let rest = &dsn[scheme_end + 3..at];
            if let Some(colon) = rest.find(':') {
                return format!(
                    "{}{}:***{}",
                    &dsn[..scheme_end + 3],
                    &rest[..colon],
                    &dsn[at..]
                );
            }
        }
    }
    dsn.to_string()
}

fn pool_map() -> &'static StdMutex<HashMap<String, ClientSlot>> {
    static POOL: OnceLock<StdMutex<HashMap<String, ClientSlot>>> = OnceLock::new();
    POOL.get_or_init(|| StdMutex::new(HashMap::new()))
}

fn result_cache() -> &'static Cache<String, DatasetQueryResult> {
    static CACHE: OnceLock<Cache<String, DatasetQueryResult>> = OnceLock::new();
    CACHE.get_or_init(|| {
        Cache::builder()
            .max_capacity(RESULT_CACHE_MAX)
            .time_to_live(Duration::from_secs(RESULT_CACHE_TTL_SECS))
            .build()
    })
}

fn cache_key(dsn: &str, sql: &str, options: &DatasetQueryOptions, meta: &SourceMeta) -> String {
    format!(
        "{dsn}|{sql}|page={}|ps={}|all={}|sort={}|filters={:?}|search={:?}|norm={:?}",
        options.page,
        options.page_size,
        options.collect_all,
        options.sort.len(),
        options.filters,
        options.search,
        meta.normalize
    )
}

async fn connect_client(dsn: &str) -> Result<Client> {
    let started = Instant::now();
    let (client, connection) = tokio_postgres::connect(dsn, NoTls)
        .await
        .with_context(|| format!("failed to connect postgres ({})", redact_dsn(dsn)))?;
    tokio::spawn(async move {
        if let Err(err) = connection.await {
            tracing::debug!(error = %err, "postgres connection task ended");
        }
    });
    tracing::debug!(
        dsn = %redact_dsn(dsn),
        connect_ms = elapsed_ms(started),
        "postgres connected"
    );
    Ok(client)
}

async fn client_slot_for(dsn: &str) -> Result<ClientSlot> {
    {
        let guard = pool_map()
            .lock()
            .map_err(|_| anyhow!("postgres connection pool poisoned"))?;
        if let Some(slot) = guard.get(dsn) {
            return Ok(slot.clone());
        }
    }
    let client = connect_client(dsn).await?;
    let slot = Arc::new(AsyncMutex::new(client));
    let mut guard = pool_map()
        .lock()
        .map_err(|_| anyhow!("postgres connection pool poisoned"))?;
    Ok(guard.entry(dsn.to_string()).or_insert(slot).clone())
}

fn drop_pooled_client(dsn: &str) {
    if let Ok(mut guard) = pool_map().lock() {
        guard.remove(dsn);
    }
}

/// Drop pooled clients for one app prefix or all.
pub fn clear_postgres_pool(app_root: Option<&Path>) -> usize {
    result_cache().invalidate_all();
    let Ok(mut guard) = pool_map().lock() else {
        return 0;
    };
    match app_root {
        None => {
            let n = guard.len();
            guard.clear();
            n
        }
        Some(_) => {
            // Pool is keyed by DSN only (not app_root); clear all on app reload.
            let n = guard.len();
            guard.clear();
            n
        }
    }
}

fn base_sql(meta: &SourceMeta, source: &SourceDecl) -> Result<String> {
    if let Some(query) = meta
        .query
        .as_deref()
        .or(source.query.as_deref())
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        let trimmed = query.trim_start();
        let lower = trimmed.to_ascii_lowercase();
        if lower.starts_with("select") || lower.starts_with("with") {
            return Ok(query.to_string());
        }
        return Ok(format!("SELECT * FROM ({query}) AS _mei_pg_q"));
    }
    if let Some(table) = meta
        .table
        .as_deref()
        .or(source.table.as_deref())
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        let parts = table
            .split('.')
            .map(|p| format!("\"{}\"", p.replace('"', "\"\"")))
            .collect::<Vec<_>>();
        return Ok(format!("SELECT * FROM {}", parts.join(".")));
    }
    Err(anyhow!("postgres source needs table or query"))
}

fn connection_raw(meta: &SourceMeta, source: &SourceDecl) -> Result<String> {
    meta.connection
        .clone()
        .or_else(|| source.connection.clone())
        .or_else(|| {
            let path = source.path.trim();
            if path.starts_with("postgres://")
                || path.starts_with("postgresql://")
                || path.starts_with("env:")
            {
                Some(path.to_string())
            } else {
                None
            }
        })
        .ok_or_else(|| anyhow!("postgres source missing connection"))
}

pub(crate) fn query_postgres_rows(
    app_root: &Path,
    source: &SourceDecl,
    meta: &SourceMeta,
    options: &DatasetQueryOptions,
) -> Result<DatasetQueryResult> {
    let _ = app_root;
    block_on(query_postgres_rows_async(
        source.clone(),
        meta.clone(),
        options.clone(),
    ))
}

async fn query_postgres_rows_async(
    source: SourceDecl,
    meta: SourceMeta,
    options: DatasetQueryOptions,
) -> Result<DatasetQueryResult> {
    let query_started = Instant::now();
    let dsn = resolve_connection_dsn(&connection_raw(&meta, &source)?)?;
    let sql = base_sql(&meta, &source)?;
    let key = cache_key(&dsn, &sql, &options, &meta);
    if let Some(hit) = result_cache().get(&key) {
        tracing::debug!(
            rows = hit.rows.len(),
            total_ms = elapsed_ms(query_started),
            "postgres query cache hit"
        );
        return Ok(hit);
    }

    let offset = options.page.saturating_sub(1) * options.page_size;
    let no_filters = options.filters.is_empty()
        && options
            .search
            .as_deref()
            .map(str::trim)
            .map(|value| value.is_empty())
            .unwrap_or(true);

    let result = match run_query_with_pool(&dsn, &sql, &meta, &options, offset, no_filters).await {
        Ok(v) => v,
        Err(err) => {
            drop_pooled_client(&dsn);
            return Err(err);
        }
    };

    result_cache().insert(key, result.clone());
    tracing::debug!(
        rows = result.rows.len(),
        total_ms = elapsed_ms(query_started),
        cached = true,
        "postgres query done"
    );
    Ok(result)
}

async fn run_query_with_pool(
    dsn: &str,
    sql: &str,
    meta: &SourceMeta,
    options: &DatasetQueryOptions,
    offset: usize,
    no_filters: bool,
) -> Result<DatasetQueryResult> {
    let query_started = Instant::now();
    let slot = client_slot_for(dsn).await?;
    let client = slot.lock().await;

    if no_filters && options.sort.is_empty() {
        let limited = if options.collect_all {
            sql.to_string()
        } else {
            format!(
                "{sql} LIMIT {} OFFSET {}",
                options.page_size.saturating_add(1),
                offset
            )
        };
        let stmt = client
            .prepare(limited.as_str())
            .await
            .with_context(|| "postgres prepare failed")?;
        let columns = stmt
            .columns()
            .iter()
            .map(|c| c.name().to_string())
            .collect::<Vec<_>>();
        let rows_pg = client
            .query(&stmt, &[])
            .await
            .with_context(|| "postgres query failed")?;
        let mut rows = rows_pg
            .iter()
            .map(|row| pg_row_to_value(row, &columns))
            .collect::<Vec<_>>();
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
            column_facets: BTreeMap::new(),
        };
        result
            .perf
            .insert("pg_query_window_ms".to_string(), elapsed_ms(query_started));
        return Ok(result);
    }

    let stmt = client
        .prepare(sql)
        .await
        .with_context(|| "postgres prepare failed")?;
    let columns = stmt
        .columns()
        .iter()
        .map(|c| c.name().to_string())
        .collect::<Vec<_>>();
    let rows_pg = client
        .query(&stmt, &[])
        .await
        .with_context(|| "postgres query failed")?;
    if !options.sort.is_empty() {
        let mut rows = Vec::new();
        for row in &rows_pg {
            let normalized = apply_normalize(pg_row_to_value(row, &columns), &meta.normalize);
            if row_matches(&normalized, &options.filters, options.search.as_deref()) {
                rows.push(normalized);
            }
        }
        let mut result = paginate_rows(rows, &columns, &meta.normalize, options, true);
        result
            .perf
            .insert("pg_query_sort_ms".to_string(), elapsed_ms(query_started));
        return Ok(result);
    }
    let mut window = QueryWindow::new(options);
    for row in &rows_pg {
        if window.should_stop() {
            break;
        }
        let normalized = apply_normalize(pg_row_to_value(row, &columns), &meta.normalize);
        if row_matches(&normalized, &options.filters, options.search.as_deref()) {
            window.push(normalized);
        }
    }
    let mut result = window.finish(columns, true);
    result
        .perf
        .insert("pg_query_filter_ms".to_string(), elapsed_ms(query_started));
    Ok(result)
}

fn pg_row_to_value(row: &Row, columns: &[String]) -> Value {
    let mut map = serde_json::Map::new();
    for (idx, name) in columns.iter().enumerate() {
        map.insert(name.clone(), pg_cell_to_value(row, idx));
    }
    Value::Object(map)
}

fn pg_cell_to_value(row: &Row, idx: usize) -> Value {
    let col = &row.columns()[idx];
    let ty = col.type_();
    if *ty == Type::BOOL {
        return match row.try_get::<_, Option<bool>>(idx) {
            Ok(Some(v)) => Value::Bool(v),
            Ok(None) => Value::Null,
            Err(_) => Value::Null,
        };
    }
    if matches!(*ty, Type::INT2 | Type::INT4 | Type::INT8 | Type::OID) {
        if let Ok(v) = row.try_get::<_, Option<i64>>(idx) {
            return match v {
                Some(n) => json!(n),
                None => Value::Null,
            };
        }
    }
    if matches!(*ty, Type::FLOAT4 | Type::FLOAT8 | Type::NUMERIC) {
        if let Ok(v) = row.try_get::<_, Option<f64>>(idx) {
            return match v {
                Some(n) => json!(n),
                None => Value::Null,
            };
        }
    }
    if *ty == Type::JSON || *ty == Type::JSONB {
        if let Ok(v) = row.try_get::<_, Option<Value>>(idx) {
            return v.unwrap_or(Value::Null);
        }
    }
    if matches!(
        *ty,
        Type::TIMESTAMP | Type::TIMESTAMPTZ | Type::DATE | Type::TIME | Type::TIMETZ
    ) {
        if let Ok(Some(v)) = row.try_get::<_, Option<chrono::DateTime<chrono::Utc>>>(idx) {
            return Value::String(v.to_rfc3339());
        }
        if let Ok(Some(v)) = row.try_get::<_, Option<chrono::NaiveDateTime>>(idx) {
            return Value::String(v.format("%Y-%m-%dT%H:%M:%S%.f").to_string());
        }
        if let Ok(Some(v)) = row.try_get::<_, Option<String>>(idx) {
            return Value::String(v);
        }
    }
    match row.try_get::<_, Option<String>>(idx) {
        Ok(Some(v)) => Value::String(v),
        Ok(None) => Value::Null,
        Err(_) => match row.try_get::<_, Option<Vec<u8>>>(idx) {
            Ok(Some(bytes)) => Value::String(String::from_utf8_lossy(&bytes).into_owned()),
            _ => Value::Null,
        },
    }
}

/// Collect all rows for parquet materialization (no pagination).
pub fn fetch_all_postgres_rows(
    app_root: &Path,
    source: &SourceDecl,
    meta: &SourceMeta,
) -> Result<(Vec<String>, Vec<Value>)> {
    let options = DatasetQueryOptions {
        collect_all: true,
        page: 1,
        page_size: 0,
        ..DatasetQueryOptions::default()
    };
    let result = query_postgres_rows(app_root, source, meta, &options)?;
    Ok((result.columns, result.rows))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_env_dsn() {
        env::set_var("MEI_TEST_PG_RESOLVE", "postgresql://u:p@localhost/db");
        let dsn = resolve_connection_dsn("env:MEI_TEST_PG_RESOLVE").unwrap();
        assert!(dsn.contains("localhost"));
        env::remove_var("MEI_TEST_PG_RESOLVE");
    }

    #[test]
    fn resolve_plain_dsn() {
        let dsn = resolve_connection_dsn("postgresql://localhost/thunder").unwrap();
        assert_eq!(dsn, "postgresql://localhost/thunder");
    }

    #[test]
    fn detect_kinds() {
        assert!(is_postgres_kind("postgres"));
        assert!(is_postgres_kind("Timescale"));
        assert!(!is_postgres_kind("db"));
    }

    #[test]
    fn bare_select_not_wrapped() {
        let source = SourceDecl {
            kind: "postgres".into(),
            path: String::new(),
            sheet: None,
            header_row: None,
            preview_rows: None,
            page_size: None,
            max_page_size: None,
            table: None,
            query: Some("WITH x AS (SELECT 1 AS n) SELECT * FROM x".into()),
            connection: Some("postgresql://x".into()),
            content: None,
        };
        let meta = SourceMeta::default();
        let sql = base_sql(&meta, &source).unwrap();
        assert!(sql.starts_with("WITH"));
        assert!(!sql.contains("_mei_pg_q"));
    }

    #[test]
    fn optional_live_query_when_dsn_set() {
        let Ok(dsn) = env::var("MEI_TEST_PG_DSN") else {
            return;
        };
        if dsn.trim().is_empty() {
            return;
        }
        let source = SourceDecl {
            kind: "postgres".into(),
            path: String::new(),
            sheet: None,
            header_row: None,
            preview_rows: None,
            page_size: None,
            max_page_size: None,
            table: None,
            query: Some("SELECT 1::int AS n".into()),
            connection: Some(dsn),
            content: None,
        };
        let meta = SourceMeta::default();
        let result = query_postgres_rows(
            Path::new("/tmp/mei-pg-test-app"),
            &source,
            &meta,
            &DatasetQueryOptions {
                collect_all: true,
                ..DatasetQueryOptions::default()
            },
        )
        .expect("MEI_TEST_PG_DSN query");
        assert_eq!(result.columns, vec!["n".to_string()]);
        assert_eq!(result.rows.len(), 1);
    }
}
