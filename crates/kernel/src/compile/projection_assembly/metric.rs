use serde_json::{Map, Value};

use crate::compile::materialize::{
    imported_world_metrics_resource_id, resolve_runtime_metric_def_key,
};
use crate::model::{Diagnostic, Severity};

pub(super) fn expand_board_assembly(
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
    shell.get("layout_mode")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn scene_shell_zones<'a>(shell: &'a Map<String, Value>) -> Vec<&'a Map<String, Value>> {
    shell.get("zones")
        .and_then(Value::as_array)
        .map(|items| {
            items.iter()
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

fn first_slot_zone_for_component(shell: &Map<String, Value>, component: &str) -> Option<String> {
    scene_shell_zones(shell)
        .into_iter()
        .find(|zone| {
            matches!(scene_zone_role(zone), Some("slots") | Some("row_preview") | Some("tab_content"))
                && scene_zone_accepts_component(zone, component)
        })
        .and_then(scene_zone_id)
        .map(str::to_string)
}

fn validate_scene_shell_slots(
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
        if let Some(max) = zone.get("max").and_then(Value::as_u64).map(|value| value as usize) {
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

fn expand_board_analytics_slots(
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

fn expand_board_list_preview_slots(
    root_metric_id: &str,
    root_dataset_id: &str,
    contract: Option<&Map<String, Value>>,
    shell: &Map<String, Value>,
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
        None,
        list,
        preview_value.as_ref(),
        false,
        resources,
        world_hint,
        diagnostics,
        target_file,
    )
}

fn expand_board_zoned_slots(
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
        let mut hero = build_root_metric_slot(
            root_metric_id,
            root_dataset_id,
            contract,
            "metric_card",
        );
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
                    code: "board_assembly_chart_component".to_string(),
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
                code: "board_assembly_detail_component".to_string(),
                message: format!(
                    "board detail view for metric `{root_metric_id}` must use kind=table"
                ),
                source_path: Some(target_file.to_string()),
            });
            return None;
        }
        detail_slot.insert(
            "layout_zone".to_string(),
            Value::String(detail_zone_id),
        );
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
                code: "board_assembly_preview_component".to_string(),
                message: format!(
                    "board preview view for metric `{root_metric_id}` must use kind=summary"
                ),
                source_path: Some(target_file.to_string()),
            });
            return None;
        }
        preview_slot.insert(
            "layout_zone".to_string(),
            Value::String(preview_zone_id),
        );
        slots.push(preview_slot);
    }

    validate_scene_shell_slots(shell, &slots, diagnostics, target_file, root_metric_id)?;
    Some(slots)
}

fn default_preview_view(contract: Option<&Map<String, Value>>) -> Option<Value> {
    let block_ref = default_detail_entry(contract)?;
    let block_id = block_ref.as_str()?.to_string();
    let mut source = Map::new();
    source.insert(
        "__ref".to_string(),
        Value::String("explain_block".to_string()),
    );
    source.insert("id".to_string(), Value::String(block_id));
    let mut view = Map::new();
    view.insert(
        "__kind".to_string(),
        Value::String("board_view".to_string()),
    );
    view.insert("kind".to_string(), Value::String("summary".to_string()));
    view.insert("source".to_string(), Value::Object(source));
    Some(Value::Object(view))
}

fn default_detail_view(
    contract: Option<&Map<String, Value>>,
    diagnostics: &mut Vec<Diagnostic>,
    root_metric_id: &str,
    target_file: &str,
) -> Option<Value> {
    let block_ref = default_detail_entry(contract).or_else(|| {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "board_assembly_missing_detail".to_string(),
            message: format!(
                "board assembly for metric `{root_metric_id}` requires detail=build_view(...) or an explain detail block"
            ),
            source_path: Some(target_file.to_string()),
        });
        None
    })?;
    let block_id = block_ref.as_str()?.to_string();
    let mut source = Map::new();
    source.insert(
        "__ref".to_string(),
        Value::String("explain_block".to_string()),
    );
    source.insert("id".to_string(), Value::String(block_id));
    let mut view = Map::new();
    view.insert(
        "__kind".to_string(),
        Value::String("board_view".to_string()),
    );
    view.insert("kind".to_string(), Value::String("table".to_string()));
    view.insert("source".to_string(), Value::Object(source));
    Some(Value::Object(view))
}

