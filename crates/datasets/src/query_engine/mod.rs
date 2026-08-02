//! Embedded query engine (DataFusion): register parquet as views; page/scalar SQL.
//! See `docs/mei-lang-v2/05-host/0528-query-engine.md`.

mod arrow_json;
mod connection;
mod metric_sql;
mod pipeline_sql;
mod query;
#[cfg(test)]
mod query_tests;
mod register;
mod sql;

pub use connection::{
    clear_query_engine_session_for_app, clear_query_engine_sessions, ensure_query_engine_session,
};
pub(crate) use connection::block_on;
/// Best-effort SQL replay for `mei-toolchain query-audit replay` (separate process;
/// parquet views must already be registerable via prior ensure / Host warmup patterns).
pub use connection::bench_sql_text;
pub(crate) use metric_sql::count_primary_dataset_rows;
pub use pipeline_sql::{
    append_query_audit, load_query_audit_jsonl, materialize_expr_to_parquet, query_audit_dir,
    query_audit_gate_failures, query_audit_jsonl_path, shape_exceeds_gate, today_yyyymmdd,
    QueryAuditEntry, QueryAuditResult, QueryAuditShape, QueryAuditTiming, CONTROLLED_SQL_MAX_CHARS,
    CONTROLLED_SQL_MAX_UNION_ALL, snapshot_pipeline_sql_stats, take_pipeline_sql_stats,
};
pub(crate) use pipeline_sql::{
    try_eval_dataframe_metrics_via_sql, try_eval_metrics_via_sql_partial,
    try_page_dataframe_metric_via_sql,
};
pub(crate) use query::{query_parquet_page, ParquetPageQuery};
pub(crate) use register::ensure_parquet_view;
pub use register::{
    derived_view_parquet_path, is_prebuild_materialized_view, resolve_parquet_file_for_source,
};

use std::sync::atomic::{AtomicU64, Ordering};

static QUERY_ENGINE_MS: AtomicU64 = AtomicU64::new(0);
static ROWS_MATERIALIZED: AtomicU64 = AtomicU64::new(0);

pub fn record_query_engine_ms(ms: u64) {
    QUERY_ENGINE_MS.fetch_add(ms, Ordering::Relaxed);
}

pub fn record_rows_materialized(rows: usize) {
    ROWS_MATERIALIZED.fetch_add(rows as u64, Ordering::Relaxed);
}

pub fn take_query_engine_io_stats() -> (u64, u64) {
    (
        QUERY_ENGINE_MS.swap(0, Ordering::Relaxed),
        ROWS_MATERIALIZED.swap(0, Ordering::Relaxed),
    )
}

pub fn snapshot_query_engine_io_stats() -> (u64, u64) {
    (
        QUERY_ENGINE_MS.load(Ordering::Relaxed),
        ROWS_MATERIALIZED.load(Ordering::Relaxed),
    )
}
