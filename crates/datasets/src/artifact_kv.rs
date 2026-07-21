//! Transactional small-artifact store.
//!
//! Values remain auditable JSON bytes (see design gate 0526). redb replaces
//! scattered small JSON files; it is not a carrier for table rowsets.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Context, Result};
use redb::{Database, ReadOnlyDatabase, ReadableDatabase, ReadableTable, TableDefinition};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const STORE_SCHEMA: &str = "mei-small-artifact-store-v1";
const DB_FILE: &str = "small-artifacts-v1.redb";
const DEFAULT_MAX_VALUE_BYTES: usize = 4 * 1024 * 1024;
const DEFAULT_REDB_CACHE_BYTES: usize = 8 * 1024 * 1024;
const ARTIFACTS: TableDefinition<&str, &[u8]> = TableDefinition::new("small_artifacts_v1");

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredArtifact {
    store_schema: String,
    kind: String,
    logical_key: String,
    generated_at_ms: u64,
    payload: serde_json::Value,
}

#[derive(Debug, Clone, Copy, Default, Serialize)]
pub struct SmallArtifactStoreStats {
    pub hits: u64,
    pub misses: u64,
    pub reads: u64,
    pub writes: u64,
    pub rejected_oversize: u64,
    pub removed: u64,
    pub read_ms: u64,
    pub write_ms: u64,
    pub db_bytes: u64,
}

#[derive(Default)]
struct StoreCounters {
    hits: AtomicU64,
    misses: AtomicU64,
    reads: AtomicU64,
    writes: AtomicU64,
    rejected_oversize: AtomicU64,
    removed: AtomicU64,
    read_ms: AtomicU64,
    write_ms: AtomicU64,
}

static COUNTERS: StoreCounters = StoreCounters {
    hits: AtomicU64::new(0),
    misses: AtomicU64::new(0),
    reads: AtomicU64::new(0),
    writes: AtomicU64::new(0),
    rejected_oversize: AtomicU64::new(0),
    removed: AtomicU64::new(0),
    read_ms: AtomicU64::new(0),
    write_ms: AtomicU64::new(0),
};

fn writer_databases() -> &'static Mutex<BTreeMap<PathBuf, Arc<Database>>> {
    static DATABASES: OnceLock<Mutex<BTreeMap<PathBuf, Arc<Database>>>> = OnceLock::new();
    DATABASES.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn reader_databases() -> &'static Mutex<BTreeMap<PathBuf, Arc<ReadOnlyDatabase>>> {
    static DATABASES: OnceLock<Mutex<BTreeMap<PathBuf, Arc<ReadOnlyDatabase>>>> = OnceLock::new();
    DATABASES.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn now_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn configured_usize(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|raw| raw.trim().parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default)
}

fn max_value_bytes() -> usize {
    configured_usize("MEI_SMALL_ARTIFACT_MAX_BYTES", DEFAULT_MAX_VALUE_BYTES)
}

fn redb_cache_bytes() -> usize {
    configured_usize("MEI_REDB_CACHE_BYTES", DEFAULT_REDB_CACHE_BYTES)
}

fn physical_key(kind: &str, logical_key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(logical_key.as_bytes());
    format!("{kind}\0{:x}", hasher.finalize())
}

fn db_path_for_eval_root(eval_root: &Path) -> PathBuf {
    eval_root.join(DB_FILE)
}

pub fn small_artifact_store_path(app_root: &Path) -> PathBuf {
    db_path_for_eval_root(&mei_lang_kernel::resolve_app_eval_cache_root(app_root))
}

pub fn small_artifact_build_store_path(app_root: &Path) -> PathBuf {
    db_path_for_eval_root(&mei_lang_kernel::resolve_app_build_eval_cache_root(
        app_root,
    ))
}

/// Stable map key so `env/current/...` and `env/WS-…/...` share one handle.
fn map_key(path: &Path) -> PathBuf {
    if let Ok(canon) = path.canonicalize() {
        return canon;
    }
    if let Some(parent) = path.parent() {
        if let Ok(parent) = parent.canonicalize() {
            if let Some(name) = path.file_name() {
                return parent.join(name);
            }
        }
    }
    path.to_path_buf()
}