fn slot_from_board_view(
    entry: &Value,
    root_metric_id: &str,
    root_dataset_id: &str,
    contract: Option<&Map<String, Value>>,
    resources: &[crate::model::LoadedResource],
    world_hint: Option<&Value>,
    diagnostics: &mut Vec<Diagnostic>,
    target_file: &str,
    zone: &str,
) -> Option<Map<String, Value>> {
    let map = entry.as_object()?;
    if map.get("__kind").and_then(Value::as_str) != Some("board_view") {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "board_assembly_invalid_view".to_string(),
            message: format!(
                "board {zone} entry for metric `{root_metric_id}` must be build_view(...)"
            ),
            source_path: Some(target_file.to_string()),
        });
        return None;
    }
    let view_kind = map
        .get("kind")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())?;
    let component = view_kind_to_component(view_kind)?;
    if view_kind == "chart" {
        let has_chart_kind = map
            .get("chart_kind")
            .and_then(Value::as_str)
            .map(str::trim)
            .is_some_and(|s| !s.is_empty());
        if !has_chart_kind {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "board_assembly_chart_kind_required".to_string(),
                message: format!(
                    "board chart view for metric `{root_metric_id}` requires explicit chart_kind"
                ),
                source_path: Some(target_file.to_string()),
            });
            return None;
        }
    }

    let mut slot = resolve_view_source_to_slot(
        map.get("source")?,
        root_metric_id,
        root_dataset_id,
        contract,
        resources,
        world_hint,
        diagnostics,
        target_file,
        zone,
    )?;
    slot.insert("component".to_string(), Value::String(component.to_string()));
    if let Some(label) = map.get("label").and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()) {
        slot.insert("label".to_string(), Value::String(label.to_string()));
    }
    if let Some(chart_kind) = map.get("chart_kind") {
        slot.insert("chart_kind".to_string(), chart_kind.clone());
    }
    if let Some(mapping) = map.get("mapping") {
        slot.insert("mapping".to_string(), mapping.clone());
    }
    if let Some(top_n) = map.get("top_n") {
        slot.insert("top_n".to_string(), top_n.clone());
    }
    if let Some(columns) = map.get("columns") {
        slot.insert("fields".to_string(), columns.clone());
    }
    if let Some(column_state) = map.get("column_state") {
        slot.insert("column_state".to_string(), column_state.clone());
    }
    if let Some(page_size) = map.get("page_size").or_else(|| map.get("pageSize")) {
        slot.insert("page_size".to_string(), page_size.clone());
    }
    if let Some(column_template) = map
        .get("column_template")
        .or_else(|| map.get("columnTemplate"))
    {
        slot.insert("column_template".to_string(), column_template.clone());
    }
    if let Some(column_formats) = map
        .get("column_formats")
        .or_else(|| map.get("columnFormats"))
    {
        slot.insert("column_formats".to_string(), column_formats.clone());
    }
    Some(slot)
}

fn view_kind_to_component(view_kind: &str) -> Option<&'static str> {
    match view_kind {
        "chart" => Some("chart"),
        "table" => Some("data_table"),
        "metric_card" => Some("metric_card"),
        "summary" => Some("summary"),
        _ => None,
    }
}

fn resolve_view_source_to_slot(
    source: &Value,
    root_metric_id: &str,
    root_dataset_id: &str,
    contract: Option<&Map<String, Value>>,
    resources: &[crate::model::LoadedResource],
    world_hint: Option<&Value>,
    diagnostics: &mut Vec<Diagnostic>,
    target_file: &str,
    zone: &str,
) -> Option<Map<String, Value>> {
    if let Some(source_map) = source.as_object() {
        let source_ref = source_map.get("__ref").and_then(Value::as_str);
        if matches!(source_ref, Some("explain_block") | Some("explain_metric")) {
            let block_id = source_map.get("id").and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty())?;
            let Some(block) = find_explain_block(contract, block_id) else {
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    code: "board_assembly_unknown_explain_block".to_string(),
                    message: format!(
                        "board {zone} source `{block_id}` for metric `{root_metric_id}` does not match any explain block"
                    ),
                    source_path: Some(target_file.to_string()),
                });
                return None;
            };
            return Some(slot_from_explain_block(
                block,
                root_metric_id,
                root_dataset_id,
            ));
        }
        if source_ref == Some("metric") {
            let metric_id = parse_metric_ref_id(source)?;
            let (dataset_id, contract) =
                lookup_metric_contract(metric_id, resources, world_hint, diagnostics, target_file)?;
            return Some(build_slot_from_root(
                metric_id,
                &dataset_id,
                contract.as_ref(),
                "chart",
                None,
            ));
        }
        if source_map.get("__kind").and_then(Value::as_str) == Some("projection_slot") {
            return lower_projection_slot(source_map, resources, world_hint, diagnostics, target_file);
        }
    }
    if let Some(block_ref) = source.as_str().map(str::trim).filter(|s| !s.is_empty()) {
        let Some(block) = find_explain_block(contract, block_ref) else {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "board_assembly_unknown_explain_block".to_string(),
                message: format!(
                    "board {zone} source `{block_ref}` for metric `{root_metric_id}` does not match any explain block"
                ),
                source_path: Some(target_file.to_string()),
            });
            return None;
        };
        return Some(slot_from_explain_block(
            block,
            root_metric_id,
            root_dataset_id,
        ));
    }
    diagnostics.push(Diagnostic {
        severity: Severity::Error,
        code: "board_assembly_invalid_source".to_string(),
        message: format!(
            "board {zone} view for metric `{root_metric_id}` requires source=explain_ref(...) or metric_ref(...)"
        ),
        source_path: Some(target_file.to_string()),
    });
    None
}

