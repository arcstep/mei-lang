//! Revision digest for ops.layoutTuning overlay (excluded from compile digest).

use serde_json::Value;
use std::collections::BTreeMap;

use crate::mei_config::types::OpsConfig;

pub fn ops_layout_tuning_revision_digest(ops: &OpsConfig) -> String {
    let Some(tuning) = ops.layout_tuning.as_ref() else {
        return String::new();
    };
    let canonical = serde_json::to_string(tuning).unwrap_or_default();
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    canonical.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

pub fn layout_tuning_overlay_keys(tuning: &Value) -> BTreeMap<String, Value> {
    let mut out = BTreeMap::new();
    if let Some(obj) = tuning.as_object() {
        for (k, v) in obj {
            out.insert(k.clone(), v.clone());
        }
    }
    out
}

/// P1: read-only diff lines between index budget and ops.layoutTuning entry.
/// When `theme_layout` is set, duplicate keys surface a migration hint (0327 §5.2).
pub fn format_layout_tuning_diff(
    preview_scope: &str,
    budget: Option<&crate::model::UiBudgetSummary>,
    tuning: Option<&Value>,
    theme_layout: Option<&Value>,
) -> Option<String> {
    let tuning_obj = tuning?.as_object()?;
    let patch = tuning_obj.get(preview_scope)?;
    let mut lines: Vec<String> = Vec::new();

    let config_profile = patch
        .get("padding_profile")
        .or_else(|| patch.get("paddingProfile"))
        .and_then(Value::as_str);
    let index_profile = budget.and_then(|b| b.padding_profile.as_deref());
    match (index_profile, config_profile) {
        (Some(index), Some(config)) if index != config => {
            lines.push(format!("padding_profile: index={index} config={config}"));
        }
        (None, Some(config)) => lines.push(format!("padding_profile: index=(none) config={config}")),
        _ => {}
    }

    let config_rows = patch
        .get("content_budget")
        .or_else(|| patch.get("contentBudget"))
        .and_then(|v| v.get("rows"))
        .and_then(Value::as_array);
    let index_rows = budget.and_then(|b| b.content_rows.as_ref());
    if let Some(config_rows) = config_rows {
        let config_vec: Vec<i64> = config_rows
            .iter()
            .filter_map(|v| v.as_i64())
            .collect();
        match index_rows {
            Some(index) if index != &config_vec => {
                lines.push(format!("content_rows: index={index:?} config={config_vec:?}"));
            }
            None if !config_vec.is_empty() => {
                lines.push(format!("content_rows: index=(none) config={config_vec:?}"));
            }
            _ => {}
        }
    }

    let config_gap = patch
        .get("content_budget")
        .or_else(|| patch.get("contentBudget"))
        .and_then(|v| v.get("gap"))
        .and_then(Value::as_str)
        .or_else(|| patch.get("gap").and_then(Value::as_str));
    let index_gap = budget.and_then(|b| b.content_gap.as_deref());
    if let Some(config_gap) = config_gap {
        match index_gap {
            Some(index) if index != config_gap => {
                lines.push(format!("content_gap: index={index} config={config_gap}"));
            }
            None => lines.push(format!("content_gap: index=(none) config={config_gap}")),
            _ => {}
        }
    }

    if let Some(theme_layout) = theme_layout.and_then(Value::as_object) {
        if let Some(theme_scope) = theme_layout_scope_for_tuning_key(preview_scope) {
            if let Some(theme_patch) = theme_layout.get(theme_scope.as_str()) {
                if theme_patch_duplicates_tuning(theme_patch, patch) {
                    lines.push(
                        "layoutTuning: 与 ops.themes.*.layout 重复，请合并进 theme profile"
                            .to_string(),
                    );
                }
            }
        }
    }

    if lines.is_empty() {
        if patch.is_object() {
            return Some("layoutTuning: 与 index 一致（或无可比字段）".to_string());
        }
        return None;
    }
    Some(lines.join("\n"))
}

fn theme_layout_scope_for_tuning_key(tuning_key: &str) -> Option<String> {
    let key = tuning_key.trim().trim_matches('/');
    if key.is_empty() {
        return None;
    }
    if key.starts_with("home/") {
        return Some(key.to_string());
    }
    Some(format!("home/T1/{key}"))
}

fn theme_patch_duplicates_tuning(theme_patch: &Value, tuning_patch: &Value) -> bool {
    let Some(theme_obj) = theme_patch.as_object() else {
        return false;
    };
    let Some(tuning_obj) = tuning_patch.as_object() else {
        return false;
    };
    for (field, tuning_value) in tuning_obj {
        if field == "contentBudget" || field == "content_budget" || field == "slotHeight" {
            continue;
        }
        if let Some(theme_value) = theme_obj.get(field) {
            if theme_value == tuning_value {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mei_config::types::OpsConfig;
    use serde_json::json;

    #[test]
    fn layout_tuning_revision_digest_stable_for_same_payload() {
        let mut ops = OpsConfig::default();
        ops.layout_tuning = Some(json!({"left_rail/enforcement": {"slotHeight": 100}}));
        let a = ops_layout_tuning_revision_digest(&ops);
        let b = ops_layout_tuning_revision_digest(&ops);
        assert_eq!(a, b);
        assert!(!a.is_empty());
    }

    #[test]
    fn layout_tuning_overlay_keys_flattens_object() {
        let tuning = json!({"scope/a": {"slotHeight": 1}});
        let keys = layout_tuning_overlay_keys(&tuning);
        assert_eq!(keys.len(), 1);
    }

    #[test]
    fn format_layout_tuning_diff_reports_padding_mismatch() {
        use crate::model::UiBudgetSummary;
        let tuning = json!({
            "left_rail/enforcement": {"paddingProfile": "compact"}
        });
        let budget = UiBudgetSummary {
            padding_profile: Some("dense_strip_100".to_string()),
            ..Default::default()
        };
        let diff = format_layout_tuning_diff(
            "left_rail/enforcement",
            Some(&budget),
            Some(&tuning),
            None,
        )
        .expect("diff");
        assert!(diff.contains("padding_profile"));
        assert!(diff.contains("dense_strip_100"));
        assert!(diff.contains("compact"));
    }

    #[test]
    fn format_layout_tuning_diff_hints_theme_layout_duplicate() {
        let tuning = json!({
            "left_rail/enforcement": {"paddingProfile": "dense_strip_100"}
        });
        let theme_layout = json!({
            "home/T1/left_rail/enforcement": {"paddingProfile": "dense_strip_100"}
        });
        let diff = format_layout_tuning_diff(
            "left_rail/enforcement",
            None,
            Some(&tuning),
            Some(&theme_layout),
        )
        .expect("diff");
        assert!(diff.contains("theme profile"));
    }
}