/// When runtime uses an instance var overlay, materialize the prebuild seed once
/// into the overlay so this process never contended-opens the shared generation DB.
fn ensure_overlay_seeded(app_root: &Path) -> Result<()> {
    let primary = small_artifact_store_path(app_root);
    let seed = small_artifact_build_store_path(app_root);
    if primary == seed || primary.is_file() || !seed.is_file() {
        return Ok(());
    }
    if let Some(parent) = primary.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!("create overlay small-artifact dir {}", parent.display())
        })?;
    }
    let tmp = primary.with_extension("redb.seeding");
    fs::copy(&seed, &tmp).with_context(|| {
        format!(
            "copy small-artifact seed {} -> {}",
            seed.display(),
            tmp.display()
        )
    })?;
    match fs::rename(&tmp, &primary) {
        Ok(()) => Ok(()),
        Err(_) if primary.is_file() => {
            let _ = fs::remove_file(&tmp);
            Ok(())
        }
        Err(error) => {
            let _ = fs::remove_file(&tmp);
            Err(error).with_context(|| {
                format!(
                    "promote seeded small-artifact store to {}",
                    primary.display()
                )
            })
        }
    }
}

fn read_paths(app_root: &Path) -> Result<Vec<PathBuf>> {
    ensure_overlay_seeded(app_root)?;
    let primary = small_artifact_store_path(app_root);
    let build = small_artifact_build_store_path(app_root);
    if primary == build || primary.is_file() {
        // Overlay present (or no overlay): prefer the writable primary only.
        Ok(vec![primary])
    } else {
        Ok(vec![primary, build])
    }
}

fn writer_for(path: &Path) -> Result<Arc<Database>> {
    let key = map_key(path);
    let mut databases = writer_databases()
        .lock()
        .map_err(|_| anyhow!("small artifact database map poisoned"))?;
    if let Some(database) = databases.get(&key).cloned() {
        return Ok(database);
    }
    if reader_databases()
        .lock()
        .map_err(|_| anyhow!("small artifact reader map poisoned"))?
        .contains_key(&key)
    {
        return Err(anyhow!(
            "cannot open writable redb while a read-only handle is already open: {}",
            key.display()
        ));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create redb artifact root {}", parent.display()))?;
    }
    let database = Arc::new(
        Database::builder()
            .set_cache_size(redb_cache_bytes())
            .create(path)
            .with_context(|| format!("open redb artifact store {}", path.display()))?,
    );
    let key = map_key(path);
    databases.insert(key, database.clone());
    Ok(database)
}

fn read_bytes_from_writer(path: &Path, key: &str) -> Result<(bool, Option<Vec<u8>>)> {
    let map_key = map_key(path);
    let database = writer_databases()
        .lock()
        .map_err(|_| anyhow!("small artifact database map poisoned"))?
        .get(&map_key)
        .cloned();
    let Some(database) = database else {
        return Ok((false, None));
    };
    Ok((true, read_table_value(database.as_ref(), key)?))
}

fn reader_for(path: &Path) -> Result<Option<Arc<ReadOnlyDatabase>>> {
    let key = map_key(path);
    let mut databases = reader_databases()
        .lock()
        .map_err(|_| anyhow!("small artifact reader map poisoned"))?;
    if let Some(database) = databases.get(&key).cloned() {
        return Ok(Some(database));
    }
    if writer_databases()
        .lock()
        .map_err(|_| anyhow!("small artifact database map poisoned"))?
        .contains_key(&key)
    {
        // Same process already has a writer: callers should read via writer.
        return Ok(None);
    }
    if !path.is_file() {
        return Ok(None);
    }
    let database = Arc::new(
        Database::builder()
            .set_cache_size(redb_cache_bytes())
            .open_read_only(path)
            .with_context(|| {
                format!(
                    "open read-only redb artifact store {} (hint: another process may hold a writable lock on the shared seed; runtime should seed instance overlay first)",
                    path.display()
                )
            })?,
    );
    let key = map_key(path);
    databases.insert(key, database.clone());
    Ok(Some(database))
}

