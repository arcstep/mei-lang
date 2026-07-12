//! In-memory layer artifact store keyed by cache key / content hash.
//!
//! Callers should prefix keys with [`mei_host_core::CachePartitionKey`] when
//! multiple App Runtime instances share a process.

use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use mei_host_core::CachePartitionKey;

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
    let part_prefix = format!("part:{app_id}/");
    let keys: Vec<String> = cache
        .keys()
        .filter(|key| key.contains(prefix.as_str()) || key.starts_with(part_prefix.as_str()))
        .cloned()
        .collect();
    let count = keys.len();
    for key in keys {
        cache.remove(key.as_str());
    }
    count
}

pub fn clear_layers_for_partition(partition: &CachePartitionKey) -> usize {
    let Ok(mut cache) = memory_store().lock() else {
        return 0;
    };
    let before = cache.len();
    cache.retain(|key, _| !partition.matches_key(key));
    before.saturating_sub(cache.len())
}

/// Convenience: store under a partition-prefixed key.
pub fn store_layer_partitioned(
    partition: &CachePartitionKey,
    inner_key: &str,
    artifact_id: &str,
    content_hash: &str,
    bytes: &[u8],
) -> String {
    let key = partition.prefix_key(inner_key);
    store_layer(key.clone(), artifact_id, content_hash, bytes);
    key
}

pub fn take_layer_partitioned(partition: &CachePartitionKey, inner_key: &str) -> Option<Vec<u8>> {
    take_layer(partition.prefix_key(inner_key).as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mei_host_core::CachePartitionKey;

    #[test]
    fn dual_partition_layer_entries_are_isolated() {
        let a = CachePartitionKey::new("mini-data", "WS-1", "cfg-a");
        let b = CachePartitionKey::new("mini-data", "WS-1", "cfg-b");
        clear_layers_for_partition(&a);
        clear_layers_for_partition(&b);

        let key_a = store_layer_partitioned(&a, "layer-x", "layer", "hash-a", b"aaa");
        let key_b = store_layer_partitioned(&b, "layer-x", "layer", "hash-b", b"bbb");
        assert_ne!(key_a, key_b);
        assert_eq!(
            take_layer_partitioned(&a, "layer-x").as_deref(),
            Some(b"aaa".as_slice())
        );
        assert_eq!(
            take_layer_partitioned(&b, "layer-x").as_deref(),
            Some(b"bbb".as_slice())
        );
        assert!(take_layer_partitioned(&a, "layer-x")
            .map(|bytes| bytes != b"bbb")
            .unwrap_or(false));

        assert_eq!(clear_layers_for_partition(&a), 1);
        assert!(take_layer_partitioned(&a, "layer-x").is_none());
        assert_eq!(
            take_layer_partitioned(&b, "layer-x").as_deref(),
            Some(b"bbb".as_slice())
        );
        clear_layers_for_partition(&b);
    }
}
