//! Session-scoped layoutTuning draft overlay (Build / host-web only; not persisted).

use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};

use serde_json::Value;

fn store() -> &'static Mutex<BTreeMap<String, Value>> {
    static STORE: OnceLock<Mutex<BTreeMap<String, Value>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(BTreeMap::new()))
}

pub fn set_layout_tuning_draft(storage_key: &str, tuning: Value) {
    let key = storage_key.trim();
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

pub fn layout_tuning_draft(storage_key: &str) -> Option<Value> {
    let key = storage_key.trim();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_scoped_drafts_do_not_collide() {
        let key_a = crate::draft_session::layout_tuning_draft_storage_key("app", "sess-a");
        let key_b = crate::draft_session::layout_tuning_draft_storage_key("app", "sess-b");
        set_layout_tuning_draft(key_a.as_str(), serde_json::json!({"slotHeight": 120}));
        set_layout_tuning_draft(key_b.as_str(), serde_json::json!({"slotHeight": 88}));
        assert_ne!(
            layout_tuning_draft(key_a.as_str()),
            layout_tuning_draft(key_b.as_str())
        );
    }
}