fn read_table_value<D: ReadableDatabase>(database: &D, key: &str) -> Result<Option<Vec<u8>>> {
    let transaction = database.begin_read()?;
    let table = match transaction.open_table(ARTIFACTS) {
        Ok(table) => table,
        Err(redb::TableError::TableDoesNotExist(_)) => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let value = table.get(key)?;
    Ok(value.map(|guard| guard.value().to_vec()))
}

fn read_bytes(path: &Path, key: &str, prefer_writer: bool) -> Result<Option<Vec<u8>>> {
    let (writer_open, value) = read_bytes_from_writer(path, key)?;
    if writer_open {
        return Ok(value);
    }
    if prefer_writer {
        if !path.is_file() {
            return Ok(None);
        }
        let database = writer_for(path)?;
        return read_table_value(database.as_ref(), key);
    }
    let Some(database) = reader_for(path)? else {
        // Writer may already own this path under a canonical key.
        let (writer_open, value) = read_bytes_from_writer(path, key)?;
        return if writer_open { Ok(value) } else { Ok(None) };
    };
    read_table_value(database.as_ref(), key)
}

pub fn load_small_artifact<T: DeserializeOwned>(
    app_root: &Path,
    kind: &str,
    logical_key: &str,
) -> Result<Option<T>> {
    let started = Instant::now();
    COUNTERS.reads.fetch_add(1, Ordering::Relaxed);
    let key = physical_key(kind, logical_key);
    let primary = small_artifact_store_path(app_root);
    for path in read_paths(app_root)? {
        let prefer_writer = path == primary;
        let Some(bytes) = read_bytes(path.as_path(), key.as_str(), prefer_writer)? else {
            continue;
        };
        let stored: StoredArtifact = serde_json::from_slice(&bytes)
            .with_context(|| format!("decode redb artifact {kind}:{logical_key}"))?;
        if stored.store_schema != STORE_SCHEMA
            || stored.kind != kind
            || stored.logical_key != logical_key
        {
            continue;
        }
        let payload = serde_json::from_value(stored.payload)
            .with_context(|| format!("decode redb payload {kind}:{logical_key}"))?;
        COUNTERS.hits.fetch_add(1, Ordering::Relaxed);
        COUNTERS
            .read_ms
            .fetch_add(started.elapsed().as_millis() as u64, Ordering::Relaxed);
        return Ok(Some(payload));
    }
    COUNTERS.misses.fetch_add(1, Ordering::Relaxed);
    COUNTERS
        .read_ms
        .fetch_add(started.elapsed().as_millis() as u64, Ordering::Relaxed);
    Ok(None)
}

pub fn store_small_artifact<T: Serialize>(
    app_root: &Path,
    kind: &str,
    logical_key: &str,
    payload: &T,
) -> Result<usize> {
    ensure_overlay_seeded(app_root)?;
    let started = Instant::now();
    let stored = StoredArtifact {
        store_schema: STORE_SCHEMA.to_string(),
        kind: kind.to_string(),
        logical_key: logical_key.to_string(),
        generated_at_ms: now_epoch_ms(),
        payload: serde_json::to_value(payload)?,
    };
    let bytes = serde_json::to_vec(&stored)?;
    if bytes.len() > max_value_bytes() {
        COUNTERS.rejected_oversize.fetch_add(1, Ordering::Relaxed);
        return Ok(0);
    }
    let path = small_artifact_store_path(app_root);
    let database = writer_for(path.as_path())?;
    let transaction = database.begin_write()?;
    {
        let mut table = transaction.open_table(ARTIFACTS)?;
        let key = physical_key(kind, logical_key);
        table.insert(key.as_str(), bytes.as_slice())?;
    }
    transaction.commit()?;
    COUNTERS.writes.fetch_add(1, Ordering::Relaxed);
    COUNTERS
        .write_ms
        .fetch_add(started.elapsed().as_millis() as u64, Ordering::Relaxed);
    Ok(bytes.len())
}

pub fn store_small_artifact_batch(
    app_root: &Path,
    kind: &str,
    values: &[(String, serde_json::Value)],
) -> Result<usize> {
    if values.is_empty() {
        return Ok(0);
    }
    ensure_overlay_seeded(app_root)?;
    let started = Instant::now();
    let mut encoded = Vec::with_capacity(values.len());
    for (logical_key, payload) in values {
        let bytes = serde_json::to_vec(&StoredArtifact {
            store_schema: STORE_SCHEMA.to_string(),
            kind: kind.to_string(),
            logical_key: logical_key.clone(),
            generated_at_ms: now_epoch_ms(),
            payload: payload.clone(),
        })?;
        if bytes.len() > max_value_bytes() {
            COUNTERS.rejected_oversize.fetch_add(1, Ordering::Relaxed);
            return Ok(0);
        }
        encoded.push((physical_key(kind, logical_key), bytes));
    }
    let total_bytes = encoded.iter().map(|(_, bytes)| bytes.len()).sum::<usize>();
    let path = small_artifact_store_path(app_root);
    let database = writer_for(path.as_path())?;
    let transaction = database.begin_write()?;
    {
        let mut table = transaction.open_table(ARTIFACTS)?;
        for (key, bytes) in &encoded {
            table.insert(key.as_str(), bytes.as_slice())?;
        }
    }
    transaction.commit()?;
    COUNTERS
        .writes
        .fetch_add(encoded.len() as u64, Ordering::Relaxed);
    COUNTERS
        .write_ms
        .fetch_add(started.elapsed().as_millis() as u64, Ordering::Relaxed);
    Ok(total_bytes)
}

pub fn remove_small_artifact(app_root: &Path, kind: &str, logical_key: &str) -> Result<bool> {
    let path = small_artifact_store_path(app_root);
    if !path.is_file() {
        return Ok(false);
    }
    let database = writer_for(path.as_path())?;
    let transaction = database.begin_write()?;
    let removed = {
        let mut table = transaction.open_table(ARTIFACTS)?;
        let key = physical_key(kind, logical_key);
        let existed = table.remove(key.as_str())?.is_some();
        existed
    };
    transaction.commit()?;
    if removed {
        COUNTERS.removed.fetch_add(1, Ordering::Relaxed);
    }
    Ok(removed)
}

pub fn retain_small_artifact_keys(
    app_root: &Path,
    kind: &str,
    retained_logical_keys: &BTreeSet<String>,
) -> Result<usize> {
    let path = small_artifact_store_path(app_root);
    if !path.is_file() {
        return Ok(0);
    }
    let database = writer_for(path.as_path())?;
    let transaction = database.begin_write()?;
    let removed = {
        let mut table = transaction.open_table(ARTIFACTS)?;
        let prefix = format!("{kind}\0");
        let mut remove = Vec::new();
        for entry in table.iter()? {
            let (key, value) = entry?;
            if !key.value().starts_with(prefix.as_str()) {
                continue;
            }
            let stored: StoredArtifact = match serde_json::from_slice(value.value()) {
                Ok(stored) => stored,
                Err(_) => {
                    remove.push(key.value().to_string());
                    continue;
                }
            };
            if !retained_logical_keys.contains(stored.logical_key.as_str()) {
                remove.push(key.value().to_string());
            }
        }
        let count = remove.len();
        for key in remove {
            let _ = table.remove(key.as_str())?;
        }
        count
    };
    transaction.commit()?;
    COUNTERS
        .removed
        .fetch_add(removed as u64, Ordering::Relaxed);
    Ok(removed)
}

pub fn remove_small_artifacts_with_prefix(
    app_root: &Path,
    kind: &str,
    logical_key_prefix: &str,
) -> Result<usize> {
    let path = small_artifact_store_path(app_root);
    if !path.is_file() {
        return Ok(0);
    }
    let database = writer_for(path.as_path())?;
    let transaction = database.begin_write()?;
    let removed = {
        let mut table = transaction.open_table(ARTIFACTS)?;
        let physical_prefix = format!("{kind}\0");
        let mut remove = Vec::new();
        for entry in table.iter()? {
            let (key, value) = entry?;
            if !key.value().starts_with(physical_prefix.as_str()) {
                continue;
            }
            let stored: StoredArtifact = match serde_json::from_slice(value.value()) {
                Ok(stored) => stored,
                Err(_) => {
                    remove.push(key.value().to_string());
                    continue;
                }
            };
            if stored.logical_key.starts_with(logical_key_prefix) {
                remove.push(key.value().to_string());
            }
        }
        let count = remove.len();
        for key in remove {
            let _ = table.remove(key.as_str())?;
        }
        count
    };
    transaction.commit()?;
    COUNTERS
        .removed
        .fetch_add(removed as u64, Ordering::Relaxed);
    Ok(removed)
}

pub fn clear_small_artifacts(app_root: &Path) -> Result<usize> {
    let path = small_artifact_store_path(app_root);
    if !path.is_file() {
        return Ok(0);
    }
    let database = writer_for(path.as_path())?;
    let transaction = database.begin_write()?;
    let removed = {
        let mut table = transaction.open_table(ARTIFACTS)?;
        let keys = table
            .iter()?
            .map(|entry| entry.map(|(key, _)| key.value().to_string()))
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let count = keys.len();
        for key in keys {
            let _ = table.remove(key.as_str())?;
        }
        count
    };
    transaction.commit()?;
    COUNTERS
        .removed
        .fetch_add(removed as u64, Ordering::Relaxed);
    Ok(removed)
}

pub fn snapshot_small_artifact_store_stats(app_root: &Path) -> SmallArtifactStoreStats {
    let db_bytes = read_paths(app_root)
        .unwrap_or_default()
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter_map(|path| fs::metadata(path).ok())
        .map(|metadata| metadata.len())
        .sum();
    SmallArtifactStoreStats {
        hits: COUNTERS.hits.load(Ordering::Relaxed),
        misses: COUNTERS.misses.load(Ordering::Relaxed),
        reads: COUNTERS.reads.load(Ordering::Relaxed),
        writes: COUNTERS.writes.load(Ordering::Relaxed),
        rejected_oversize: COUNTERS.rejected_oversize.load(Ordering::Relaxed),
        removed: COUNTERS.removed.load(Ordering::Relaxed),
        read_ms: COUNTERS.read_ms.load(Ordering::Relaxed),
        write_ms: COUNTERS.write_ms.load(Ordering::Relaxed),
        db_bytes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_app_root() -> (tempfile::TempDir, PathBuf) {
        let temp = tempfile::tempdir().expect("tempdir");
        let app_root = temp.path().join("demo");
        let env = app_root.join("env").join("WS-1");
        fs::create_dir_all(env.join("build")).expect("build");
        fs::create_dir_all(env.join("var")).expect("var");
        #[cfg(unix)]
        {
            fs::create_dir_all(app_root.join("env")).expect("env");
            std::os::unix::fs::symlink("WS-1", app_root.join("env/current")).expect("current");
        }
        #[cfg(not(unix))]
        fs::create_dir_all(app_root.join("env/current")).expect("current");
        (temp, app_root)
    }

    #[test]
    fn round_trips_and_rejects_wrong_logical_key() {
        let (_temp, app_root) = temp_app_root();
        let payload = serde_json::json!({"value": 7});
        let bytes =
            store_small_artifact(&app_root, "metric-lite", "scope-a", &payload).expect("store");
        assert!(bytes > 0);
        let loaded: serde_json::Value = load_small_artifact(&app_root, "metric-lite", "scope-a")
            .expect("load")
            .expect("present");
        assert_eq!(loaded, payload);
        assert!(
            load_small_artifact::<serde_json::Value>(&app_root, "metric-lite", "scope-b")
                .expect("miss")
                .is_none()
        );
    }

    #[test]
    fn retain_removes_unlisted_logical_keys() {
        let (_temp, app_root) = temp_app_root();
        store_small_artifact(&app_root, "metric-lite", "a", &1).expect("store a");
        store_small_artifact(&app_root, "metric-lite", "b", &2).expect("store b");
        let removed =
            retain_small_artifact_keys(&app_root, "metric-lite", &BTreeSet::from(["b".into()]))
                .expect("retain");
        assert_eq!(removed, 1);
        assert!(load_small_artifact::<u64>(&app_root, "metric-lite", "a")
            .expect("load a")
            .is_none());
        assert_eq!(
            load_small_artifact::<u64>(&app_root, "metric-lite", "b").expect("load b"),
            Some(2)
        );
    }

    #[test]
    fn batch_is_visible_after_one_commit() {
        let (_temp, app_root) = temp_app_root();
        let bytes = store_small_artifact_batch(
            &app_root,
            "plan",
            &[
                ("a".into(), serde_json::json!({"revision": 1})),
                ("b".into(), serde_json::json!({"revision": 2})),
            ],
        )
        .expect("batch");
        assert!(bytes > 0);
        assert_eq!(
            load_small_artifact::<serde_json::Value>(&app_root, "plan", "a")
                .expect("a")
                .and_then(|value| value.get("revision").and_then(|value| value.as_u64())),
            Some(1)
        );
        assert_eq!(
            load_small_artifact::<serde_json::Value>(&app_root, "plan", "b")
                .expect("b")
                .and_then(|value| value.get("revision").and_then(|value| value.as_u64())),
            Some(2)
        );
    }
}
