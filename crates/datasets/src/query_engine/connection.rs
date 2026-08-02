use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use anyhow::{Context, Result};
use datafusion::prelude::SessionContext;

struct AppConn {
    _app_root: PathBuf,
    ctx: SessionContext,
}

fn pool() -> &'static Mutex<HashMap<String, AppConn>> {
    static POOL: OnceLock<Mutex<HashMap<String, AppConn>>> = OnceLock::new();
    POOL.get_or_init(|| Mutex::new(HashMap::new()))
}

fn runtime() -> &'static tokio::runtime::Runtime {
    static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .thread_name("mei-query-engine")
            // DataFusion planning of factored WITH/UNION graphs needs more than the
            // default ~2MiB worker stack (effectiveness category-expand).
            .thread_stack_size(8 * 1024 * 1024)
            .build()
            .expect("query engine tokio runtime")
    })
}

/// Block on a DataFusion async future.
///
/// Must not nest `Runtime::block_on` / `Handle::block_on` on a thread that is
/// already driving a Tokio runtime (plug-ds / host `#[tokio::main]`). Those
/// paths call `enter_runtime` and panic with "Cannot start a runtime from
/// within a runtime". Drive the future with `futures::executor` inside
/// `block_in_place` so the current runtime exits its enter flag first.
pub fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    match tokio::runtime::Handle::try_current() {
        Ok(_handle) => tokio::task::block_in_place(|| futures_executor::block_on(fut)),
        Err(_) => runtime().block_on(fut),
    }
}

fn app_key(app_root: &Path) -> String {
    app_root
        .canonicalize()
        .unwrap_or_else(|_| app_root.to_path_buf())
        .to_string_lossy()
        .into_owned()
}

pub fn with_app_session<T>(
    app_root: &Path,
    f: impl FnOnce(&SessionContext) -> Result<T>,
) -> Result<T> {
    let key = app_key(app_root);
    let mut guard = pool()
        .lock()
        .map_err(|_| anyhow::anyhow!("query engine session pool poisoned"))?;
    if !guard.contains_key(&key) {
        let ctx = SessionContext::new();
        guard.insert(
            key.clone(),
            AppConn {
                _app_root: app_root.to_path_buf(),
                ctx,
            },
        );
    }
    let entry = guard
        .get(&key)
        .ok_or_else(|| anyhow::anyhow!("query engine session missing after insert"))?;
    f(&entry.ctx)
}

pub fn clear_query_engine_sessions() -> usize {
    let Ok(mut guard) = pool().lock() else {
        return 0;
    };
    let n = guard.len();
    guard.clear();
    n
}

pub fn clear_query_engine_session_for_app(app_root: &Path) -> usize {
    let Ok(mut guard) = pool().lock() else {
        return 0;
    };
    usize::from(guard.remove(&app_key(app_root)).is_some())
}

/// Ensure an in-memory DataFusion session exists for `app_root` (warm / probe).
pub fn ensure_query_engine_session(app_root: &Path) -> Result<()> {
    with_app_session(app_root, |_| Ok(())).context("ensure query engine session")
}

/// Run one SQL statement on the app session and return wall-clock milliseconds.
pub fn bench_sql_text(app_root: &Path, sql: &str) -> Result<u64> {
    let started = std::time::Instant::now();
    let sql = sql.trim().trim_end_matches(';').to_string();
    with_app_session(app_root, |ctx| {
        block_on(async {
            let _batches = ctx
                .sql(&sql)
                .await
                .with_context(|| {
                    format!(
                        "replay prepare sql failed (sql_chars={})",
                        sql.chars().count()
                    )
                })?
                .collect()
                .await
                .with_context(|| {
                    format!(
                        "replay collect sql failed (sql_chars={})",
                        sql.chars().count()
                    )
                })?;
            Ok(())
        })
    })?;
    Ok(started.elapsed().as_millis() as u64)
}
