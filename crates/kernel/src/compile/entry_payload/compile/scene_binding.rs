use std::collections::BTreeSet;
use std::path::Path;

use serde_json::Value;

use crate::config_refs::{decode_theme_ref_token, walk_value_for_config_refs, ConfigRefResolver};
use crate::model::{Diagnostic, SceneDecl, Severity};
use crate::typed_refs::decode_binding_value;

use super::super::helpers::all_world_resource_decls;

pub(super) fn normalize_scene_params(
    value: &mut Value,
    context: &str,
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if value.is_null() {
        *value = serde_json::json!({});
        return;
    }
    let Some(map) = value.as_object() else {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "invalid_scene_params_value".to_string(),
            message: format!("{context} 必须是对象（dict），键为参数名，值为 param(...)"),
            source_path: Some(target_file.to_string()),
        });
        *value = serde_json::json!({});
        return;
    };
    let mut out = serde_json::Map::new();
    for (key, param) in map {
        let normalized_key = key.trim();
        if normalized_key.is_empty() {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "invalid_scene_param_key".to_string(),
                message: format!("{context} 含空参数名"),
                source_path: Some(target_file.to_string()),
            });
            continue;
        }
        let Some(obj) = param.as_object() else {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "invalid_scene_param_value".to_string(),
                message: format!("{context}.{normalized_key} 必须是 param(...)"),
                source_path: Some(target_file.to_string()),
            });
            continue;
        };
        if obj.get("__kind").and_then(Value::as_str) != Some("scene_param") {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "invalid_scene_param_value".to_string(),
                message: format!("{context}.{normalized_key} 必须是 param(...)"),
                source_path: Some(target_file.to_string()),
            });
            continue;
        }
        let mut normalized = obj.clone();
        let param_type = normalized
            .get("type")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("string")
            .to_string();
        normalized.insert("type".to_string(), Value::String(param_type));
        out.insert(normalized_key.to_string(), Value::Object(normalized));
    }
    *value = Value::Object(out);
}

pub(super) fn normalize_scene_bindings(
    value: &mut Value,
    context: &str,
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if value.is_null() {
        *value = serde_json::json!({});
        return;
    }
    let Some(map) = value.as_object() else {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "invalid_scene_bindings_value".to_string(),
            message: format!("{context} 必须是对象（dict），键为 slot/entry，值为 ref 或内联对象"),
            source_path: Some(target_file.to_string()),
        });
        *value = serde_json::json!({});
        return;
    };
    let mut out = serde_json::Map::new();
    for (key, binding) in map {
        let normalized_key = key.trim();
        if normalized_key.is_empty() {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "invalid_scene_binding_key".to_string(),
                message: format!("{context} 含空 binding key"),
                source_path: Some(target_file.to_string()),
            });
            continue;
        }
        if decode_binding_value(binding).is_none() {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "invalid_scene_binding_value".to_string(),
                message: format!(
                    "{context}.{normalized_key} 必须是 *_ref(...)、非空字符串或内联对象"
                ),
                source_path: Some(target_file.to_string()),
            });
            continue;
        }
        out.insert(normalized_key.to_string(), binding.clone());
    }
    *value = Value::Object(out);
}

pub(super) fn normalize_scene_examples(
    value: &mut Value,
    context: &str,
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if value.is_null() {
        *value = Value::Array(Vec::new());
        return;
    }
    let mut items: Vec<Value> = Vec::new();
    if let Some(array) = value.as_array() {
        items = array.clone();
    } else if let Some(map) = value.as_object() {
        if map.contains_key("bindings") || map.contains_key("id") || map.contains_key("title") {
            items.push(Value::Object(map.clone()));
        } else {
            for (id, entry) in map {
                let Some(entry_map) = entry.as_object() else {
                    diagnostics.push(Diagnostic {
                        severity: Severity::Error,
                        code: "invalid_scene_example_value".to_string(),
                        message: format!("{context}.{id} 必须是对象"),
                        source_path: Some(target_file.to_string()),
                    });
                    continue;
                };
                let mut out = entry_map.clone();
                out.entry("id".to_string())
                    .or_insert_with(|| Value::String(id.to_string()));
                items.push(Value::Object(out));
            }
        }
    } else {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "invalid_scene_examples_value".to_string(),
            message: format!("{context} 必须是数组或对象"),
            source_path: Some(target_file.to_string()),
        });
        *value = Value::Array(Vec::new());
        return;
    }
    let mut normalized = Vec::new();
    for (index, item) in items.into_iter().enumerate() {
        let Some(obj) = item.as_object() else {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "invalid_scene_example_value".to_string(),
                message: format!("{context}[{index}] 必须是对象"),
                source_path: Some(target_file.to_string()),
            });
            continue;
        };
        let mut example = Value::Object(obj.clone());
        let bindings_value = example
            .get("bindings")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}));
        let mut normalized_bindings = bindings_value;
        normalize_scene_bindings(
            &mut normalized_bindings,
            &format!("{context}[{index}].bindings"),
            target_file,
            diagnostics,
        );
        if let Some(example_map) = example.as_object_mut() {
            example_map.insert("bindings".to_string(), normalized_bindings);
        }
        normalized.push(example);
    }
    *value = Value::Array(normalized);
}

