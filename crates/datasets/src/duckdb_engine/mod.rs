//! Embedded query engine (DataFusion): register parquet as views; page/scalar SQL.
//! See `docs/mei-lang-v2/05-host/0528-duckdb-query-engine.md`.

mod arrow_json;
mod connection;
mod metric_sql;
mod pipeline_sql;
mod query;
#[cfg(test)]
mod query_tests;
mod register;
mod sql;

pub use connection::{clear_duckdb_connections, ensure_duckdb_connection};
pub(crate) use metric_sql::count_primary_dataset_rows;
pub use pipeline_sql::{snapshot_pipeline_sql_stats, take_pipeline_sql_stats};
pub(crate) use pipeline_sql::{
    try_eval_dataframe_metrics_via_sql, try_eval_metrics_via_sql_partial,
};
pub(crate) use query::{query_parquet_page, DuckdbPageQuery};
pub use register::resolve_parquet_file_for_source;
pub(crate) use register::ensure_parquet_view;

use std::sync::atomic::{AtomicU64, Ordering};

static DUCKDB_QUERY_MS: AtomicU64 = AtomicU64::new(0);
static ROWS_MATERIALIZED: AtomicU64 = AtomicU64::new(0);

pub fn record_duckdb_query_ms(ms: u64) {
    DUCKDB_QUERY_MS.fetch_add(ms, Ordering::Relaxed);
}

pub fn record_rows_materialized(rows: usize) {
    ROWS_MATERIALIZED.fetch_add(rows as u64, Ordering::Relaxed);
}

pub fn take_duckdb_io_stats() -> (u64, u64) {
    (
        DUCKDB_QUERY_MS.swap(0, Ordering::Relaxed),
        ROWS_MATERIALIZED.swap(0, Ordering::Relaxed),
    )
}

pub fn snapshot_duckdb_io_stats() -> (u64, u64) {
    (
        DUCKDB_QUERY_MS.load(Ordering::Relaxed),
        ROWS_MATERIALIZED.load(Ordering::Relaxed),
    )
}