fn validate_analytics_slots(
    metric_id: &str,
    slots: &[Map<String, Value>],
    diagnostics: &mut Vec<Diagnostic>,
    target_file: &str,
) -> Option<()> {
    if slots.iter().all(|slot| {
        slot.get("layout_zone").and_then(Value::as_str) != Some("detail")
    }) {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "analytics_projection_missing_detail".to_string(),
            message: format!(
                "analytics drilldown for metric `{metric_id}` requires at least one detail slot"
            ),
            source_path: Some(target_file.to_string()),
        });
        return None;
    }
    if slots.is_empty() {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "projection_slots_empty".to_string(),
            message: format!("no projection slots for metric `{metric_id}`"),
            source_path: Some(target_file.to_string()),
        });
        return None;
    }
    Some(())
}

fn default_detail_entry(contract: Option<&Map<String, Value>>) -> Option<Value> {
    let blocks = contract?
        .get("blocks")
        .and_then(Value::as_array)?;
    for block in blocks {
        let block_map = block.as_object()?;
        let support_role = block_map
            .get("support_role")
            .or_else(|| block_map.get("kind"))
            .and_then(Value::as_str)
            .unwrap_or("");
        if support_role == "detail" {
            let block_ref = block_map
                .get("id")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|id| !id.is_empty())
                .unwrap_or("detail");
            return Some(Value::String(block_ref.to_string()));
        }
    }
    None
}

fn find_explain_block<'a>(
    contract: Option<&'a Map<String, Value>>,
    block_ref: &str,
) -> Option<&'a Map<String, Value>> {
    let blocks = contract?.get("blocks").and_then(Value::as_array)?;
    let mut role_match: Option<&'a Map<String, Value>> = None;
    for block in blocks {
        let block_map = block.as_object()?;
        let support_role = block_map
            .get("support_role")
            .or_else(|| block_map.get("kind"))
            .and_then(Value::as_str)
            .unwrap_or("");
        if matches!(support_role, "note" | "definition") {
            continue;
        }
        if block_map
            .get("id")
            .and_then(Value::as_str)
            .is_some_and(|id| id == block_ref)
        {
            return Some(block_map);
        }
        if role_match.is_none() && support_role == block_ref {
            role_match = Some(block_map);
        }
    }
    role_match
}

fn is_world_metrics_owner_dataset_id(dataset_id: &str) -> bool {
    let dataset_id = dataset_id.trim();
    dataset_id == imported_world_metrics_resource_id("")
        || dataset_id.starts_with("__world_metrics__::")
}