pub(super) fn validate_scene_binding_contract(
    scene: &SceneDecl,
    world: &crate::model::WorldDecl,
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let provided_keys = collect_scene_binding_keys(scene);
    for resource in all_world_resource_decls(world) {
        let binding = resource
            .dataset
            .as_ref()
            .and_then(binding_meta_from_value)
            .or_else(|| binding_meta_from_object_field(resource.dataset.as_ref(), "binding"));
        if let Some(meta) = binding {
            validate_binding_meta(
                &resource.id,
                "resource",
                &meta,
                &provided_keys,
                target_file,
                diagnostics,
            );
        }
        if let Some(metrics) = resource.metrics.as_ref() {
            for (metric_id, metric_value) in metrics {
                if let Some(meta) = binding_meta_from_value(metric_value) {
                    validate_binding_meta(
                        metric_id,
                        "metric",
                        &meta,
                        &provided_keys,
                        target_file,
                        diagnostics,
                    );
                }
            }
        }
    }
    for metric_value in &world.metrics {
        let metric_id = metric_value
            .get("key")
            .or_else(|| metric_value.get("id"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .unwrap_or("<unnamed_metric>");
        if let Some(meta) = binding_meta_from_value(metric_value) {
            validate_binding_meta(
                metric_id,
                "metric",
                &meta,
                &provided_keys,
                target_file,
                diagnostics,
            );
        }
    }
}

fn collect_scene_binding_keys(scene: &SceneDecl) -> BTreeSet<String> {
    let mut keys = BTreeSet::new();
    if let Some(map) = scene.bindings.as_object() {
        for key in map.keys() {
            let normalized = key.trim();
            if !normalized.is_empty() {
                keys.insert(normalized.to_string());
            }
        }
    }
    if let Some(examples) = scene.examples.as_array() {
        for example in examples {
            let Some(bindings) = example.get("bindings").and_then(Value::as_object) else {
                continue;
            };
            for key in bindings.keys() {
                let normalized = key.trim();
                if !normalized.is_empty() {
                    keys.insert(normalized.to_string());
                }
            }
        }
    }
    keys
}

fn binding_meta_from_object_field(value: Option<&Value>, field: &str) -> Option<Value> {
    value
        .and_then(Value::as_object)
        .and_then(|map| map.get(field))
        .cloned()
        .filter(|value| value.is_object())
}

pub(super) fn validate_config_refs(
    app_root: &Path,
    entry_decls: &Value,
    scene: Option<&SceneDecl>,
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let config = crate::mei_config::load_mei_config_for_app(app_root, None);
    let resolver = ConfigRefResolver::new(&config);
    walk_value_for_config_refs(entry_decls, target_file, &resolver, diagnostics);
    if let Some(scene) = scene {
        if let Some(theme) = scene.theme.as_deref() {
            if decode_theme_ref_token(theme).is_some() {
                resolver.validate_theme_token(theme, target_file, diagnostics);
            }
        }
    }
}

fn binding_meta_from_value(value: &Value) -> Option<Value> {
    value
        .as_object()
        .and_then(|map| map.get("binding"))
        .cloned()
        .filter(|value| value.is_object())
}

fn validate_binding_meta(
    binding_key: &str,
    subject: &str,
    meta: &Value,
    provided_keys: &BTreeSet<String>,
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(meta_map) = meta.as_object() else {
        return;
    };
    let enabled = meta_map
        .get("enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if !enabled {
        return;
    }
    if let Some(replace) = meta_map.get("replace").and_then(Value::as_str) {
        if replace != "source" && replace != "full" {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "invalid_binding_replace_mode".to_string(),
                message: format!(
                    "{subject} `{binding_key}` 的 binding.replace 仅支持 `source` 或 `full`"
                ),
                source_path: Some(target_file.to_string()),
            });
        }
    }
    if let Some(accept) = meta_map.get("accept") {
        let Some(accept_map) = accept.as_object() else {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "invalid_binding_accept".to_string(),
                message: format!("{subject} `{binding_key}` 的 binding.accept 必须是对象"),
                source_path: Some(target_file.to_string()),
            });
            return;
        };
        for key in accept_map.keys() {
            if key != "shape" && key != "schema" && key != "kind" {
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    code: "invalid_binding_accept_key".to_string(),
                    message: format!(
                        "{subject} `{binding_key}` 的 binding.accept 仅支持 `shape` / `schema` / `kind`"
                    ),
                    source_path: Some(target_file.to_string()),
                });
            }
        }
    }
    if meta_map
        .get("required")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        && !provided_keys.contains(binding_key)
    {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "missing_required_scene_binding".to_string(),
            message: format!(
                "{subject} `{binding_key}` 声明了 required binding，但 scene.bindings / scene.examples 未提供对应条目"
            ),
            source_path: Some(target_file.to_string()),
        });
    }
}
