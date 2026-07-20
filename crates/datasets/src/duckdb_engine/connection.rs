use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use anyhow::{Context, Result};
use duckdb::Connection;

struct AppConn {
    _app_root: PathBuf,
    conn: Connection,
}

fn pool() -> &'static Mutex<HashMap<String, AppConn>> {
    static POOL: OnceLock<Mutex<HashMap<String, AppConn>>> = OnceLock::new();
    POOL.get_or_init(|| Mutex::new(HashMap::new()))
}

fn app_key(app_root: &Path) -> String {
    app_root
        .canonicalize()
        .unwrap_or_else(|_| app_root.to_path_buf())
        .to_string_lossy()
        .into_owned()
}

pub fn with_app_connection<T>(
    app_root: &Path,
    f: impl FnOnce(&Connection) -> Result<T>,
) -> Result<T> {
    let key = app_key(app_root);
    let mut guard = pool()
        .lock()
        .map_err(|_| anyhow::anyhow!("duckdb connection pool poisoned"))?;
    if !guard.contains_key(&key) {
        let conn = Connection::open_in_memory().context("open in-memory DuckDB")?;
        let _ = conn.execute_batch("SET threads TO 2; SET memory_limit='512MB';");
        guard.insert(
            key.clone(),
            AppConn {
                _app_root: app_root.to_path_buf(),
                conn,
            },
        );
    }
    let entry = guard
        .get(&key)
        .ok_or_else(|| anyhow::anyhow!("duckdb connection missing after insert"))?;
    f(&entry.conn)
}

pub fn clear_duckdb_connections() -> usize {
    let Ok(mut guard) = pool().lock() else {
        return 0;
    };
    let n = guard.len();
    guard.clear();
    n
}

/// Ensure an in-memory DuckDB connection exists for `app_root` (warm / probe).
pub fn ensure_duckdb_connection(app_root: &Path) -> Result<()> {
    with_app_connection(app_root, |_| Ok(()))
}
