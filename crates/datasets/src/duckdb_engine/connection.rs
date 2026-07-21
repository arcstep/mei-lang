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
            .build()
            .expect("query engine tokio runtime")
    })
}

/// Block on a DataFusion async future.
///
/// When already inside a Tokio runtime (host/app warmup paths), use
/// `block_in_place` + the current handle — never nest `Runtime::block_on`.
pub fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => tokio::task::block_in_place(|| handle.block_on(fut)),
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

pub fn clear_duckdb_connections() -> usize {
    let Ok(mut guard) = pool().lock() else {
        return 0;
    };
    let n = guard.len();
    guard.clear();
    n
}

/// Ensure an in-memory DataFusion session exists for `app_root` (warm / probe).
pub fn ensure_duckdb_connection(app_root: &Path) -> Result<()> {
    with_app_session(app_root, |_| Ok(())).context("ensure query engine session")
}
