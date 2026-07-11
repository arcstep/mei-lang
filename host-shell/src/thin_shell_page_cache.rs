//! In-memory HTML cache for revision-first unified `/view` SSR responses.

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

const MAX_ENTRIES: usize = 48;

#[derive(Debug, Clone)]
struct CacheEntry {
    html: String,
}

static CACHE: Mutex<Option<ThinShellPageCache>> = Mutex::new(None);

#[derive(Debug, Default)]
struct ThinShellPageCache {
    entries: HashMap<String, CacheEntry>,
    order: VecDeque<String>,
}

impl ThinShellPageCache {
    fn global() -> std::sync::MutexGuard<'static, Option<Self>> {
        let mut guard = CACHE.lock().expect("thin shell page cache lock");
        if guard.is_none() {
            *guard = Some(Self::default());
        }
        guard
    }

    fn get_inner(&self, key: &str) -> Option<String> {
        self.entries.get(key).map(|entry| entry.html.clone())
    }

    fn put_inner(&mut self, key: String, html: String) {
        if self.entries.contains_key(key.as_str()) {
            self.order.retain(|existing| existing != &key);
        }
        self.entries.insert(key.clone(), CacheEntry { html });
        self.order.push_back(key);
        while self.order.len() > MAX_ENTRIES {
            if let Some(stale) = self.order.pop_front() {
                self.entries.remove(stale.as_str());
            }
        }
    }

    fn clear_for_app_inner(&mut self, app_id: &str) {
        let prefix = format!("app_id={app_id}");
        let keys: Vec<String> = self
            .entries
            .keys()
            .filter(|key| key.contains(prefix.as_str()))
            .cloned()
            .collect();
        for key in keys {
            self.entries.remove(key.as_str());
            self.order.retain(|existing| existing != &key);
        }
    }
}

pub fn get(key: &str) -> Option<String> {
    let guard = ThinShellPageCache::global();
    guard.as_ref().and_then(|cache| cache.get_inner(key))
}

pub fn put(key: String, html: String) {
    let mut guard = ThinShellPageCache::global();
    if let Some(cache) = guard.as_mut() {
        cache.put_inner(key, html);
    }
}

pub fn clear_for_app(app_id: &str) {
    let mut guard = ThinShellPageCache::global();
    if let Some(cache) = guard.as_mut() {
        cache.clear_for_app_inner(app_id);
    }
}
