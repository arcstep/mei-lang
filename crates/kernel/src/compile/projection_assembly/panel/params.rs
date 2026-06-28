use std::collections::BTreeMap;

use serde_json::{Map, Value};

use crate::model::{
    Diagnostic, SceneContract, Severity,
};
use crate::typed_refs::{decode_ref_value, RefKind};
use super::super::metric::parse_metric_ref_id;

use super::super::metric::expand_drilldown_tabs;

pub(super) fn validate_and_resolve_scene_params(
    link: &mut Map<String, Value>,
    target_scene_contract: &SceneContract,
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Map<String, Value>> {
    let mut params = link
        .get("params")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let declared_params = target_scene_contract
        .scene
        .params
        .as_object()
        .cloned()
        .unwrap_or_default();
    let mut has_error = false;
    for (param_id, declared_param) in declared_params {
        let Some(param_decl) = declared_param.as_object() else {
            continue;
        };
        let required = param_decl
            .get("required")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let param_type = param_decl
            .get("type")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("string");
        if !params.contains_key(&param_id) {
            if let Some(default_value) = param_decl.get("default") {
                params.insert(param_id.clone(), default_value.clone());
            } else if required {
                has_error = true;
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    code: "scene_link_param_missing".to_string(),
                    message: format!(
                        "link(scene=...) 缺少必填参数 `{param_id}`（scene `{}`）",
                        target_scene_contract.scene.id
                    ),
                    source_path: Some(target_file.to_string()),
                });
            }
        }
        let Some(value) = params.get(&param_id) else {
            continue;
        };
        if value.is_null() {
            if required {
                has_error = true;
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    code: "scene_link_param_missing".to_string(),
                    message: format!(
                        "link(scene=...) 参数 `{param_id}` 不能为空（scene `{}`）",
                        target_scene_contract.scene.id
                    ),
                    source_path: Some(target_file.to_string()),
                });
            }
            continue;
        }
        if !scene_param_value_matches_type(value, param_type) {
            has_error = true;
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "scene_link_param_type_mismatch".to_string(),
                message: format!(
                    "link(scene=...) 参数 `{param_id}` 类型不匹配：期望 `{param_type}`（scene `{}`）",
                    target_scene_contract.scene.id
                ),
                source_path: Some(target_file.to_string()),
            });
        }
    }
    link.insert("params".to_string(), Value::Object(params.clone()));
    if has_error {
        return None;
    }
    Some(params)
}

fn scene_param_value_matches_type(value: &Value, param_type: &str) -> bool {
    match param_type {
        "string" => value.is_string(),
        "number" | "float" | "int" | "integer" => value.is_number(),
        "bool" | "boolean" => value.is_boolean(),
        "dict" | "object" | "map" => value.is_object(),
        "list" | "array" => value.is_array(),
        "metric" => {
            matches!(decode_ref_value(value), Some(expr) if expr.kind == RefKind::Metric)
        }
        _ => true,
    }
}

