use super::{resolve_target_scene_id, scene_shell_contract_from_scene_contract, synthesize_scene_first_board_payload, synthesize_scene_first_generic_tabs_slots, validate_and_resolve_scene_params};

use std::collections::BTreeMap;

use serde_json::{json, Map, Value};

use crate::model::{
    Diagnostic, PanelDecl, SceneContract, Severity, UiNodeDecl,
};

use super::super::metric::{
    build_generic_rowset_filter_schema, expand_board_assembly,
};

pub(crate) fn lower_scene_links_in_panels(
    panels: &mut [PanelDecl],
    resources: &[crate::model::LoadedResource],
    target_file: &str,
    target_scene_contracts: &BTreeMap<String, SceneContract>,
    target_scene_ids_by_file: &BTreeMap<String, Vec<String>>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for panel in panels.iter_mut() {
        let import_scope = panel
            .import_scope
            .as_deref()
            .map(str::trim)
            .filter(|scope| !scope.is_empty());
        walk_scene_value_mut(
            &mut panel.props,
            resources,
            target_file,
            import_scope,
            target_scene_contracts,
            target_scene_ids_by_file,
            diagnostics,
        );
        walk_scene_ui_nodes_mut(
            &mut panel.blocks,
            resources,
            target_file,
            import_scope,
            target_scene_contracts,
            target_scene_ids_by_file,
            diagnostics,
        );
    }
}

fn walk_scene_ui_nodes_mut(
    nodes: &mut [UiNodeDecl],
    resources: &[crate::model::LoadedResource],
    target_file: &str,
    import_scope: Option<&str>,
    target_scene_contracts: &BTreeMap<String, SceneContract>,
    target_scene_ids_by_file: &BTreeMap<String, Vec<String>>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for node in nodes.iter_mut() {
        match node {
            UiNodeDecl::Panel(panel) => {
                let scope = panel
                    .import_scope
                    .as_deref()
                    .map(str::trim)
                    .filter(|scope| !scope.is_empty())
                    .or(import_scope);
                walk_scene_value_mut(
                    &mut panel.props,
                    resources,
                    target_file,
                    scope,
                    target_scene_contracts,
                    target_scene_ids_by_file,
                    diagnostics,
                );
                walk_scene_ui_nodes_mut(
                    &mut panel.blocks,
                    resources,
                    target_file,
                    scope,
                    target_scene_contracts,
                    target_scene_ids_by_file,
                    diagnostics,
                );
            }
            UiNodeDecl::Block(block) => {
                walk_scene_value_mut(
                    &mut block.props,
                    resources,
                    target_file,
                    import_scope,
                    target_scene_contracts,
                    target_scene_ids_by_file,
                    diagnostics,
                );
                if let Some(component) = block.component.as_mut() {
                    walk_scene_value_mut(
                        component,
                        resources,
                        target_file,
                        import_scope,
                        target_scene_contracts,
                        target_scene_ids_by_file,
                        diagnostics,
                    );
                }
                for child in block.blocks.iter_mut() {
                    walk_scene_value_mut(
                        child,
                        resources,
                        target_file,
                        import_scope,
                        target_scene_contracts,
                        target_scene_ids_by_file,
                        diagnostics,
                    );
                }
            }
            UiNodeDecl::PanelRefEmbed(_) => {}
        }
    }
}

fn walk_scene_value_mut(
    value: &mut Value,
    resources: &[crate::model::LoadedResource],
    target_file: &str,
    import_scope: Option<&str>,
    target_scene_contracts: &BTreeMap<String, SceneContract>,
    target_scene_ids_by_file: &BTreeMap<String, Vec<String>>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match value {
        Value::Object(map) => {
            if map.get("__kind").and_then(Value::as_str) == Some("board_link")
                || map.get("mode").and_then(Value::as_str) == Some("board_link")
            {
                lower_scene_link(
                    map,
                    resources,
                    target_file,
                    import_scope,
                    target_scene_contracts,
                    target_scene_ids_by_file,
                    diagnostics,
                );
            }
            for child in map.values_mut() {
                walk_scene_value_mut(
                    child,
                    resources,
                    target_file,
                    import_scope,
                    target_scene_contracts,
                    target_scene_ids_by_file,
                    diagnostics,
                );
            }
        }
        Value::Array(items) => {
            for child in items.iter_mut() {
                walk_scene_value_mut(
                    child,
                    resources,
                    target_file,
                    import_scope,
                    target_scene_contracts,
                    target_scene_ids_by_file,
                    diagnostics,
                );
            }
        }
        _ => {}
    }
}

