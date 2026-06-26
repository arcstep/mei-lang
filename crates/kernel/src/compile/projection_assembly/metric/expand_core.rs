use super::{build_analytics_filter_schema, expand_board_analytics_slots, expand_board_list_preview_slots, expand_board_zoned_slots, lookup_metric_contract, parse_metric_ref_id};

use serde_json::{Map, Value};

use crate::model::{Diagnostic, Severity};

pub(crate) fn expand_board_assembly(
    payload: &Map<String, Value>,
    resources: &[crate::model::LoadedResource],
    world_hint: Option<&Value>,
    diagnostics: &mut Vec<Diagnostic>,
    target_file: &str,
) -> Option<(Vec<Map<String, Value>>, Option<Value>, Option<String>)> {
    let context_ref = payload.get("context")?;
    let metric_id = parse_metric_ref_id(context_ref)?;
    let include_hero = payload
        .get("include_hero")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let rowset_dataset_id = resolve_board_filters_rowset_dataset_id(payload.get("filters"))
        .or_else(|| {
            payload
                .get("rowset_dataset_id")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        });
    let (dataset_id, contract) =
        lookup_metric_contract(metric_id, resources, world_hint, diagnostics, target_file)?;

    let Some(shell) = resolve_scene_shell_contract(payload) else {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "scene_shell_missing".to_string(),
            message: format!(
                "board assembly for context metric `{metric_id}` requires a scene shell contract"
            ),
            source_path: Some(target_file.to_string()),
        });
        return None;
    };
    let layout_mode = scene_shell_layout_mode(&shell);
    let slots_dataset_id = match (layout_mode.as_deref(), rowset_dataset_id.as_deref()) {
        (Some("analytics") | Some("list_preview"), Some(rowset)) => rowset,
        _ => dataset_id.as_str(),
    };

    let slots = match layout_mode.as_deref() {
        Some("generic_tabs") => {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "board_assembly_unsupported_shell".to_string(),
                message: format!(
                    "board assembly for context metric `{metric_id}` does not support generic_tabs shell; use scene.params + scene.bindings + link(scene=..., params=...)"
                ),
                source_path: Some(target_file.to_string()),
            });
            return None;
        }
        Some("analytics") => expand_board_analytics_slots(
            metric_id,
            slots_dataset_id,
            contract.as_ref(),
            &shell,
            payload.get("charts"),
            payload.get("detail"),
            include_hero,
            resources,
            world_hint,
            diagnostics,
            target_file,
        )?,
        Some("list_preview") => expand_board_list_preview_slots(
            metric_id,
            slots_dataset_id,
            contract.as_ref(),
            &shell,
            payload.get("charts"),
            payload.get("detail"),
            payload.get("preview"),
            resources,
            world_hint,
            diagnostics,
            target_file,
        )?,
        Some(_) => expand_board_zoned_slots(
            metric_id,
            &dataset_id,
            contract.as_ref(),
            &shell,
            payload.get("charts"),
            payload.get("detail"),
            payload.get("preview"),
            include_hero,
            resources,
            world_hint,
            diagnostics,
            target_file,
        )?,
        _ => {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "board_assembly_unsupported_shell".to_string(),
                message: format!(
                    "board assembly for context metric `{metric_id}` requires shell.layout_mode"
                ),
                source_path: Some(target_file.to_string()),
            });
            return None;
        }
    };

    let filter_schema = if scene_shell_has_filter_zone(&shell) {
        Some(build_analytics_filter_schema(
            &slots,
            rowset_dataset_id.as_deref(),
            contract.as_ref(),
            payload.get("filters"),
        ))
    } else {
        None
    };
    Some((slots, filter_schema, layout_mode))
}

fn resolve_scene_shell_contract(payload: &Map<String, Value>) -> Option<Map<String, Value>> {
    payload
        .get("shell_contract")
        .and_then(Value::as_object)
        .cloned()
}