fn slot_from_explain_block(
    block_map: &Map<String, Value>,
    metric_id: &str,
    dataset_id: &str,
) -> Map<String, Value> {
    let support_role = block_map
        .get("support_role")
        .or_else(|| block_map.get("kind"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let component = component_for_support_role(&support_role, block_map);
    let label = block_map
        .get("label")
        .and_then(Value::as_str)
        .map(str::to_string);
    let slot_metric_id = block_map
        .get("metric_id")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| metric_id.to_string());
    let block_dataset_id = block_map
        .get("dataset_id")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or_else(|| {
            block_map
                .get("runtime_ref")
                .and_then(Value::as_object)
                .and_then(|runtime_ref| runtime_ref.get("dataset_id"))
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        });
    let slot_dataset_id = match block_dataset_id.as_deref() {
        Some(block_ds)
            if is_world_metrics_owner_dataset_id(block_ds)
                && !dataset_id.is_empty()
                && !is_world_metrics_owner_dataset_id(dataset_id) =>
        {
            dataset_id.to_string()
        }
        Some(block_ds) => block_ds.to_string(),
        None => dataset_id.to_string(),
    };
    let slot_id = block_map
        .get("id")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| support_role.clone());
    let mut slot = Map::new();
    slot.insert("id".to_string(), Value::String(slot_id));
    slot.insert("metric_id".to_string(), Value::String(slot_metric_id));
    slot.insert("dataset_id".to_string(), Value::String(slot_dataset_id));
    slot.insert("component".to_string(), Value::String(component));
    slot.insert(
        "support_role".to_string(),
        Value::String(support_role.clone()),
    );
    if let Some(label) = label {
        slot.insert("label".to_string(), Value::String(label));
    }
    if let Some(fields) = block_map.get("fields") {
        slot.insert("fields".to_string(), fields.clone());
    }
    if let Some(by) = block_map.get("by") {
        slot.insert("by".to_string(), by.clone());
    }
    if let Some(mapping) = block_map.get("mapping") {
        slot.insert("mapping".to_string(), mapping.clone());
    }
    if let Some(top_n) = block_map.get("top_n") {
        slot.insert("top_n".to_string(), top_n.clone());
    }
    if let Some(block_id) = block_map.get("id").and_then(Value::as_str) {
        slot.insert(
            "explain_block_id".to_string(),
            Value::String(block_id.to_string()),
        );
    }
    slot
}

pub(super) fn expand_drilldown_tabs(
    metric_ref: &Value,
    include_hero: bool,
    _default_slot: Option<usize>,
    resources: &[crate::model::LoadedResource],
    world_hint: Option<&Value>,
    diagnostics: &mut Vec<Diagnostic>,
    target_file: &str,
) -> Option<Vec<Map<String, Value>>> {
    let metric_id = parse_metric_ref_id(metric_ref)?;
    let (dataset_id, contract) =
        lookup_metric_contract(metric_id, resources, world_hint, diagnostics, target_file)?;

    let mut slots = Vec::new();
    if include_hero {
        slots.push(build_root_metric_slot(
            metric_id,
            &dataset_id,
            contract.as_ref(),
            "metric_card",
        ));
    }
    slots.extend(build_explain_slots(
        metric_id,
        &dataset_id,
        contract.as_ref(),
    ));

    if slots.is_empty() {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "projection_slots_empty".to_string(),
            message: format!("no projection slots for metric `{metric_id}`"),
            source_path: Some(target_file.to_string()),
        });
        return None;
    }

    Some(slots)
}

