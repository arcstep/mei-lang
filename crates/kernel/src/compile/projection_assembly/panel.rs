use serde_json::{json, Map, Value};

use crate::model::{Diagnostic, PanelDecl, Severity, UiNodeDecl};

use super::metric::{
    expand_analytics_drilldown_tabs, expand_board_assembly, expand_drilldown_tabs, lower_projection_slot,
};

pub(crate) fn lower_projection_assembly_in_panels(
    panels: &mut [PanelDecl],
    resources: &[crate::model::LoadedResource],
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for panel in panels.iter_mut() {
        let import_scope = panel
            .import_scope
            .as_deref()
            .map(str::trim)
            .filter(|scope| !scope.is_empty());
        walk_ui_nodes_mut(
            &mut panel.blocks,
            resources,
            target_file,
            import_scope,
            diagnostics,
        );
    }
}

fn walk_ui_nodes_mut(
    nodes: &mut [UiNodeDecl],
    resources: &[crate::model::LoadedResource],
    target_file: &str,
    import_scope: Option<&str>,
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
                walk_ui_nodes_mut(
                    &mut panel.blocks,
                    resources,
                    target_file,
                    scope,
                    diagnostics,
                );
            }
            UiNodeDecl::Block(block) => {
                walk_value_mut(
                    &mut block.props,
                    resources,
                    target_file,
                    import_scope,
                    diagnostics,
                );
                if let Some(component) = block.component.as_mut() {
                    walk_value_mut(component, resources, target_file, import_scope, diagnostics);
                }
                for child in block.blocks.iter_mut() {
                    walk_value_mut(child, resources, target_file, import_scope, diagnostics);
                }
            }
            UiNodeDecl::PanelRefEmbed(_) => {}
        }
    }
}

fn walk_value_mut(
    value: &mut Value,
    resources: &[crate::model::LoadedResource],
    target_file: &str,
    import_scope: Option<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match value {
        Value::Object(map) => {
            if map.get("__kind").and_then(Value::as_str) == Some("board_link")
                || map.get("mode").and_then(Value::as_str) == Some("board_link")
            {
                lower_board_link(map, resources, target_file, import_scope, diagnostics);
            }
            for child in map.values_mut() {
                walk_value_mut(child, resources, target_file, import_scope, diagnostics);
            }
        }
        Value::Array(items) => {
            for child in items.iter_mut() {
                walk_value_mut(child, resources, target_file, import_scope, diagnostics);
            }
        }
        _ => {}
    }
}

fn lower_board_link(
    link: &mut Map<String, Value>,
    resources: &[crate::model::LoadedResource],
    target_file: &str,
    import_scope: Option<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let board_value = link.get("board").cloned();
    let tabs_value = link.get("tabs").cloned();
    if board_value.is_none() && tabs_value.is_none() {
        return;
    }

    let world_hint = resolve_world_hint(link.get("world"), import_scope, target_file);
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

    let analytics_layout;
    let mut analytics_filter_schema = None;
    let tabs_default_slot = tabs_value
        .as_ref()
        .and_then(Value::as_object)
        .and_then(|map| map.get("default_slot"))
        .and_then(Value::as_u64)
        .map(|v| v as usize);

    let slots = if let Some(board) = board_value.as_ref() {
        let Some(board_map) = board.as_object() else {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "board_assembly_invalid".to_string(),
                message: "link board must be build_board_assembly(...)".to_string(),
                source_path: Some(target_file.to_string()),
            });
            return;
        };
        if board_map.get("__kind").and_then(Value::as_str) != Some("board_assembly") {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "board_assembly_invalid".to_string(),
                message: "link board must use __kind=board_assembly".to_string(),
                source_path: Some(target_file.to_string()),
            });
            return;
        }
        if let Some(scene) = board_map.get("scene") {
            link.insert("scene".to_string(), scene.clone());
        }
        let Some(expanded) = expand_board_assembly(
            board_map,
            resources,
            world_hint.as_ref(),
            diagnostics,
            target_file,
        ) else {
            return;
        };
        analytics_layout = expanded.2;
        analytics_filter_schema = expanded.1;
        expanded.0
    } else {
        let Some(tabs_value) = tabs_value else {
            return;
        };
        analytics_layout = tabs_value
            .as_object()
            .and_then(|map| map.get("__kind"))
            .and_then(Value::as_str)
            == Some("analytics_projection_slot_list");
        match expand_tabs_value(
            &tabs_value,
            resources,
            world_hint.as_ref(),
            diagnostics,
            target_file,
            &mut analytics_filter_schema,
        ) {
            Some(slots) if !slots.is_empty() => slots,
            Some(_) => {
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    code: "empty_projection_slots".to_string(),
                    message: "link tabs expanded to an empty projection_slots list".to_string(),
                    source_path: Some(target_file.to_string()),
                });
                return;
            }
            None => return,
        }
    };

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
    if analytics_layout {
        link.insert("layout_mode".to_string(), Value::String("analytics".to_string()));
        if let Some(schema) = analytics_filter_schema {
            link.insert("filter_schema".to_string(), schema);
        }
    }
    if let Some(title) = title {
        link.insert("title".to_string(), Value::String(title));
    }
    link.remove("world");
    link.remove("board");
    link.remove("tabs");
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

fn expand_tabs_value(
    tabs: &Value,
    resources: &[crate::model::LoadedResource],
    world_hint: Option<&Value>,
    diagnostics: &mut Vec<Diagnostic>,
    target_file: &str,
    analytics_filter_schema: &mut Option<Value>,
) -> Option<Vec<Map<String, Value>>> {
    if let Some(map) = tabs.as_object() {
        if map.get("__kind").and_then(Value::as_str) == Some("analytics_projection_slot_list") {
            let expanded = expand_analytics_drilldown_tabs(
                map,
                resources,
                world_hint,
                diagnostics,
                target_file,
            )?;
            *analytics_filter_schema = Some(expanded.1);
            return Some(expanded.0);
        }
        if map.get("__kind").and_then(Value::as_str) == Some("projection_slot_list") {
            let metric = map.get("source")?;
            let include_hero = map
                .get("include_hero")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let default_slot = map
                .get("default_slot")
                .and_then(Value::as_u64)
                .map(|v| v as usize);
            return expand_drilldown_tabs(
                metric,
                include_hero,
                default_slot,
                resources,
                world_hint,
                diagnostics,
                target_file,
            );
        }
        if map.get("__kind").and_then(Value::as_str) == Some("projection_slot") {
            return lower_projection_slot(map, resources, world_hint, diagnostics, target_file)
                .map(|slot| vec![slot]);
        }
    }
    if let Some(items) = tabs.as_array() {
        let mut out = Vec::new();
        for item in items {
            if let Some(mut slots) = expand_tabs_value(
                item,
                resources,
                world_hint,
                diagnostics,
                target_file,
                analytics_filter_schema,
            ) {
                out.append(&mut slots);
            }
        }
        return Some(out);
    }
    None
}
