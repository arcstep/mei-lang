use std::collections::BTreeMap;

use serde_json::{json, Map, Value};

use crate::model::{Diagnostic, LayoutDecl, PanelDecl, SceneContract, Severity, UiNodeDecl};
use crate::typed_refs::{decode_ref_value, RefKind};

use super::metric::{
    build_generic_rowset_filter_schema, expand_analytics_drilldown_tabs, expand_board_assembly,
    expand_drilldown_tabs, lower_projection_slot,
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
        walk_value_mut(
            &mut panel.props,
            resources,
            target_file,
            import_scope,
            diagnostics,
        );
        walk_ui_nodes_mut(
            &mut panel.blocks,
            resources,
            target_file,
            import_scope,
            diagnostics,
        );
    }
}

pub(crate) fn lower_scene_first_board_links_in_panels(
    panels: &mut [PanelDecl],
    resources: &[crate::model::LoadedResource],
    target_file: &str,
    target_scene_contracts: &BTreeMap<String, SceneContract>,
    target_scene_ids_by_file: &BTreeMap<String, String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for panel in panels.iter_mut() {
        let import_scope = panel
            .import_scope
            .as_deref()
            .map(str::trim)
            .filter(|scope| !scope.is_empty());
        walk_scene_first_value_mut(
            &mut panel.props,
            resources,
            target_file,
            import_scope,
            target_scene_contracts,
            target_scene_ids_by_file,
            diagnostics,
        );
        walk_scene_first_ui_nodes_mut(
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

fn walk_scene_first_ui_nodes_mut(
    nodes: &mut [UiNodeDecl],
    resources: &[crate::model::LoadedResource],
    target_file: &str,
    import_scope: Option<&str>,
    target_scene_contracts: &BTreeMap<String, SceneContract>,
    target_scene_ids_by_file: &BTreeMap<String, String>,
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
                walk_scene_first_value_mut(
                    &mut panel.props,
                    resources,
                    target_file,
                    scope,
                    target_scene_contracts,
                    target_scene_ids_by_file,
                    diagnostics,
                );
                walk_scene_first_ui_nodes_mut(
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
                walk_scene_first_value_mut(
                    &mut block.props,
                    resources,
                    target_file,
                    import_scope,
                    target_scene_contracts,
                    target_scene_ids_by_file,
                    diagnostics,
                );
                if let Some(component) = block.component.as_mut() {
                    walk_scene_first_value_mut(
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
                    walk_scene_first_value_mut(
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

fn walk_scene_first_value_mut(
    value: &mut Value,
    resources: &[crate::model::LoadedResource],
    target_file: &str,
    import_scope: Option<&str>,
    target_scene_contracts: &BTreeMap<String, SceneContract>,
    target_scene_ids_by_file: &BTreeMap<String, String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match value {
        Value::Object(map) => {
            if map.get("__kind").and_then(Value::as_str) == Some("board_link")
                || map.get("mode").and_then(Value::as_str) == Some("board_link")
            {
                lower_scene_first_board_link(
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
                walk_scene_first_value_mut(
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
                walk_scene_first_value_mut(
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
    let mut board_layout_mode: Option<String> = None;
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
        analytics_layout = expanded
            .2
            .as_deref()
            .is_some_and(|mode| mode == "analytics");
        board_layout_mode = expanded.2;
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

    apply_lowered_slots(
        link,
        slots,
        board_layout_mode,
        analytics_layout,
        analytics_filter_schema,
        default_slot,
        tabs_default_slot,
        title,
        target_file,
        diagnostics,
    );
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
            let rowset_dataset_id = map
                .get("rowset_dataset_id")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty());
            let slots = expand_drilldown_tabs(
                metric,
                include_hero,
                default_slot,
                resources,
                world_hint,
                diagnostics,
                target_file,
            )?;
            if let Some(dataset_id) = rowset_dataset_id {
                *analytics_filter_schema = Some(build_generic_rowset_filter_schema(
                    slots.as_slice(),
                    dataset_id,
                ));
            }
            return Some(slots);
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

fn apply_lowered_slots(
    link: &mut Map<String, Value>,
    slots: Vec<Map<String, Value>>,
    board_layout_mode: Option<String>,
    analytics_layout: bool,
    analytics_filter_schema: Option<Value>,
    default_slot: Option<usize>,
    tabs_default_slot: Option<usize>,
    title: Option<String>,
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
        link.insert("layout_mode".to_string(), Value::String("analytics".to_string()));
    }
    if let Some(schema) = analytics_filter_schema {
        link.insert("filter_schema".to_string(), schema);
    }
    if let Some(title) = title {
        link.insert("title".to_string(), Value::String(title));
    }
    link.remove("world");
    link.remove("board");
    link.remove("tabs");
}

fn lower_scene_first_board_link(
    link: &mut Map<String, Value>,
    resources: &[crate::model::LoadedResource],
    target_file: &str,
    import_scope: Option<&str>,
    target_scene_contracts: &BTreeMap<String, SceneContract>,
    target_scene_ids_by_file: &BTreeMap<String, String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if link.get("board").is_some()
        || link.get("tabs").is_some()
        || link.get("projection_slots").is_some()
    {
        return;
    }
    let Some(scene_ref) = link.get("scene").and_then(Value::as_object) else {
        return;
    };
    let Some(target_scene_id) =
        resolve_target_scene_id(scene_ref, target_scene_ids_by_file)
    else {
        return;
    };
    let Some(target_scene_contract) = target_scene_contracts.get(&target_scene_id) else {
        return;
    };
    let Some(shell_contract) = scene_shell_contract_from_scene_contract(target_scene_contract) else {
        return;
    };
    let layout_mode = shell_contract
        .get("layout_mode")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("");
    if layout_mode.is_empty() || layout_mode == "generic_tabs" {
        return;
    }
    let Some(board_payload) =
        synthesize_scene_first_board_payload(link, target_scene_contract, &shell_contract)
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
        target_file,
        diagnostics,
    );
}

fn resolve_target_scene_id(
    scene_ref: &Map<String, Value>,
    target_scene_ids_by_file: &BTreeMap<String, String>,
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
        .and_then(|path| target_scene_ids_by_file.get(path).cloned())
}

fn synthesize_scene_first_board_payload(
    link: &Map<String, Value>,
    target_scene_contract: &SceneContract,
    shell_contract: &Map<String, Value>,
) -> Option<Map<String, Value>> {
    let params = link
        .get("params")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let context = params
        .get("metric")
        .cloned()
        .filter(|value| matches!(decode_ref_value(value), Some(expr) if expr.kind == RefKind::Metric))?;
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

    let chart_zone_id = first_zone_id_for_role(&zones, "slots", Some("chart"));
    let detail_zone_id = first_zone_id_for_role(&zones, "slots", Some("data_table"));
    let preview_zone_id = first_zone_id_for_role(&zones, "row_preview", Some("summary"))
        .or_else(|| first_zone_id_for_role(&zones, "slots", Some("summary")));
    let filter_zone_id = first_zone_id_for_role(&zones, "filter", None);

    if let Some(filters) = binding_value_for_keys(
        bindings_map,
        &[
            filter_zone_id.as_deref().unwrap_or(""),
            "filter_schema",
            "filters",
        ],
    )
    .or_else(|| {
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
        payload.insert("filters".to_string(), filters);
    }

    if let Some(charts) = binding_value_for_keys(
        bindings_map,
        &[chart_zone_id.as_deref().unwrap_or(""), "chart", "charts"],
    ) {
        match charts {
            Value::Array(_) => {
                payload.insert("charts".to_string(), charts);
            }
            other => {
                payload.insert("charts".to_string(), Value::Array(vec![other]));
            }
        }
    }
    if let Some(detail) = binding_value_for_keys(
        bindings_map,
        &[detail_zone_id.as_deref().unwrap_or(""), "detail", "list"],
    ) {
        payload.insert("detail".to_string(), detail);
    }
    if let Some(preview) = binding_value_for_keys(
        bindings_map,
        &[preview_zone_id.as_deref().unwrap_or(""), "preview", "summary"],
    ) {
        payload.insert("preview".to_string(), preview);
    }
    Some(payload)
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
            if map.get("__ref").and_then(Value::as_str) == Some("scene_param") {
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

fn binding_value_for_keys(bindings: &Map<String, Value>, keys: &[&str]) -> Option<Value> {
    keys.iter().find_map(|key| {
        let normalized = key.trim();
        if normalized.is_empty() {
            return None;
        }
        bindings.get(normalized).cloned().filter(|value| !value.is_null())
    })
}

fn first_zone_id_for_role(
    zones: &[Value],
    role: &str,
    accepted_component: Option<&str>,
) -> Option<String> {
    zones.iter().find_map(|zone| {
        let zone_map = zone.as_object()?;
        let zone_role = zone_map
            .get("role")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())?;
        if zone_role != role {
            return None;
        }
        if let Some(component) = accepted_component {
            let accepts = zone_map
                .get("accepts")
                .and_then(Value::as_array)
                .map(|items| {
                    items.iter().any(|item| {
                        item.as_str()
                            .map(str::trim)
                            .is_some_and(|value| value == component)
                    })
                })
                .unwrap_or(false);
            if !accepts && role != "filter" && role != "row_preview" {
                return None;
            }
        }
        zone_map
            .get("id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}

fn scene_shell_contract_from_scene_contract(
    contract: &SceneContract,
) -> Option<Map<String, Value>> {
    let mut zones = Vec::new();
    collect_scene_shell_zones(&contract.panels, "", &mut zones);
    let layout = contract
        .frame
        .as_ref()
        .and_then(|frame| frame.layout.as_ref())
        .map(layout_decl_to_value);
    if zones.is_empty() && layout.is_none() {
        return None;
    }
    let layout_mode = infer_scene_shell_layout_mode(&zones);
    let mut payload = Map::new();
    payload.insert(
        "__kind".to_string(),
        Value::String("scene_shell_contract".to_string()),
    );
    if !layout_mode.is_empty() {
        payload.insert("layout_mode".to_string(), Value::String(layout_mode));
    }
    if let Some(layout) = layout {
        payload.insert("layout".to_string(), layout);
    }
    payload.insert("zones".to_string(), Value::Array(zones));
    if let Some(overlay_size) = contract
        .scene
        .local_nav
        .get("overlay_size")
        .or_else(|| contract.scene.local_nav.get("overlaySize"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        payload.insert(
            "overlay_size".to_string(),
            Value::String(overlay_size.to_string()),
        );
    }
    Some(payload)
}

fn infer_scene_shell_layout_mode(zones: &[Value]) -> String {
    let mut has_tab_bar = false;
    let mut has_tab_content = false;
    let mut has_row_preview = false;
    let mut has_filter = false;
    let mut has_slots = false;
    for zone in zones {
        let Some(role) = zone
            .as_object()
            .and_then(|map| map.get("role"))
            .and_then(Value::as_str)
            .map(str::trim)
        else {
            continue;
        };
        match role {
            "tab_bar" => has_tab_bar = true,
            "tab_content" => has_tab_content = true,
            "row_preview" => has_row_preview = true,
            "filter" => has_filter = true,
            "slots" => has_slots = true,
            _ => {}
        }
    }
    if has_tab_bar && has_tab_content {
        return "generic_tabs".to_string();
    }
    if has_row_preview {
        return "list_preview".to_string();
    }
    if has_filter && has_slots {
        return "analytics".to_string();
    }
    String::new()
}

fn collect_scene_shell_zones(
    panels: &[PanelDecl],
    parent: &str,
    out: &mut Vec<Value>,
) {
    for panel in panels {
        if let Some(zone) = panel_zone_to_value(panel, parent) {
            out.push(Value::Object(zone));
        }
        let child_panels = panel
            .blocks
            .iter()
            .filter_map(|node| match node {
                UiNodeDecl::Panel(child) => Some(child.clone()),
                _ => None,
            })
            .collect::<Vec<_>>();
        let next_parent = panel.id.as_str();
        collect_scene_shell_zones(&child_panels, next_parent, out);
    }
}

fn panel_zone_to_value(panel: &PanelDecl, parent: &str) -> Option<Map<String, Value>> {
    let props = panel.props.as_object()?;
    let role = props
        .get("projection_role")
        .or_else(|| props.get("zone_role"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    let mut zone = Map::new();
    zone.insert("id".to_string(), Value::String(panel.id.clone()));
    zone.insert("role".to_string(), Value::String(role.to_string()));
    if let Some(area) = panel.area.as_deref().map(str::trim).filter(|value| !value.is_empty()) {
        zone.insert("area".to_string(), Value::String(area.to_string()));
    }
    if !parent.trim().is_empty() {
        zone.insert("parent".to_string(), Value::String(parent.trim().to_string()));
    }
    if let Some(source) = props
        .get("projection_source")
        .or_else(|| props.get("source"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        zone.insert("source".to_string(), Value::String(source.to_string()));
    }
    if let Some(selection_source) = props
        .get("selection_source")
        .or_else(|| props.get("selectionSource"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        zone.insert(
            "selection_source".to_string(),
            Value::String(selection_source.to_string()),
        );
    }
    if let Some(required) = props.get("projection_required") {
        zone.insert("required".to_string(), required.clone());
    }
    if let Some(max) = props.get("projection_max") {
        zone.insert("max".to_string(), max.clone());
    }
    if let Some(accepts) = props.get("projection_accepts").and_then(Value::as_array) {
        zone.insert("accepts".to_string(), Value::Array(accepts.clone()));
    }
    if let Some(layout) = panel.layout.as_ref() {
        zone.insert("layout".to_string(), layout_decl_to_value(layout));
    }
    Some(zone)
}

fn layout_decl_to_value(layout: &LayoutDecl) -> Value {
    let mut out = Map::new();
    out.insert(
        "type".to_string(),
        Value::String(layout.layout_type.clone()),
    );
    if let Some(direction) = layout.direction.as_ref() {
        out.insert("direction".to_string(), Value::String(direction.clone()));
    }
    if let Some(columns) = layout.columns.as_ref() {
        out.insert(
            "columns".to_string(),
            Value::Array(columns.iter().cloned().map(Value::String).collect()),
        );
    }
    if let Some(rows) = layout.rows.as_ref() {
        out.insert(
            "rows".to_string(),
            Value::Array(rows.iter().cloned().map(Value::String).collect()),
        );
    }
    if let Some(areas) = layout.areas.as_ref() {
        out.insert(
            "areas".to_string(),
            Value::Array(
                areas
                    .iter()
                    .map(|row| Value::Array(row.iter().cloned().map(Value::String).collect()))
                    .collect(),
            ),
        );
    }
    if let Some(gap) = layout.gap.as_ref() {
        out.insert("gap".to_string(), Value::String(gap.clone()));
    }
    if let Some(padding) = layout.padding.as_ref() {
        out.insert("padding".to_string(), Value::String(padding.clone()));
    }
    if let Some(align) = layout.align.as_ref() {
        out.insert("align".to_string(), Value::String(align.clone()));
    }
    if let Some(justify) = layout.justify.as_ref() {
        out.insert("justify".to_string(), Value::String(justify.clone()));
    }
    Value::Object(out)
}