pub(super) fn lower_projection_slot(
    map: &Map<String, Value>,
    resources: &[crate::model::LoadedResource],
    world_hint: Option<&Value>,
    diagnostics: &mut Vec<Diagnostic>,
    target_file: &str,
) -> Option<Map<String, Value>> {
    let metric_ref = map.get("metric")?;
    let metric_id = parse_metric_ref_id(metric_ref)?;
    let (dataset_id, contract) =
        lookup_metric_contract(metric_id, resources, world_hint, diagnostics, target_file)?;
    let component = map
        .get("as")
        .or_else(|| map.get("component"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| "metric_card".to_string());
    let label = map.get("label").and_then(Value::as_str).map(str::to_string);
    Some(build_slot_from_root(
        metric_id,
        &dataset_id,
        contract.as_ref(),
        &component,
        label,
    ))
}

fn build_explain_slots(
    metric_id: &str,
    dataset_id: &str,
    contract: Option<&Map<String, Value>>,
) -> Vec<Map<String, Value>> {
    let Some(contract) = contract else {
        return Vec::new();
    };
    let Some(blocks) = contract.get("blocks").and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut slots = Vec::new();
    for block in blocks {
        let Some(block_map) = block.as_object() else {
            continue;
        };
        let support_role = block_map
            .get("support_role")
            .or_else(|| block_map.get("kind"))
            .and_then(Value::as_str)
            .unwrap_or("");
        if matches!(support_role, "note" | "definition") {
            continue;
        }
        slots.push(slot_from_explain_block(block_map, metric_id, dataset_id));
    }
    slots
}

fn build_root_metric_slot(
    metric_id: &str,
    dataset_id: &str,
    contract: Option<&Map<String, Value>>,
    component: &str,
) -> Map<String, Value> {
    let label = contract
        .and_then(|c| c.get("title").and_then(Value::as_str))
        .map(str::to_string);
    build_slot_from_root(metric_id, dataset_id, contract, component, label)
}

fn build_slot_from_root(
    metric_id: &str,
    dataset_id: &str,
    contract: Option<&Map<String, Value>>,
    component: &str,
    label: Option<String>,
) -> Map<String, Value> {
    let mut slot = Map::new();
    slot.insert("id".to_string(), Value::String("metric".to_string()));
    slot.insert(
        "metric_id".to_string(),
        Value::String(metric_id.to_string()),
    );
    slot.insert(
        "dataset_id".to_string(),
        Value::String(dataset_id.to_string()),
    );
    slot.insert(
        "component".to_string(),
        Value::String(component.to_string()),
    );
    slot.insert(
        "support_role".to_string(),
        Value::String("metric".to_string()),
    );
    let resolved_label = label.or_else(|| {
        contract
            .and_then(|c| c.get("title").and_then(Value::as_str))
            .map(str::to_string)
    });
    if let Some(label) = resolved_label {
        slot.insert("label".to_string(), Value::String(label));
    }
    slot
}

fn component_for_support_role(support_role: &str, block: &Map<String, Value>) -> String {
    match support_role {
        "composition" | "trend" | "attribution" => "chart".to_string(),
        "detail" => "data_table".to_string(),
        "dataframe" | "timeseries" | "series" => {
            if block
                .get("fields")
                .and_then(Value::as_array)
                .is_some_and(|fields| {
                    fields.iter().any(|field| {
                        field
                            .as_object()
                            .and_then(|obj| obj.get("name").or_else(|| obj.get("id")))
                            .and_then(Value::as_str)
                            .is_some_and(|name| {
                                matches!(
                                    name.trim().to_ascii_lowercase().as_str(),
                                    "month" | "year"
                                )
                            })
                    })
                })
            {
                "data_table".to_string()
            } else {
                "chart".to_string()
            }
        }
        "numerator_denominator" | "ratio" => "summary".to_string(),
        _ => "data_table".to_string(),
    }
}

fn lookup_metric_contract(
    metric_id: &str,
    resources: &[crate::model::LoadedResource],
    world_hint: Option<&Value>,
    diagnostics: &mut Vec<Diagnostic>,
    target_file: &str,
) -> Option<(String, Option<Map<String, Value>>)> {
    let preferred_resource_ids = world_metrics_resource_candidates(world_hint, resources);
    for resource in resources {
        if !preferred_resource_ids.is_empty()
            && !preferred_resource_ids.iter().any(|id| id == &resource.id)
        {
            continue;
        }
        let Some(dataset) = resource.dataset.as_ref() else {
            continue;
        };
        let Some(key) =
            resolve_runtime_metric_def_key(&resource.id, metric_id, &dataset.runtime_metric_defs)
        else {
            continue;
        };
        let contract = dataset
            .runtime_analysis_contracts
            .get(&key)
            .and_then(Value::as_object)
            .cloned();
        return Some((resource.id.clone(), contract));
    }
    for resource in resources {
        let Some(dataset) = resource.dataset.as_ref() else {
            continue;
        };
        let Some(key) =
            resolve_runtime_metric_def_key(&resource.id, metric_id, &dataset.runtime_metric_defs)
        else {
            continue;
        };
        let contract = dataset
            .runtime_analysis_contracts
            .get(&key)
            .and_then(Value::as_object)
            .cloned();
        return Some((resource.id.clone(), contract));
    }
    diagnostics.push(Diagnostic {
        severity: Severity::Error,
        code: "projection_metric_not_found".to_string(),
        message: format!("projection assembly cannot resolve metric `{metric_id}`"),
        source_path: Some(target_file.to_string()),
    });
    None
}

fn world_metrics_resource_candidates(
    world_hint: Option<&Value>,
    resources: &[crate::model::LoadedResource],
) -> Vec<String> {
    let mut out = Vec::new();
    let mut push_path = |path: &str| {
        let path = path.trim();
        if path.is_empty() {
            return;
        }
        let id = imported_world_metrics_resource_id(path);
        if resources.iter().any(|resource| resource.id == id) && !out.iter().any(|item| item == &id)
        {
            out.push(id);
        }
    };
    if let Some(hint) = world_hint {
        if let Some(path) = hint
            .as_object()
            .and_then(|obj| obj.get("scene_file").or_else(|| obj.get("scene_path")))
            .and_then(Value::as_str)
        {
            push_path(path);
        }
    }
    out
}

fn build_analytics_filter_schema(
    slots: &[Map<String, Value>],
    rowset_dataset_id: Option<&str>,
    contract: Option<&Map<String, Value>>,
    board_filters: Option<&Value>,
) -> Value {
    if let Some(explicit) = board_filters_explicit_fields(board_filters) {
        let mut payload = serde_json::Map::new();
        payload.insert("fields".to_string(), Value::Array(explicit));
        if let Some(dataset_id) = rowset_dataset_id
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            payload.insert(
                "rowset_dataset_id".to_string(),
                Value::String(dataset_id.to_string()),
            );
        }
        return Value::Object(payload);
    }

    let mut fields = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for slot in slots {
        let by = slot
            .get("by")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let Some(column) = by else {
            continue;
        };
        let (key, label) = analytics_filter_key_for_column(
            column,
            slot.get("label").and_then(Value::as_str),
        );
        if seen.insert(key.clone()) {
            fields.push(serde_json::json!({
                "key": key,
                "label": label,
                "column": column,
            }));
        }
    }
    if let Some(contract) = contract {
        if let Some(blocks) = contract.get("blocks").and_then(Value::as_array) {
            for block in blocks {
                let Some(block_map) = block.as_object() else {
                    continue;
                };
                let support_role = block_map
                    .get("support_role")
                    .or_else(|| block_map.get("kind"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                if support_role != "detail" {
                    continue;
                }
                if let Some(columns) = block_map.get("fields").and_then(Value::as_array) {
                    for field in columns {
                        let Some(column) = field.as_str().map(str::trim).filter(|s| !s.is_empty())
                        else {
                            continue;
                        };
                        let (key, label) =
                            analytics_filter_key_for_column(column, Some(column));
                        if seen.insert(key.clone()) {
                            fields.push(serde_json::json!({
                                "key": key,
                                "label": label,
                                "column": column,
                            }));
                        }
                    }
                }
            }
        }
    }
    let mut payload = serde_json::Map::new();
    payload.insert("fields".to_string(), Value::Array(fields));
    if let Some(dataset_id) = rowset_dataset_id
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        payload.insert(
            "rowset_dataset_id".to_string(),
            Value::String(dataset_id.to_string()),
        );
    } else if let Some(detail_slot) = slots.iter().find(|slot| {
        slot.get("layout_zone").and_then(Value::as_str) == Some("detail")
    }) {
        if let Some(dataset_id) = detail_slot
            .get("dataset_id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            payload.insert(
                "rowset_dataset_id".to_string(),
                Value::String(dataset_id.to_string()),
            );
        }
    }
    Value::Object(payload)
}

