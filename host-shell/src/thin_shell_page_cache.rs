//! Stable thin-shell document cache.
//! Business preview HTML is never stored here; entries contain only the revision envelope.

use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

const MAX_ENTRIES: usize = 128;

#[derive(Clone)]
pub struct ThinShellCacheEntry {
    pub html: String,
    pub etag: String,
}

#[derive(Clone)]
struct StoredEntry {
    app_id: String,
    value: ThinShellCacheEntry,
}

fn cache() -> &'static RwLock<HashMap<String, StoredEntry>> {
    static CACHE: OnceLock<RwLock<HashMap<String, StoredEntry>>> = OnceLock::new();
    CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

pub fn get(key: &str) -> Option<ThinShellCacheEntry> {
    cache()
        .read()
        .ok()?
        .get(key)
        .map(|entry| entry.value.clone())
}

pub fn put(app_id: &str, key: String, html: String, etag: String) {
    let Ok(mut entries) = cache().write() else {
        return;
    };
    if entries.len() >= MAX_ENTRIES && !entries.contains_key(key.as_str()) {
        if let Some(stale) = entries.keys().next().cloned() {
            entries.remove(stale.as_str());
        }
    }
    entries.insert(
        key,
        StoredEntry {
            app_id: app_id.to_string(),
            value: ThinShellCacheEntry { html, etag },
        },
    );
}

pub fn clear_for_app(app_id: &str) {
    let Ok(mut entries) = cache().write() else {
        return;
    };
    entries.retain(|_, entry| entry.app_id != app_id);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_is_scoped_and_invalidated_by_app() {
        put(
            "demo",
            "demo:home".to_string(),
            "<html>thin</html>".to_string(),
            "W/\"demo\"".to_string(),
        );
        assert!(get("demo:home").is_some());
        clear_for_app("demo");
        assert!(get("demo:home").is_none());
    }
}
