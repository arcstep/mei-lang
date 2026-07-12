//! Process-wide cache for `AssembleOutcome` keyed by semantic identity.
//!
//! Keys may be partition-prefixed via [`mei_host_core::CachePartitionKey`] so
//! same-process multi-instance embeds do not share entries.

use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};

use mei_host_core::CachePartitionKey;

use crate::assemble::AssembleOutcome;
use crate::semantic_cache::{semantic_cache_core_signature, SemanticCacheCore};

fn store() -> &'static Mutex<BTreeMap<String, AssembleOutcome>> {
    static CACHE: OnceLock<Mutex<BTreeMap<String, AssembleOutcome>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(BTreeMap::new()))
}

/// Legacy (unpartitioned) key — prefer [`assemble_cache_key_partitioned`].
pub fn assemble_cache_key(core: &SemanticCacheCore) -> Option<String> {
    semantic_cache_core_signature(core)
}

pub fn assemble_cache_key_partitioned(
    core: &SemanticCacheCore,
    partition: &CachePartitionKey,
) -> Option<String> {
    let inner = semantic_cache_core_signature(core)?;
    Some(partition.prefix_key(&inner))
}

pub fn take_assemble_outcome(cache_key: &str) -> Option<AssembleOutcome> {
    let Ok(cache) = store().lock() else {
        return None;
    };
    cache.get(cache_key).cloned()
}

pub fn store_assemble_outcome(cache_key: String, outcome: AssembleOutcome) {
    let Ok(mut cache) = store().lock() else {
        return;
    };
    cache.insert(cache_key, outcome);
}

pub fn clear_assemble_cache_for_app(app_id: &str) {
    let Ok(mut cache) = store().lock() else {
        return;
    };
    let prefix = format!("\"app_id\":\"{app_id}\"");
    cache.retain(|key, _| !key.contains(prefix.as_str()));
}

pub fn clear_assemble_cache_for_partition(partition: &CachePartitionKey) -> usize {
    let Ok(mut cache) = store().lock() else {
        return 0;
    };
    let before = cache.len();
    cache.retain(|key, _| !partition.matches_key(key));
    before.saturating_sub(cache.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::semantic_cache::build_semantic_cache_core;
    use mei_host_core::CachePartitionKey;

    #[test]
    fn dual_partition_keys_do_not_collide() {
        let core = build_semantic_cache_core(
            "mini-data",
            "home",
            None,
            "reg-1",
            "client-1",
            "data-1",
            "epoch-1",
        );
        let a = CachePartitionKey::new("mini-data", "WS-1", "cfg-a");
        let b = CachePartitionKey::new("mini-data", "WS-1", "cfg-b");
        let key_a = assemble_cache_key_partitioned(&core, &a).expect("key a");
        let key_b = assemble_cache_key_partitioned(&core, &b).expect("key b");
        assert_ne!(key_a, key_b);
        assert!(a.matches_key(key_a.as_str()));
        assert!(!a.matches_key(key_b.as_str()));
        assert_eq!(clear_assemble_cache_for_partition(&a), 0);
    }
}
