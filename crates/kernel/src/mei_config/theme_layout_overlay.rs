//! Revision digest and overlay helpers for `ops.themes.*.layout` (0327 D3).

use std::collections::hash_map::DefaultHasher;
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};

use serde_json::Value;

use crate::mei_config::types::MeiConfig;

pub fn ops_theme_layout_revision_digest(config: &MeiConfig, theme_id: &str) -> String {
    let id = theme_id.trim();
    let layout = config
        .ops
        .extensions
        .get("layout")
        .cloned()
        .or_else(|| {
            config
                .ops
                .themes
                .get("_layout")
                .and_then(|theme| theme.get("layout").cloned())
        })
        .or_else(|| {
            config
                .ops
                .themes
                .get(id)
                .and_then(|theme| theme.get("layout").cloned())
        });
    let Some(layout) = layout else {
        return String::new();
    };
    let canonical = serde_json::to_string(&layout).unwrap_or_default();
    let mut hasher = DefaultHasher::new();
    canonical.hash(&mut hasher);
    format!("theme-layout:{:016x}", hasher.finish())
}

pub fn theme_layout_overlay_keys(layout: &Value) -> BTreeMap<String, Value> {
    let mut out = BTreeMap::new();
    if let Some(obj) = layout.as_object() {
        for (k, v) in obj {
            out.insert(k.clone(), v.clone());
        }
    }
    out
}

pub fn merge_theme_layout_overlay(
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
                    if let (Some(existing), Some(patch)) = (out.get(k), v.as_object()) {
                        let mut scope = existing.as_object().cloned().unwrap_or_default();
                        for (pk, pv) in patch {
                            scope.insert(pk.clone(), pv.clone());
                        }
                        out.insert(k.clone(), Value::Object(scope));
                    } else {
                        out.insert(k.clone(), v.clone());
                    }
                }
            }
            Some(merged)
        }
    }
}

pub fn merge_theme_layout_draft_into_theme(theme: &Value, layout_draft: &Value) -> Value {
    let mut out = theme.clone();
    let merged_layout = merge_theme_layout_overlay(theme.get("layout"), Some(layout_draft))
        .unwrap_or_else(|| layout_draft.clone());
    if let Some(obj) = out.as_object_mut() {
        obj.insert("layout".to_string(), merged_layout);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mei_config::types::{
        AppEntryConfig, AppFeaturesConfig, AppPathsConfig, MeiConfig, OpsConfig,
    };
    use serde_json::json;
    use std::collections::BTreeMap;

    #[test]
    fn theme_layout_revision_digest_changes_when_section_rows_change() {
        let mut themes = BTreeMap::new();
        themes.insert(
            "cockpit".to_string(),
            json!({"layout": {"home/T1/left_rail": {"sectionRows": ["1fr"]}}}),
        );
        let config = MeiConfig {
            schema_version: 1,
            entry: AppEntryConfig {
                main: "main.mei".to_string(),
            },
            paths: AppPathsConfig::default(),
            features: AppFeaturesConfig::default(),
            ops: OpsConfig {
                themes,
                ..Default::default()
            },
            ..Default::default()
        };
        let first = ops_theme_layout_revision_digest(&config, "cockpit");
        let mut themes_b = BTreeMap::new();
        themes_b.insert(
            "cockpit".to_string(),
            json!({"layout": {"home/T1/left_rail": {"sectionRows": ["2fr"]}}}),
        );
        let config_b = MeiConfig {
            ops: OpsConfig {
                themes: themes_b,
                ..config.ops.clone()
            },
            ..config
        };
        let second = ops_theme_layout_revision_digest(&config_b, "cockpit");
        assert_ne!(first, second);
    }

    #[test]
    fn merge_theme_layout_overlay_merges_scope_patches() {
        let persisted = json!({
            "home/T1/left_rail": {"sectionRows": ["1fr"], "gap": "12px"}
        });
        let draft = json!({
            "home/T1/left_rail": {"sectionRows": ["1fr", "2fr"]}
        });
        let merged = merge_theme_layout_overlay(Some(&persisted), Some(&draft)).expect("merged");
        let scope = merged.get("home/T1/left_rail").expect("scope");
        assert_eq!(scope.get("sectionRows"), Some(&json!(["1fr", "2fr"])));
        assert_eq!(scope.get("gap"), Some(&json!("12px")));
    }
}