pub(super) fn resolve_target_scene_id(
    scene_ref: &Map<String, Value>,
    target_scene_ids_by_file: &BTreeMap<String, Vec<String>>,
) -> Option<String> {
    if let Some(scene_id) = scene_ref
        .get("scene_id")
        .or_else(|| scene_ref.get("sceneId"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Some(scene_id.to_string());
    }
    scene_ref
        .get("scene_file")
        .or_else(|| scene_ref.get("sceneFile"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .and_then(|path| target_scene_ids_by_file.get(path))
        .and_then(|scene_ids| match scene_ids.as_slice() {
            [scene_id] => Some(scene_id.clone()),
            _ => None,
        })
}

pub(super) fn synthesize_scene_first_board_payload(
    link: &Map<String, Value>,
    target_scene_contract: &SceneContract,
    shell_contract: &Map<String, Value>,
    params: &Map<String, Value>,
) -> Option<Map<String, Value>> {
    let context = params.get("metric").cloned().filter(|value| {
        parse_metric_ref_id(value).is_some()
    })?;
    let resolved_bindings = resolve_scene_bindings(target_scene_contract, &params);
    let Some(bindings_map) = resolved_bindings.as_object() else {
        return None;
    };
    let zones = shell_contract
        .get("zones")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut payload = Map::new();
    payload.insert(
        "__kind".to_string(),
        Value::String("board_assembly".to_string()),
    );
    if let Some(scene) = link.get("scene") {
        payload.insert("scene".to_string(), scene.clone());
    }
    payload.insert("context".to_string(), context);
    payload.insert(
        "shell_contract".to_string(),
        Value::Object(shell_contract.clone()),
    );

    let mut chart_views = Vec::<Value>::new();
    let mut detail_view: Option<Value> = None;
    let mut preview_view: Option<Value> = None;
    let mut filters: Option<Value> = None;

    for zone in zones.iter().filter_map(Value::as_object) {
        let role = zone
            .get("role")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("");
        let accepts = zone
            .get("accepts")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let has_chart = accepts.iter().any(|value| value.as_str() == Some("chart"));
        let has_data_table = accepts
            .iter()
            .any(|value| value.as_str() == Some("data_table"));
        let has_summary = accepts
            .iter()
            .any(|value| value.as_str() == Some("summary"));
        let aliases = if role == "filter" {
            vec!["filter_schema", "filters"]
        } else if role == "row_preview" || has_summary {
            vec!["preview", "summary"]
        } else if has_chart {
            vec!["chart", "charts"]
        } else if has_data_table {
            vec!["detail", "list", "table"]
        } else if role == "tab_content" {
            vec!["content", "tabs"]
        } else {
            Vec::new()
        };
        let Some(value) = binding_value_for_zone(bindings_map, zone, aliases.as_slice()) else {
            continue;
        };
        if role == "filter" {
            if filters.is_none() {
                filters = Some(value);
            }
            continue;
        }
        if has_chart {
            match value {
                Value::Array(items) => chart_views.extend(items),
                other => chart_views.push(other),
            }
            continue;
        }
        if role == "row_preview" || has_summary {
            if preview_view.is_none() {
                preview_view = Some(value);
            }
            continue;
        }
        if has_data_table && detail_view.is_none() {
            detail_view = Some(value);
        }
    }

    if let Some(mut filters) = filters.or_else(|| {
        params
            .get("rowset_dataset_id")
            .or_else(|| params.get("rowsetDatasetId"))
            .cloned()
            .filter(|value| !value.is_null())
            .map(|rowset_dataset_id| {
                let mut filters = Map::new();
                filters.insert("rowset_dataset_id".to_string(), rowset_dataset_id);
                Value::Object(filters)
            })
    }) {
        merge_rowset_dataset_id_from_params(&mut filters, params);
        payload.insert("filters".to_string(), filters);
    }

    if !chart_views.is_empty() {
        payload.insert("charts".to_string(), Value::Array(chart_views));
    }
    if let Some(detail) = detail_view {
        payload.insert("detail".to_string(), detail);
    }
    if let Some(preview) = preview_view {
        payload.insert("preview".to_string(), preview);
    }
    Some(payload)
}

pub(crate) fn synthesize_board_payload_from_bindings(
    bindings: &Map<String, Value>,
    shell_contract: &Map<String, Value>,
    params: &Map<String, Value>,
    scene: Option<&str>,
) -> Option<Map<String, Value>> {
    let context = params.get("metric").cloned().filter(|value| {
        parse_metric_ref_id(value).is_some()
    })?;
    let bindings_map = resolve_scene_param_refs(&Value::Object(bindings.clone()), params)
        .as_object()
        .cloned()?;
    let zones = shell_contract
        .get("zones")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut payload = Map::new();
    payload.insert(
        "__kind".to_string(),
        Value::String("board_assembly".to_string()),
    );
    if let Some(scene) = scene.filter(|value| !value.is_empty()) {
        payload.insert("scene".to_string(), Value::String(scene.to_string()));
    }
    payload.insert("context".to_string(), context);
    payload.insert(
        "shell_contract".to_string(),
        Value::Object(shell_contract.clone()),
    );

    let mut chart_views = Vec::<Value>::new();
    let mut detail_view: Option<Value> = None;
    let mut preview_view: Option<Value> = None;
    let mut filters: Option<Value> = None;

    for zone in zones.iter().filter_map(Value::as_object) {
        let role = zone
            .get("role")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("");
        let accepts = zone
            .get("accepts")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let has_chart = accepts.iter().any(|value| value.as_str() == Some("chart"));
        let has_data_table = accepts
            .iter()
            .any(|value| value.as_str() == Some("data_table"));
        let has_summary = accepts
            .iter()
            .any(|value| value.as_str() == Some("summary"));
        let aliases = if role == "filter" {
            vec!["filter_schema", "filters"]
        } else if role == "row_preview" || has_summary {
            vec!["preview", "summary"]
        } else if has_chart {
            vec!["chart", "charts"]
        } else if has_data_table {
            vec!["detail", "list", "table"]
        } else if role == "tab_content" {
            vec!["content", "tabs"]
        } else {
            Vec::new()
        };
        let Some(value) = binding_value_for_zone(&bindings_map, zone, aliases.as_slice()) else {
            continue;
        };
        if role == "filter" {
            if filters.is_none() {
                filters = Some(value);
            }
            continue;
        }
        if has_chart {
            match value {
                Value::Array(items) => chart_views.extend(items),
                other => chart_views.push(other),
            }
            continue;
        }
        if role == "row_preview" || has_summary {
            if preview_view.is_none() {
                preview_view = Some(value);
            }
            continue;
        }
        if has_data_table && detail_view.is_none() {
            detail_view = Some(value);
        }
    }

    if let Some(mut filters) = filters.or_else(|| {
        params
            .get("rowset_dataset_id")
            .or_else(|| params.get("rowsetDatasetId"))
            .cloned()
            .filter(|value| !value.is_null())
            .map(|rowset_dataset_id| {
                let mut filters = Map::new();
                filters.insert("rowset_dataset_id".to_string(), rowset_dataset_id);
                Value::Object(filters)
            })
    }) {
        merge_rowset_dataset_id_from_params(&mut filters, params);
        payload.insert("filters".to_string(), filters);
    }

    if !chart_views.is_empty() {
        payload.insert("charts".to_string(), Value::Array(chart_views));
    }
    if let Some(detail) = detail_view {
        payload.insert("detail".to_string(), detail);
    }
    if let Some(preview) = preview_view {
        payload.insert("preview".to_string(), preview);
    }
    Some(payload)
}

pub(super) fn synthesize_scene_first_generic_tabs_slots(
    target_scene_contract: &SceneContract,
    params: &Map<String, Value>,
    resources: &[crate::model::LoadedResource],
    world_hint: Option<&Value>,
    diagnostics: &mut Vec<Diagnostic>,
    target_file: &str,
) -> Option<Vec<Map<String, Value>>> {
    let metric_ref = params.get("metric")?;
    if !matches!(decode_ref_value(metric_ref), Some(expr) if expr.kind == RefKind::Metric) {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "scene_link_param_type_mismatch".to_string(),
            message: format!(
                "scene `{}` 的 generic_tabs 投影要求 params.metric=metric_ref(...)",
                target_scene_contract.scene.id
            ),
            source_path: Some(target_file.to_string()),
        });
        return None;
    }
    let include_hero = target_scene_contract
        .scene
        .local_nav
        .get("include_hero")
        .or_else(|| target_scene_contract.scene.local_nav.get("includeHero"))
        .and_then(Value::as_bool)
        .unwrap_or(true);
    expand_drilldown_tabs(
        metric_ref,
        include_hero,
        None,
        resources,
        world_hint,
        diagnostics,
        target_file,
    )
}

fn merge_rowset_dataset_id_from_params(filters: &mut Value, params: &Map<String, Value>) {
    let Some(map) = filters.as_object_mut() else {
        return;
    };
    if map
        .get("rowset_dataset_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .is_some_and(|value| !value.is_empty())
    {
        return;
    }
    if let Some(rowset) = params
        .get("rowset_dataset_id")
        .or_else(|| params.get("rowsetDatasetId"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        map.insert(
            "rowset_dataset_id".to_string(),
            Value::String(rowset.to_string()),
        );
    }
}

fn resolve_scene_bindings(
    target_scene_contract: &SceneContract,
    params: &Map<String, Value>,
) -> Value {
    let mut merged = target_scene_contract
        .scene
        .examples
        .as_array()
        .and_then(|examples| examples.first())
        .and_then(|example| example.get("bindings"))
        .cloned()
        .unwrap_or_else(|| Value::Object(Map::new()));
    if let Some(scene_bindings) = target_scene_contract.scene.bindings.as_object() {
        let merged_map = merged.as_object_mut().unwrap_or_else(|| unreachable!());
        for (key, value) in scene_bindings {
            merged_map.insert(key.clone(), value.clone());
        }
    }
    resolve_scene_param_refs(&merged, params)
}

fn resolve_scene_param_refs(value: &Value, params: &Map<String, Value>) -> Value {
    match value {
        Value::Object(map) => {
            if let Some(ref_kind) = map.get("__ref").and_then(Value::as_str) {
                if ref_kind == "scene_param" {
                    let param_id = map
                        .get("id")
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .filter(|value| !value.is_empty());
                    if let Some(param_id) = param_id {
                        return params
                            .get(param_id)
                            .cloned()
                            .or_else(|| map.get("default").cloned())
                            .unwrap_or(Value::Null);
                    }
                }
                if ref_kind == "param_ref" {
                    let param_id = map
                        .get("__args")
                        .and_then(|args| args.get("arg0").or_else(|| args.get("id")))
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .filter(|value| !value.is_empty());
                    if let Some(param_id) = param_id {
                        return params
                            .get(param_id)
                            .cloned()
                            .or_else(|| map.get("default").cloned())
                            .unwrap_or(Value::Null);
                    }
                }
            }
            let mut out = Map::new();
            for (key, child) in map {
                out.insert(key.clone(), resolve_scene_param_refs(child, params));
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|item| resolve_scene_param_refs(item, params))
                .collect(),
        ),
        _ => value.clone(),
    }
}

fn binding_value_for_zone(
    bindings: &Map<String, Value>,
    zone: &Map<String, Value>,
    aliases: &[&str],
) -> Option<Value> {
    let mut keys = Vec::<String>::new();
    if let Some(id) = zone
        .get("id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        keys.push(id.to_string());
    }
    if let Some(source) = zone
        .get("source")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        keys.push(source.to_string());
    }
    for alias in aliases {
        let normalized = alias.trim();
        if !normalized.is_empty() {
            keys.push(normalized.to_string());
        }
    }
    keys.into_iter()
        .find_map(|key| bindings.get(&key).cloned().filter(|value| !value.is_null()))
}

