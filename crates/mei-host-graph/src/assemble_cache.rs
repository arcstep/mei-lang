//! Process-wide cache for `AssembleOutcome` keyed by semantic identity.

use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};

use crate::assemble::AssembleOutcome;
use crate::semantic_cache::{semantic_cache_core_signature, SemanticCacheCore};

fn store() -> &'static Mutex<BTreeMap<String, AssembleOutcome>> {
    static CACHE: OnceLock<Mutex<BTreeMap<String, AssembleOutcome>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(BTreeMap::new()))
}

pub fn assemble_cache_key(core: &SemanticCacheCore) -> Option<String> {
    semantic_cache_core_signature(core)
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