fn resolve_world_hint(
    authored_world: Option<&Value>,
    import_scope: Option<&str>,
    target_file: &str,
) -> Option<Value> {
    if let Some(world) = authored_world.filter(|value| !value.is_null()) {
        return Some(world.clone());
    }
    if let Some(scope) = import_scope
        .map(str::trim)
        .filter(|scope| !scope.is_empty())
    {
        return Some(json!({ "scene_file": scope }));
    }
    let target = target_file.trim();
    if target.ends_with(".mei") && !target.is_empty() {
        return Some(json!({ "scene_file": target }));
    }
    None
}

fn apply_lowered_slots(
    link: &mut Map<String, Value>,
    slots: Vec<Map<String, Value>>,
    board_layout_mode: Option<String>,
    analytics_layout: bool,
    analytics_filter_schema: Option<Value>,
    default_slot: Option<usize>,
    tabs_default_slot: Option<usize>,
    title: Option<String>,
    shell_contract: Option<Map<String, Value>>,
    local_nav: Option<Value>,
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if slots.is_empty() {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "empty_projection_slots".to_string(),
            message: "board assembly expanded to an empty projection_slots list".to_string(),
            source_path: Some(target_file.to_string()),
        });
        return;
    }

    let resolved_default_slot = default_slot.or(tabs_default_slot);
    let mut projection_slots = Vec::new();
    for (index, slot) in slots.into_iter().enumerate() {
        let mut slot_obj = slot;
        let is_default = resolved_default_slot
            .map(|preferred| preferred == index)
            .unwrap_or_else(|| {
                slot_obj
                    .get("default")
                    .and_then(Value::as_bool)
                    .unwrap_or(index == 0)
            });
        slot_obj.insert("default".to_string(), Value::Bool(is_default));
        projection_slots.push(Value::Object(slot_obj));
    }

    link.insert(
        "projection_slots".to_string(),
        Value::Array(projection_slots),
    );
    if let Some(layout_mode) = board_layout_mode {
        link.insert("layout_mode".to_string(), Value::String(layout_mode));
    } else if analytics_layout {
        link.insert(
            "layout_mode".to_string(),
            Value::String("analytics".to_string()),
        );
    }
    if let Some(schema) = analytics_filter_schema {
        link.insert("filter_schema".to_string(), schema);
    }
    if let Some(title) = title {
        link.insert("title".to_string(), Value::String(title));
    }
    if let Some(shell_contract) = shell_contract {
        link.insert("shell_contract".to_string(), Value::Object(shell_contract));
    }
    if let Some(local_nav) = local_nav.filter(|value| !value.is_null()) {
        let include = match local_nav.as_object() {
            Some(map) => !map.is_empty(),
            None => true,
        };
        if include {
            link.insert("local_nav".to_string(), local_nav);
        }
    }
    link.remove("world");
    link.remove("board");
    link.remove("tabs");
}

