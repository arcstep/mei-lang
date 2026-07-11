use super::{
    build_root_metric_slot, default_detail_view, default_preview_view,
    first_slot_zone_for_component, slot_from_board_view, validate_analytics_slots,
    validate_scene_shell_slots,
};

use serde_json::{Map, Value};

use crate::model::{Diagnostic, Severity};

pub(super) fn expand_board_analytics_slots(
    root_metric_id: &str,
    root_dataset_id: &str,
    contract: Option<&Map<String, Value>>,
    shell: &Map<String, Value>,
    charts: Option<&Value>,
    detail: Option<&Value>,
    include_hero: bool,
    resources: &[crate::model::LoadedResource],
    world_hint: Option<&Value>,
    diagnostics: &mut Vec<Diagnostic>,
    target_file: &str,
) -> Option<Vec<Map<String, Value>>> {
    let slots = expand_board_zoned_slots(
        root_metric_id,
        root_dataset_id,
        contract,
        shell,
        charts,
        detail,
        None,
        include_hero,
        resources,
        world_hint,
        diagnostics,
        target_file,
    )?;
    validate_analytics_slots(root_metric_id, &slots, diagnostics, target_file)?;
    Some(slots)
}

pub(super) fn expand_board_list_preview_slots(
    root_metric_id: &str,
    root_dataset_id: &str,
    contract: Option<&Map<String, Value>>,
    shell: &Map<String, Value>,
    charts: Option<&Value>,
    list: Option<&Value>,
    preview: Option<&Value>,
    resources: &[crate::model::LoadedResource],
    world_hint: Option<&Value>,
    diagnostics: &mut Vec<Diagnostic>,
    target_file: &str,
) -> Option<Vec<Map<String, Value>>> {
    let preview_value = preview.cloned().or_else(|| default_preview_view(contract));
    expand_board_zoned_slots(
        root_metric_id,
        root_dataset_id,
        contract,
        shell,
        charts,
        list,
        preview_value.as_ref(),
        false,
        resources,
        world_hint,
        diagnostics,
        target_file,
    )
}

pub(super) fn expand_board_zoned_slots(
    root_metric_id: &str,
    root_dataset_id: &str,
    contract: Option<&Map<String, Value>>,
    shell: &Map<String, Value>,
    charts: Option<&Value>,
    detail: Option<&Value>,
    preview: Option<&Value>,
    include_hero: bool,
    resources: &[crate::model::LoadedResource],
    world_hint: Option<&Value>,
    diagnostics: &mut Vec<Diagnostic>,
    target_file: &str,
) -> Option<Vec<Map<String, Value>>> {
    let mut slots = Vec::new();
    if include_hero {
        let Some(hero_zone_id) = first_slot_zone_for_component(shell, "metric_card") else {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "scene_shell_zone_missing".to_string(),
                message: format!(
                    "board shell for metric `{root_metric_id}` does not declare a metric_card zone for include_hero"
                ),
                source_path: Some(target_file.to_string()),
            });
            return None;
        };
        let mut hero =
            build_root_metric_slot(root_metric_id, root_dataset_id, contract, "metric_card");
        hero.insert("layout_zone".to_string(), Value::String(hero_zone_id));
        slots.push(hero);
    }

    let empty_charts: &[Value] = &[];
    let chart_entries = charts
        .and_then(Value::as_array)
        .map(|items| items.as_slice())
        .unwrap_or(empty_charts);
    if !chart_entries.is_empty() {
        let Some(chart_zone_id) = first_slot_zone_for_component(shell, "chart") else {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "scene_shell_zone_missing".to_string(),
                message: format!(
                    "board shell for metric `{root_metric_id}` does not declare a chart zone"
                ),
                source_path: Some(target_file.to_string()),
            });
            return None;
        };
        for (index, entry) in chart_entries.iter().enumerate() {
            let Some(mut slot) = slot_from_board_view(
                entry,
                root_metric_id,
                root_dataset_id,
                contract,
                resources,
                world_hint,
                diagnostics,
                target_file,
                &chart_zone_id,
            ) else {
                return None;
            };
            if slot.get("component").and_then(Value::as_str) != Some("chart") {
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    code: "page_instance_chart_component".to_string(),
                    message: format!(
                        "board chart view #{index} for metric `{root_metric_id}` must use kind=chart"
                    ),
                    source_path: Some(target_file.to_string()),
                });
                return None;
            }
            slot.insert(
                "layout_zone".to_string(),
                Value::String(chart_zone_id.clone()),
            );
            slots.push(slot);
        }
    }

    let detail_value = match detail {
        Some(value) => Some(value.clone()),
        None => {
            if first_slot_zone_for_component(shell, "data_table").is_none() {
                None
            } else {
                default_detail_view(contract, diagnostics, root_metric_id, target_file)
            }
        }
    };
    if let Some(detail_value) = detail_value {
        let Some(detail_zone_id) = first_slot_zone_for_component(shell, "data_table") else {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "scene_shell_zone_missing".to_string(),
                message: format!(
                    "board shell for metric `{root_metric_id}` does not declare a data_table zone"
                ),
                source_path: Some(target_file.to_string()),
            });
            return None;
        };
        let Some(mut detail_slot) = slot_from_board_view(
            &detail_value,
            root_metric_id,
            root_dataset_id,
            contract,
            resources,
            world_hint,
            diagnostics,
            target_file,
            &detail_zone_id,
        ) else {
            return None;
        };
        if detail_slot.get("component").and_then(Value::as_str) != Some("data_table") {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "page_instance_detail_component".to_string(),
                message: format!(
                    "board detail view for metric `{root_metric_id}` must use kind=table"
                ),
                source_path: Some(target_file.to_string()),
            });
            return None;
        }
        detail_slot.insert("layout_zone".to_string(), Value::String(detail_zone_id));
        detail_slot.insert("default".to_string(), Value::Bool(true));
        slots.push(detail_slot);
    }

    if let Some(preview_value) = preview {
        let Some(preview_zone_id) = first_slot_zone_for_component(shell, "summary") else {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "scene_shell_zone_missing".to_string(),
                message: format!(
                    "board shell for metric `{root_metric_id}` does not declare a summary zone"
                ),
                source_path: Some(target_file.to_string()),
            });
            return None;
        };
        let Some(mut preview_slot) = slot_from_board_view(
            preview_value,
            root_metric_id,
            root_dataset_id,
            contract,
            resources,
            world_hint,
            diagnostics,
            target_file,
            &preview_zone_id,
        ) else {
            return None;
        };
        if preview_slot.get("component").and_then(Value::as_str) != Some("summary") {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "page_instance_preview_component".to_string(),
                message: format!(
                    "board preview view for metric `{root_metric_id}` must use kind=summary"
                ),
                source_path: Some(target_file.to_string()),
            });
            return None;
        }
        preview_slot.insert("layout_zone".to_string(), Value::String(preview_zone_id));
        slots.push(preview_slot);
    }

    validate_scene_shell_slots(shell, &slots, diagnostics, target_file, root_metric_id)?;
    Some(slots)
}
