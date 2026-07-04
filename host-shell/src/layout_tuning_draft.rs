//! Session-scoped layoutTuning draft overlay (Build / host-web only; not persisted).

use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};

use serde_json::Value;

fn store() -> &'static Mutex<BTreeMap<String, Value>> {
    static STORE: OnceLock<Mutex<BTreeMap<String, Value>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(BTreeMap::new()))
}

pub fn set_layout_tuning_draft(app_id: &str, tuning: Value) {
    let key = app_id.trim();
    if key.is_empty() {
        return;
    }
    if let Ok(mut guard) = store().lock() {
        if tuning.is_null() {
            guard.remove(key);
        } else {
            guard.insert(key.to_string(), tuning);
        }
    }
}

pub fn layout_tuning_draft(app_id: &str) -> Option<Value> {
    let key = app_id.trim();
    if key.is_empty() {
        return None;
    }
    store()
        .lock()
        .ok()
        .and_then(|guard| guard.get(key).cloned())
}

pub fn merge_layout_tuning_overlay(
    persisted: Option<&Value>,
    draft: Option<&Value>,
) -> Option<Value> {
    match (persisted, draft) {
        (None, None) => None,
        (Some(p), None) => Some(p.clone()),
        (None, Some(d)) => Some(d.clone()),
        (Some(p), Some(d)) => {
            let mut merged = p.clone();
            if let (Some(out), Some(draft_obj)) = (merged.as_object_mut(), d.as_object()) {
                for (k, v) in draft_obj {
                    out.insert(k.clone(), v.clone());
                }
            }
            Some(merged)
        }
    }
}
