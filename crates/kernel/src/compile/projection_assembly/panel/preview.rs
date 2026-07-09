use super::{scene_shell_contract_from_scene_contract, synthesize_scene_first_board_payload};


use serde_json::{Map, Value};

use crate::model::{
    Diagnostic, SceneContract, Severity,
};
use super::super::metric::expand_page_instance;
use super::super::metric::parse_metric_ref_id;

/// Manage/build 预览：用 scene `examples[0].params` 展开 projection_slots，供无 caller 时装配 filter/chart/detail。
pub(crate) fn enrich_scene_projection_assembly_preview(
    assembly: &mut Map<String, Value>,
    contract: &SceneContract,
    resources: &[crate::model::LoadedResource],
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let scene_id = contract.scene.id.clone();
    let Some(shell_contract) = scene_shell_contract_from_scene_contract(contract) else {
        return;
    };
    let layout_mode = shell_contract
        .get("layout_mode")
        .and_then(Value::as_str)
        .unwrap_or("");
    if !matches!(layout_mode, "analytics" | "list_preview") {
        return;
    }
    let Some(params) = resolve_preview_example_params(contract) else {
        diagnostics.push(Diagnostic {
            severity: Severity::Warning,
            code: "board_preview_params_missing".to_string(),
            message: format!(
                "scene `{scene_id}` analytics preview assembly skipped: examples[0].params.metric is required"
            ),
            source_path: Some(target_file.to_string()),
        });
        return;
    };
    let link = Map::new();
    let Some(board_payload) =
        synthesize_scene_first_board_payload(&link, contract, &shell_contract, &params)
    else {
        diagnostics.push(Diagnostic {
            severity: Severity::Warning,
            code: "board_preview_payload_missing".to_string(),
            message: format!(
                "scene `{scene_id}` analytics preview assembly skipped: could not synthesize board payload from bindings"
            ),
            source_path: Some(target_file.to_string()),
        });
        return;
    };
    let world_hint = contract.scene.world.clone();
    let mut expand_diagnostics = Vec::new();
    let Some(expanded) = expand_page_instance(
        &board_payload,
        resources,
        world_hint.as_ref(),
        &mut expand_diagnostics,
        target_file,
    ) else {
        let had_expand_diagnostics = !expand_diagnostics.is_empty();
        diagnostics.extend(expand_diagnostics);
        if !had_expand_diagnostics {
            diagnostics.push(Diagnostic {
                severity: Severity::Warning,
                code: "board_preview_expand_failed".to_string(),
                message: format!(
                    "scene `{scene_id}` analytics preview assembly skipped: expand_page_instance returned no slots"
                ),
                source_path: Some(target_file.to_string()),
            });
        }
        return;
    };
    diagnostics.extend(expand_diagnostics);
    let (slots, filter_schema, _) = expanded;
    if slots.is_empty() {
        diagnostics.push(Diagnostic {
            severity: Severity::Warning,
            code: "board_preview_slots_empty".to_string(),
            message: format!(
                "scene `{scene_id}` analytics preview assembly skipped: projection slots expanded to an empty list"
            ),
            source_path: Some(target_file.to_string()),
        });
        return;
    }
    assembly.insert(
        "projection_slots".to_string(),
        Value::Array(slots.into_iter().map(Value::Object).collect()),
    );
    if let Some(filter_schema) = filter_schema.filter(|value| !value.is_null()) {
        assembly.insert("filter_schema".to_string(), filter_schema);
    }
    assembly.insert("preview_params".to_string(), Value::Object(params));
}

fn resolve_preview_example_params(contract: &SceneContract) -> Option<Map<String, Value>> {
    let params = contract
        .scene
        .examples
        .as_array()
        .and_then(|items| items.first())
        .and_then(|example| example.as_object())
        .and_then(|example| example.get("params"))
        .and_then(|value| value.as_object())
        .cloned()?;
    params
        .get("metric")
        .is_some_and(|metric| parse_metric_ref_id(metric).is_some())
        .then_some(params)
}
