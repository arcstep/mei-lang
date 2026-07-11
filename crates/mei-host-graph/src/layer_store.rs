//! In-memory layer artifact store keyed by cache key / content hash.

use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const LAYER_CACHE_TTL_MS: u64 = 300_000;
const MAX_LAYER_ENTRIES: usize = 256;

#[derive(Debug, Clone)]
struct CachedLayer {
    expires_at: Instant,
    bytes: Vec<u8>,
    content_hash: String,
    artifact_id: String,
}

fn memory_store() -> &'static Mutex<BTreeMap<String, CachedLayer>> {
    static CACHE: OnceLock<Mutex<BTreeMap<String, CachedLayer>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn ttl() -> Duration {
    Duration::from_millis(LAYER_CACHE_TTL_MS)
}

pub fn take_layer(cache_key: &str) -> Option<Vec<u8>> {
    let Ok(mut cache) = memory_store().lock() else {
        return None;
    };
    let now = Instant::now();
    cache.retain(|_, entry| entry.expires_at > now);
    cache.get(cache_key).map(|entry| entry.bytes.clone())
}

pub fn store_layer(cache_key: String, artifact_id: &str, content_hash: &str, bytes: &[u8]) {
    let Ok(mut cache) = memory_store().lock() else {
        return;
    };
    cache.retain(|_, entry| entry.expires_at > Instant::now());
    if cache.len() >= MAX_LAYER_ENTRIES {
        cache.clear();
    }
    cache.insert(
        cache_key,
        CachedLayer {
            expires_at: Instant::now() + ttl(),
            bytes: bytes.to_vec(),
            content_hash: content_hash.to_string(),
            artifact_id: artifact_id.to_string(),
        },
    );
}

pub fn layer_entry_meta(cache_key: &str) -> Option<(String, String)> {
    let Ok(cache) = memory_store().lock() else {
        return None;
    };
    cache
        .get(cache_key)
        .map(|entry| (entry.artifact_id.clone(), entry.content_hash.clone()))
}

pub fn clear_layers_for_app(app_id: &str) -> usize {
    let Ok(mut cache) = memory_store().lock() else {
        return 0;
    };
    let prefix = format!("\"app_id\":\"{app_id}\"");
    let keys: Vec<String> = cache
        .keys()
        .filter(|key| key.contains(prefix.as_str()))
        .cloned()
        .collect();
    let count = keys.len();
    for key in keys {
        cache.remove(key.as_str());
    }
    count
}
