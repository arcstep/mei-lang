use serde_json::{Map, Value};

use crate::model::{Diagnostic, LoadedResource, Severity};

use super::super::metric::{expand_page_instance, parse_metric_ref_id};
use super::params::synthesize_board_payload_from_bindings;

/// Host-graph runtime：用 `examples[0].params` + `bindings` 展开 `projection_slots`。
pub fn enrich_runtime_page_instance_projection_slots(
    assembly: &mut Map<String, Value>,
    resources: &[LoadedResource],
    target_file: &str,
) -> Vec<Diagnostic> {
    if assembly
        .get("projection_slots")
        .and_then(Value::as_array)
        .is_some_and(|slots| !slots.is_empty())
    {
        // Slots already expanded; merge author fields onto resolved filter_schema.
        // Never replace wholesale: bindings.filter_schema.rowset_dataset_id is often still
        // an unresolved `{__ref:"param_ref"}` and would stringify to "[object Object]".
        let author = assembly
            .get("bindings")
            .and_then(Value::as_object)
            .and_then(|bindings| bindings.get("filter_schema"))
            .cloned()
            .filter(|value| value.is_object());
        let resolved = assembly.get("filter_schema").cloned().filter(|value| value.is_object());
        if let Some(merged) = merge_author_filter_schema(resolved, author) {
            assembly.insert("filter_schema".to_string(), merged);
        }
        return Vec::new();
    }
    let Some(shell_contract) = assembly
        .get("shell_contract")
        .and_then(Value::as_object)
        .cloned()
    else {
        return Vec::new();
    };
    let layout_mode = shell_contract
        .get("layout_mode")
        .and_then(Value::as_str)
        .unwrap_or("");
    if !matches!(layout_mode, "analytics" | "list_preview") {
        return Vec::new();
    }
    let Some(params) = resolve_runtime_preview_params(assembly) else {
        return vec![Diagnostic {
            severity: Severity::Warning,
            code: "board_runtime_preview_params_missing".to_string(),
            message: format!(
                "board assembly `{target_file}` skipped projection_slots: examples[0].params.metric is required"
            ),
            source_path: Some(target_file.to_string()),
        }];
    };
    let Some(bindings) = assembly.get("bindings").and_then(Value::as_object) else {
        return vec![Diagnostic {
            severity: Severity::Warning,
            code: "board_runtime_bindings_missing".to_string(),
            message: format!(
                "board assembly `{target_file}` skipped projection_slots: bindings are required"
            ),
            source_path: Some(target_file.to_string()),
        }];
    };
    let scene = assembly
        .get("scene")
        .and_then(Value::as_str)
        .map(str::to_string);
    let Some(board_payload) = synthesize_board_payload_from_bindings(
        bindings,
        &shell_contract,
        &params,
        scene.as_deref(),
    ) else {
        return vec![Diagnostic {
            severity: Severity::Warning,
            code: "board_runtime_payload_missing".to_string(),
            message: format!(
                "board assembly `{target_file}` skipped projection_slots: could not synthesize board payload"
            ),
            source_path: Some(target_file.to_string()),
        }];
    };
    let mut expand_diagnostics = Vec::new();
    let Some(expanded) = expand_page_instance(
        &board_payload,
        resources,
        None,
        &mut expand_diagnostics,
        target_file,
    ) else {
        let mut diagnostics = expand_diagnostics;
        if diagnostics.is_empty() {
            diagnostics.push(Diagnostic {
                severity: Severity::Warning,
                code: "board_runtime_expand_failed".to_string(),
                message: format!(
                    "board assembly `{target_file}` skipped projection_slots: expand_page_instance returned no slots"
                ),
                source_path: Some(target_file.to_string()),
            });
        }
        return diagnostics;
    };
    let (slots, expanded_filter_schema, _) = expanded;
    if slots.is_empty() {
        let mut diagnostics = expand_diagnostics;
        diagnostics.push(Diagnostic {
            severity: Severity::Warning,
            code: "board_runtime_slots_empty".to_string(),
            message: format!(
                "board assembly `{target_file}` skipped projection_slots: expanded to an empty list"
            ),
            source_path: Some(target_file.to_string()),
        });
        return diagnostics;
    }
    assembly.insert(
        "projection_slots".to_string(),
        Value::Array(slots.into_iter().map(Value::Object).collect()),
    );
    // Merge author fields/preset onto expanded schema; keep resolved string rowset_dataset_id.
    let author_filter_schema = assembly
        .get("bindings")
        .and_then(Value::as_object)
        .and_then(|bindings| bindings.get("filter_schema"))
        .cloned()
        .filter(|value| value.is_object());
    let resolved = expanded_filter_schema.filter(|value| !value.is_null() && value.is_object());
    if let Some(filter_schema) = merge_author_filter_schema(resolved, author_filter_schema) {
        assembly.insert("filter_schema".to_string(), filter_schema);
    }
    assembly.insert("preview_params".to_string(), Value::Object(params));
    expand_diagnostics
}

/// Overlay author `fields` / `preset_filter_count` / `allow_extra` onto a resolved schema.
/// Keep a string `rowset_dataset_id` from either side; never promote unresolved param_ref objects.
fn merge_author_filter_schema(resolved: Option<Value>, author: Option<Value>) -> Option<Value> {
    match (resolved, author) {
        (None, None) => None,
        (Some(resolved), None) => Some(resolved),
        (None, Some(author)) => Some(strip_unresolved_rowset_dataset_id(author)),
        (Some(Value::Object(mut resolved_map)), Some(Value::Object(author_map))) => {
            let resolved_rowset = string_rowset_dataset_id(&resolved_map);
            let author_rowset = string_rowset_dataset_id(&author_map);
            for (key, value) in author_map {
                if key == "rowset_dataset_id" || key == "rowsetDatasetId" {
                    continue;
                }
                resolved_map.insert(key, value);
            }
            if let Some(rowset) = author_rowset.or(resolved_rowset) {
                resolved_map.insert("rowset_dataset_id".to_string(), Value::String(rowset));
                resolved_map.remove("rowsetDatasetId");
            } else {
                resolved_map.remove("rowset_dataset_id");
                resolved_map.remove("rowsetDatasetId");
            }
            Some(Value::Object(resolved_map))
        }
        (Some(resolved), Some(_)) => Some(resolved),
    }
}

fn string_rowset_dataset_id(map: &Map<String, Value>) -> Option<String> {
    map.get("rowset_dataset_id")
        .or_else(|| map.get("rowsetDatasetId"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn strip_unresolved_rowset_dataset_id(mut schema: Value) -> Value {
    let Some(map) = schema.as_object_mut() else {
        return schema;
    };
    if string_rowset_dataset_id(map).is_none() {
        map.remove("rowset_dataset_id");
        map.remove("rowsetDatasetId");
    }
    schema
}

fn resolve_runtime_preview_params(assembly: &Map<String, Value>) -> Option<Map<String, Value>> {
    if let Some(params) = assembly
        .get("preview_params")
        .and_then(Value::as_object)
        .cloned()
        .filter(|params| params_has_metric(params))
    {
        return Some(params);
    }
    let params = assembly
        .get("examples")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(|example| example.as_object())
        .and_then(|example| example.get("params"))
        .and_then(|value| value.as_object())
        .cloned()?;
    params_has_metric(&params).then_some(params)
}

fn params_has_metric(params: &Map<String, Value>) -> bool {
    params
        .get("metric")
        .and_then(|metric| parse_metric_ref_id(metric))
        .is_some()
}
