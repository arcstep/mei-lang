use serde_json::Value;

use crate::config_refs::{decode_theme_ref_token, ConfigRefResolver};
use crate::model::{Diagnostic, SceneContract, SceneDecl, Severity, ThemeDecl};

pub(super) fn selected_custom_theme_shared(scene: &SceneDecl, themes: &[ThemeDecl]) -> Value {
    let theme_id = scene
        .theme
        .as_deref()
        .and_then(decode_theme_ref_token)
        .or_else(|| scene.theme.clone())
        .or_else(|| scene.profile.clone())
        .unwrap_or_else(|| "page".to_string());
    themes
        .iter()
        .find(|item| item.id == theme_id)
        .or_else(|| themes.first())
        .map(|theme| theme.shared.clone())
        .unwrap_or_else(|| serde_json::json!({}))
}

pub(super) fn resolve_scene_contract_config_refs(
    contract: &mut SceneContract,
    resolver: &ConfigRefResolver<'_>,
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let raw = match serde_json::to_value(&*contract) {
        Ok(value) => value,
        Err(error) => {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "invalid_config_ref".to_string(),
                message: format!(
                    "failed to serialize scene contract for config ref resolution: {error}"
                ),
                source_path: Some(target_file.to_string()),
            });
            return;
        }
    };
    let resolved = resolver.resolve_config_refs_in_value(&raw, target_file, diagnostics);
    match serde_json::from_value::<SceneContract>(resolved) {
        Ok(next) => *contract = next,
        Err(error) => diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "invalid_config_ref".to_string(),
            message: format!("failed to decode resolved scene contract: {error}"),
            source_path: Some(target_file.to_string()),
        }),
    }
}

pub(super) fn normalize_shared_context(
    value: &mut Value,
    context: &str,
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if value.is_null() {
        *value = serde_json::json!({});
        return;
    }
    if !value.is_object() {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "invalid_shared_context_value".to_string(),
            message: format!("{context} 必须是对象（dict），不能是数组、标量或 null"),
            source_path: Some(target_file.to_string()),
        });
        *value = serde_json::json!({});
        return;
    }
    let mut invalid_paths = Vec::new();
    collect_invalid_shared_paths(value, "$", &mut invalid_paths);
    if invalid_paths.is_empty() {
        return;
    }
    for path in invalid_paths {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "invalid_shared_context_value".to_string(),
            message: format!(
                "{context} 只允许字面量 JSON 值；`{path}` 处检测到 ref 或分析表达式，请改为显式常量"
            ),
            source_path: Some(target_file.to_string()),
        });
    }
    *value = strip_invalid_shared_entries(value);
}

fn collect_invalid_shared_paths(value: &Value, path: &str, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            if map.get("__ref").is_some()
                || (map.get("__kind").and_then(Value::as_str) == Some("analysis_expr"))
            {
                out.push(path.to_string());
                return;
            }
            for (key, child) in map {
                collect_invalid_shared_paths(child, &format!("{path}.{key}"), out);
            }
        }
        Value::Array(items) => {
            for (idx, child) in items.iter().enumerate() {
                collect_invalid_shared_paths(child, &format!("{path}[{idx}]"), out);
            }
        }
        _ => {}
    }
}

fn strip_invalid_shared_entries(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            if map.get("__ref").is_some()
                || (map.get("__kind").and_then(Value::as_str) == Some("analysis_expr"))
            {
                return Value::Null;
            }
            let mut out = serde_json::Map::new();
            for (key, child) in map {
                out.insert(key.clone(), strip_invalid_shared_entries(child));
            }
            Value::Object(out)
        }
        Value::Array(items) => {
            Value::Array(items.iter().map(strip_invalid_shared_entries).collect())
        }
        _ => value.clone(),
    }
}