pub(super) fn build_generic_rowset_filter_schema(
    slots: &[Map<String, Value>],
    rowset_dataset_id: &str,
) -> Value {
    build_analytics_filter_schema(slots, Some(rowset_dataset_id), None, None)
}

fn board_filters_explicit_fields(board_filters: Option<&Value>) -> Option<Vec<Value>> {
    let map = board_filters?.as_object()?;
    let items = map.get("fields")?.as_array()?;
    if items.is_empty() {
        return None;
    }
    let mut fields = Vec::new();
    for item in items {
        let Some(field_map) = item.as_object() else {
            continue;
        };
        let key = field_map
            .get("key")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())?;
        let column = field_map
            .get("column")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(key);
        let label = field_map
            .get("label")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(column);
        let control = field_map
            .get("control")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("text");
        fields.push(serde_json::json!({
            "key": key,
            "label": label,
            "column": column,
            "control": control,
        }));
    }
    if fields.is_empty() {
        None
    } else {
        Some(fields)
    }
}

fn analytics_filter_key_for_column(column: &str, label: Option<&str>) -> (String, String) {
    let label = label
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(column)
        .to_string();
    let key = column
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>();
    (key, label)
}

fn parse_metric_ref_id(value: &Value) -> Option<&str> {
    let map = value.as_object()?;
    if map.get("__ref").and_then(Value::as_str) != Some("metric") {
        return None;
    }
    map.get("id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
}