fn lower_scene_link(
    link: &mut Map<String, Value>,
    resources: &[crate::model::LoadedResource],
    target_file: &str,
    import_scope: Option<&str>,
    target_scene_contracts: &BTreeMap<String, SceneContract>,
    target_scene_ids_by_file: &BTreeMap<String, Vec<String>>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if link.get("board").is_some() {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "scene_link_board_removed".to_string(),
            message: "link(board=...) 已移除；请改用 link(scene=..., params=..., projection=...)"
                .to_string(),
            source_path: Some(target_file.to_string()),
        });
        return;
    }
    if link.get("tabs").is_some() {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "scene_link_tabs_removed".to_string(),
            message: "link(tabs=...) 已移除；请改用 scene.params + scene.bindings + link(scene=..., params=...)".to_string(),
            source_path: Some(target_file.to_string()),
        });
        return;
    }
    if link.get("projection_slots").is_some() {
        return;
    }
    let Some(scene_ref) = link.get("scene").and_then(Value::as_object) else {
        return;
    };
    let Some(target_scene_id) = resolve_target_scene_id(scene_ref, target_scene_ids_by_file) else {
        return;
    };
    let Some(target_scene_contract) = target_scene_contracts.get(&target_scene_id) else {
        return;
    };
    let Some(params) =
        validate_and_resolve_scene_params(link, target_scene_contract, target_file, diagnostics)
    else {
        return;
    };
    let Some(shell_contract) = scene_shell_contract_from_scene_contract(target_scene_contract)
    else {
        return;
    };
    let layout_mode = shell_contract
        .get("layout_mode")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("");
    if layout_mode.is_empty() {
        return;
    }
    if layout_mode == "generic_tabs" {
        let world_hint = resolve_world_hint(link.get("world"), import_scope, target_file);
        let Some(mut slots) = synthesize_scene_first_generic_tabs_slots(
            target_scene_contract,
            &params,
            resources,
            world_hint.as_ref(),
            diagnostics,
            target_file,
        ) else {
            return;
        };
        let tab_content_zone = shell_contract
            .get("zones")
            .and_then(Value::as_array)
            .and_then(|zones| {
                zones.iter().find_map(|zone| {
                    let zone_map = zone.as_object()?;
                    let role = zone_map
                        .get("role")
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .filter(|value| !value.is_empty())?;
                    if role != "tab_content" {
                        return None;
                    }
                    zone_map
                        .get("id")
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(str::to_string)
                })
            })
            .unwrap_or_else(|| "content".to_string());
        let preferred_entry = link
            .get("entry")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .or_else(|| {
                link.get("scene")
                    .and_then(Value::as_object)
                    .and_then(|scene| scene.get("entry"))
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
            })
            .or_else(|| {
                params
                    .get("entry")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
            });
        let has_preferred_entry = preferred_entry.is_some_and(|entry| {
            slots
                .iter()
                .any(|slot| slot.get("id").and_then(Value::as_str) == Some(entry))
        });
        for slot in slots.iter_mut() {
            slot.insert(
                "layout_zone".to_string(),
                Value::String(tab_content_zone.clone()),
            );
            if has_preferred_entry {
                let entry = preferred_entry.unwrap_or_default();
                let is_default = slot
                    .get("id")
                    .and_then(Value::as_str)
                    .is_some_and(|slot_id| slot_id == entry);
                slot.insert("default".to_string(), Value::Bool(is_default));
            }
        }
        let filter_schema = params
            .get("rowset_dataset_id")
            .or_else(|| params.get("rowsetDatasetId"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|rowset_dataset_id| {
                build_generic_rowset_filter_schema(slots.as_slice(), rowset_dataset_id)
            });
        let title = link
            .get("title")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        apply_lowered_slots(
            link,
            slots,
            Some("generic_tabs".to_string()),
            false,
            filter_schema,
            None,
            None,
            title,
            Some(shell_contract.clone()),
            Some(target_scene_contract.scene.local_nav.clone()),
            target_file,
            diagnostics,
        );
        return;
    }
    let Some(board_payload) =
        synthesize_scene_first_board_payload(link, target_scene_contract, &shell_contract, &params)
    else {
        return;
    };
    let world_hint = resolve_world_hint(link.get("world"), import_scope, target_file);
    let Some(expanded) = expand_board_assembly(
        &board_payload,
        resources,
        world_hint.as_ref(),
        diagnostics,
        target_file,
    ) else {
        return;
    };
    let default_slot = link
        .get("default_slot")
        .and_then(Value::as_u64)
        .map(|v| v as usize);
    let title = link
        .get("title")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let analytics_layout = expanded
        .2
        .as_deref()
        .is_some_and(|mode| mode == "analytics");
    apply_lowered_slots(
        link,
        expanded.0,
        expanded.2,
        analytics_layout,
        expanded.1,
        default_slot,
        None,
        title,
        Some(shell_contract.clone()),
        Some(target_scene_contract.scene.local_nav.clone()),
        target_file,
        diagnostics,
    );
}

