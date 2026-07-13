//! Process-local counters for eval-cache artifact I/O (P2.0 observability).

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use serde_json::{json, Value};

#[derive(Debug, Default)]
struct Counters {
    read_ops: AtomicU64,
    read_bytes: AtomicU64,
    write_ops: AtomicU64,
    write_bytes: AtomicU64,
    node_pack_loads: AtomicU64,
    node_pack_stores: AtomicU64,
    node_pack_store_skipped_full_hit: AtomicU64,
    response_store_skipped: AtomicU64,
    response_store_atomic: AtomicU64,
    content_hash_dedupe_skips: AtomicU64,
}

static COUNTERS: Counters = Counters {
    read_ops: AtomicU64::new(0),
    read_bytes: AtomicU64::new(0),
    write_ops: AtomicU64::new(0),
    write_bytes: AtomicU64::new(0),
    node_pack_loads: AtomicU64::new(0),
    node_pack_stores: AtomicU64::new(0),
    node_pack_store_skipped_full_hit: AtomicU64::new(0),
    response_store_skipped: AtomicU64::new(0),
    response_store_atomic: AtomicU64::new(0),
    content_hash_dedupe_skips: AtomicU64::new(0),
};

static LAST_SNAPSHOT: Mutex<Option<EvalCacheIoSnapshot>> = Mutex::new(None);

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EvalCacheIoSnapshot {
    pub read_ops: u64,
    pub read_bytes: u64,
    pub write_ops: u64,
    pub write_bytes: u64,
    pub node_pack_loads: u64,
    pub node_pack_stores: u64,
    pub node_pack_store_skipped_full_hit: u64,
    pub response_store_skipped: u64,
    pub response_store_atomic: u64,
    pub content_hash_dedupe_skips: u64,
}

impl EvalCacheIoSnapshot {
    pub fn to_json(&self) -> Value {
        json!({
            "readOps": self.read_ops,
            "readBytes": self.read_bytes,
            "writeOps": self.write_ops,
            "writeBytes": self.write_bytes,
            "nodePackLoads": self.node_pack_loads,
            "nodePackStores": self.node_pack_stores,
            "nodePackStoreSkippedFullHit": self.node_pack_store_skipped_full_hit,
            "responseStoreSkipped": self.response_store_skipped,
            "responseStoreAtomic": self.response_store_atomic,
            "contentHashDedupeSkips": self.content_hash_dedupe_skips,
        })
    }

    pub fn saturating_sub(&self, earlier: &Self) -> Self {
        Self {
            read_ops: self.read_ops.saturating_sub(earlier.read_ops),
            read_bytes: self.read_bytes.saturating_sub(earlier.read_bytes),
            write_ops: self.write_ops.saturating_sub(earlier.write_ops),
            write_bytes: self.write_bytes.saturating_sub(earlier.write_bytes),
            node_pack_loads: self.node_pack_loads.saturating_sub(earlier.node_pack_loads),
            node_pack_stores: self
                .node_pack_stores
                .saturating_sub(earlier.node_pack_stores),
            node_pack_store_skipped_full_hit: self
                .node_pack_store_skipped_full_hit
                .saturating_sub(earlier.node_pack_store_skipped_full_hit),
            response_store_skipped: self
                .response_store_skipped
                .saturating_sub(earlier.response_store_skipped),
            response_store_atomic: self
                .response_store_atomic
                .saturating_sub(earlier.response_store_atomic),
            content_hash_dedupe_skips: self
                .content_hash_dedupe_skips
                .saturating_sub(earlier.content_hash_dedupe_skips),
        }
    }
}

fn load_all() -> EvalCacheIoSnapshot {
    EvalCacheIoSnapshot {
        read_ops: COUNTERS.read_ops.load(Ordering::Relaxed),
        read_bytes: COUNTERS.read_bytes.load(Ordering::Relaxed),
        write_ops: COUNTERS.write_ops.load(Ordering::Relaxed),
        write_bytes: COUNTERS.write_bytes.load(Ordering::Relaxed),
        node_pack_loads: COUNTERS.node_pack_loads.load(Ordering::Relaxed),
        node_pack_stores: COUNTERS.node_pack_stores.load(Ordering::Relaxed),
        node_pack_store_skipped_full_hit: COUNTERS
            .node_pack_store_skipped_full_hit
            .load(Ordering::Relaxed),
        response_store_skipped: COUNTERS.response_store_skipped.load(Ordering::Relaxed),
        response_store_atomic: COUNTERS.response_store_atomic.load(Ordering::Relaxed),
        content_hash_dedupe_skips: COUNTERS.content_hash_dedupe_skips.load(Ordering::Relaxed),
    }
}

pub fn record_artifact_read(bytes: u64) {
    COUNTERS.read_ops.fetch_add(1, Ordering::Relaxed);
    COUNTERS.read_bytes.fetch_add(bytes, Ordering::Relaxed);
}

pub fn record_artifact_write(bytes: u64) {
    COUNTERS.write_ops.fetch_add(1, Ordering::Relaxed);
    COUNTERS.write_bytes.fetch_add(bytes, Ordering::Relaxed);
}

pub fn record_node_pack_load() {
    COUNTERS.node_pack_loads.fetch_add(1, Ordering::Relaxed);
}

pub fn record_node_pack_store() {
    COUNTERS.node_pack_stores.fetch_add(1, Ordering::Relaxed);
}

pub fn record_node_pack_store_skipped_full_hit() {
    COUNTERS
        .node_pack_store_skipped_full_hit
        .fetch_add(1, Ordering::Relaxed);
}

pub fn record_response_store_skipped() {
    COUNTERS
        .response_store_skipped
        .fetch_add(1, Ordering::Relaxed);
}

pub fn record_response_store_atomic() {
    COUNTERS
        .response_store_atomic
        .fetch_add(1, Ordering::Relaxed);
}

pub fn record_content_hash_dedupe_skips(count: u64) {
    if count == 0 {
        return;
    }
    COUNTERS
        .content_hash_dedupe_skips
        .fetch_add(count, Ordering::Relaxed);
}

pub fn snapshot_eval_cache_io() -> EvalCacheIoSnapshot {
    let snap = load_all();
    if let Ok(mut guard) = LAST_SNAPSHOT.lock() {
        *guard = Some(snap.clone());
    }
    snap
}

pub fn take_eval_cache_io_delta() -> EvalCacheIoSnapshot {
    let now = load_all();
    let earlier = LAST_SNAPSHOT
        .lock()
        .ok()
        .and_then(|mut guard| guard.take())
        .unwrap_or_default();
    now.saturating_sub(&earlier)
}

pub fn reset_eval_cache_io_stats_for_tests() {
    COUNTERS.read_ops.store(0, Ordering::Relaxed);
    COUNTERS.read_bytes.store(0, Ordering::Relaxed);
    COUNTERS.write_ops.store(0, Ordering::Relaxed);
    COUNTERS.write_bytes.store(0, Ordering::Relaxed);
    COUNTERS.node_pack_loads.store(0, Ordering::Relaxed);
    COUNTERS.node_pack_stores.store(0, Ordering::Relaxed);
    COUNTERS
        .node_pack_store_skipped_full_hit
        .store(0, Ordering::Relaxed);
    COUNTERS.response_store_skipped.store(0, Ordering::Relaxed);
    COUNTERS.response_store_atomic.store(0, Ordering::Relaxed);
    COUNTERS
        .content_hash_dedupe_skips
        .store(0, Ordering::Relaxed);
    if let Ok(mut guard) = LAST_SNAPSHOT.lock() {
        *guard = None;
    }
}
