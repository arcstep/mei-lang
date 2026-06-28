use serde_json::{Map, Value};

use crate::model::{Diagnostic, LoadedResource, Severity};

use super::params::synthesize_board_payload_from_bindings;
use super::super::metric::{expand_board_assembly, parse_metric_ref_id};

/// Host-graph runtime：用 `examples[0].params` + `bindings` 展开 `projection_slots`。
pub fn enrich_runtime_board_assembly_projection_slots(
    assembly: &mut Map<String, Value>,
    resources: &[LoadedResource],
    target_file: &str,
) -> Vec<Diagnostic> {
    if assembly
        .get("projection_slots")
        .and_then(Value::as_array)
        .is_some_and(|slots| !slots.is_empty())
    {
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
    let Some(expanded) = expand_board_assembly(
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
                    "board assembly `{target_file}` skipped projection_slots: expand_board_assembly returned no slots"
                ),
                source_path: Some(target_file.to_string()),
            });
        }
        return diagnostics;
    };
    let (slots, filter_schema, _) = expanded;
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
    if let Some(filter_schema) = filter_schema.filter(|value| !value.is_null()) {
        assembly.insert("filter_schema".to_string(), filter_schema);
    }
    assembly.insert("preview_params".to_string(), Value::Object(params));
    expand_diagnostics
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