fn scene_shell_layout_mode(shell: &Map<String, Value>) -> Option<String> {
    shell
        .get("layout_mode")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn scene_shell_zones<'a>(shell: &'a Map<String, Value>) -> Vec<&'a Map<String, Value>> {
    shell
        .get("zones")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_object)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn scene_zone_id(zone: &Map<String, Value>) -> Option<&str> {
    zone.get("id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn scene_zone_role(zone: &Map<String, Value>) -> Option<&str> {
    zone.get("role")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn scene_shell_has_filter_zone(shell: &Map<String, Value>) -> bool {
    scene_shell_zones(shell)
        .iter()
        .any(|zone| scene_zone_role(zone) == Some("filter"))
}

fn scene_zone_accepts_component(zone: &Map<String, Value>, component: &str) -> bool {
    zone.get("accepts")
        .and_then(Value::as_array)
        .map(|items| {
            items.iter().any(|item| {
                item.as_str()
                    .map(str::trim)
                    .is_some_and(|value| value == component)
            })
        })
        .unwrap_or(false)
}

pub(super) fn first_slot_zone_for_component(shell: &Map<String, Value>, component: &str) -> Option<String> {
    scene_shell_zones(shell)
        .into_iter()
        .find(|zone| {
            matches!(
                scene_zone_role(zone),
                Some("slots") | Some("row_preview") | Some("tab_content")
            ) && scene_zone_accepts_component(zone, component)
        })
        .and_then(scene_zone_id)
        .map(str::to_string)
}

pub(super) fn validate_scene_shell_slots(
    shell: &Map<String, Value>,
    slots: &[Map<String, Value>],
    diagnostics: &mut Vec<Diagnostic>,
    target_file: &str,
    root_metric_id: &str,
) -> Option<()> {
    let mut counts = std::collections::BTreeMap::<String, usize>::new();
    for slot in slots {
        let Some(zone_id) = slot
            .get("layout_zone")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "scene_shell_zone_missing".to_string(),
                message: format!(
                    "board assembly for metric `{root_metric_id}` produced a slot without layout_zone"
                ),
                source_path: Some(target_file.to_string()),
            });
            return None;
        };
        let Some(zone) = scene_shell_zones(shell)
            .into_iter()
            .find(|zone| scene_zone_id(zone) == Some(zone_id))
        else {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "scene_shell_zone_unknown".to_string(),
                message: format!(
                    "board assembly for metric `{root_metric_id}` resolved unknown shell zone `{zone_id}`"
                ),
                source_path: Some(target_file.to_string()),
            });
            return None;
        };
        let component = slot
            .get("component")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or("");
        if !component.is_empty() && !scene_zone_accepts_component(zone, component) {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "scene_shell_zone_component_mismatch".to_string(),
                message: format!(
                    "scene shell zone `{zone_id}` does not accept component `{component}` for metric `{root_metric_id}`"
                ),
                source_path: Some(target_file.to_string()),
            });
            return None;
        }
        *counts.entry(zone_id.to_string()).or_insert(0) += 1;
    }

    for zone in scene_shell_zones(shell) {
        let Some(zone_id) = scene_zone_id(zone) else {
            continue;
        };
        let count = counts.get(zone_id).copied().unwrap_or(0);
        if zone
            .get("required")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            && count == 0
        {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "scene_shell_zone_required".to_string(),
                message: format!(
                    "scene shell zone `{zone_id}` is required for metric `{root_metric_id}`"
                ),
                source_path: Some(target_file.to_string()),
            });
            return None;
        }
        if let Some(max) = zone
            .get("max")
            .and_then(Value::as_u64)
            .map(|value| value as usize)
        {
            if count > max {
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    code: "scene_shell_zone_max".to_string(),
                    message: format!(
                        "scene shell zone `{zone_id}` allows at most {max} items for metric `{root_metric_id}`, got {count}"
                    ),
                    source_path: Some(target_file.to_string()),
                });
                return None;
            }
        }
    }
    Some(())
}

fn resolve_board_filters_rowset_dataset_id(filters: Option<&Value>) -> Option<String> {
    let map = filters?.as_object()?;
    map.get("rowset_dataset_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

