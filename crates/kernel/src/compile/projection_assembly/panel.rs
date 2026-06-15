use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Map, Value};

use crate::model::{Diagnostic, LayoutDecl, PanelDecl, PanelSlotDecl, SceneContract, Severity, UiNodeDecl};
use crate::typed_refs::{decode_ref_value, RefKind};

use super::metric::{
    build_generic_rowset_filter_schema, expand_board_assembly, expand_drilldown_tabs,
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
        link.insert("layout_mode".to_string(), Value::String("analytics".to_string()));
    }
    if let Some(schema) = analytics_filter_schema {
        link.insert("filter_schema".to_string(), schema);
    }
    if let Some(title) = title {
        link.insert("title".to_string(), Value::String(title));
    }
    if let Some(shell_contract) = shell_contract {
        link.insert(
            "shell_contract".to_string(),
            Value::Object(shell_contract),
        );
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
            message: "link(board=...) 已移除；请改用 link(scene=..., params=..., projection=...)".to_string(),
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
    let Some(target_scene_id) =
        resolve_target_scene_id(scene_ref, target_scene_ids_by_file)
    else {
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
    let Some(shell_contract) = scene_shell_contract_from_scene_contract(target_scene_contract) else {
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
        let has_preferred_entry = preferred_entry
            .is_some_and(|entry| {
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

fn validate_and_resolve_scene_params(
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

fn resolve_target_scene_id(
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

fn synthesize_scene_first_board_payload(
    link: &Map<String, Value>,
    target_scene_contract: &SceneContract,
    shell_contract: &Map<String, Value>,
    params: &Map<String, Value>,
) -> Option<Map<String, Value>> {
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
        let has_summary = accepts.iter().any(|value| value.as_str() == Some("summary"));
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

fn synthesize_scene_first_generic_tabs_slots(
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

pub(crate) fn scene_shell_contract_from_scene_contract(
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
    if let Some(ref layout) = layout {
        retain_shell_zones_matching_layout(layout, &mut zones);
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

fn retain_shell_zones_matching_layout(layout: &Value, zones: &mut Vec<Value>) {
    let Some(areas) = layout.get("areas").and_then(Value::as_array) else {
        return;
    };
    let mut allowed = BTreeSet::new();
    for row in areas {
        let Some(cells) = row.as_array() else {
            continue;
        };
        for cell in cells {
            let Some(area) = cell.as_str().map(str::trim).filter(|value| !value.is_empty()) else {
                continue;
            };
            if area != "." {
                allowed.insert(area.to_string());
            }
        }
    }
    if allowed.is_empty() {
        return;
    }
    let mut kept_ids = BTreeSet::new();
    for zone in zones.iter() {
        let Some(map) = zone.as_object() else {
            continue;
        };
        let Some(area) = map
            .get("area")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        if allowed.contains(area) {
            if let Some(id) = map.get("id").and_then(Value::as_str).filter(|id| !id.is_empty()) {
                kept_ids.insert(id.to_string());
            }
        }
    }
    let mut changed = true;
    while changed {
        changed = false;
        for zone in zones.iter() {
            let Some(map) = zone.as_object() else {
                continue;
            };
            let Some(id) = map.get("id").and_then(Value::as_str).filter(|id| !id.is_empty()) else {
                continue;
            };
            if kept_ids.contains(id) {
                continue;
            }
            let Some(parent) = map
                .get("parent")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
            else {
                continue;
            };
            if kept_ids.contains(parent) && kept_ids.insert(id.to_string()) {
                changed = true;
            }
        }
    }
    zones.retain(|zone| {
        zone.as_object()
            .and_then(|map| map.get("id"))
            .and_then(Value::as_str)
            .is_some_and(|id| kept_ids.contains(id))
    });
}

fn infer_scene_shell_layout_mode(zones: &[Value]) -> String {
    let mut has_tab_bar = false;
    let mut has_tab_content = false;
    let mut has_row_preview = false;
    let mut has_filter = false;
    let mut has_slots = false;
    let mut has_analytics_content = false;
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
        if zone_implies_analytics_content(zone) {
            has_analytics_content = true;
        }
    }
    if has_tab_bar && has_tab_content {
        return "generic_tabs".to_string();
    }
    if has_row_preview {
        return "list_preview".to_string();
    }
    if has_filter && (has_slots || has_analytics_content) {
        return "analytics".to_string();
    }
    String::new()
}

fn zone_implies_analytics_content(zone: &Value) -> bool {
    let Some(map) = zone.as_object() else {
        return false;
    };
    if map.get("role").and_then(Value::as_str) == Some("slots") {
        return true;
    }
    if map
        .get("accepts")
        .and_then(Value::as_array)
        .is_some_and(|items| {
            items.iter().any(|value| {
                matches!(value.as_str(), Some("chart") | Some("data_table"))
            })
        })
    {
        return true;
    }
    map.get("layout")
        .and_then(|layout| layout.get("areas"))
        .and_then(Value::as_array)
        .is_some_and(|rows| {
            rows.iter().any(|row| {
                row.as_array().is_some_and(|cells| {
                    cells.iter().any(|cell| {
                        matches!(cell.as_str(), Some("chart") | Some("detail"))
                    })
                })
            })
        })
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
    let slot_map = panel_slot_as_map(panel)?;
    let role = slot_map
        .get("kind")
        .or_else(|| slot_map.get("role"))
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
    if let Some(source) = slot_map
        .get("source")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        zone.insert("source".to_string(), Value::String(source.to_string()));
    }
    if let Some(selection_source) = slot_map
        .get("selection_from")
        .or_else(|| slot_map.get("selectionFrom"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        zone.insert(
            "selection_source".to_string(),
            Value::String(selection_source.to_string()),
        );
    }
    if let Some(required) = slot_map.get("required") {
        zone.insert("required".to_string(), required.clone());
    }
    if let Some(max) = slot_map.get("max") {
        zone.insert("max".to_string(), max.clone());
    }
    if let Some(accepts) = slot_map.get("accepts").and_then(Value::as_array) {
        zone.insert("accepts".to_string(), Value::Array(accepts.clone()));
    }
    if let Some(layout) = panel.layout.as_ref() {
        zone.insert("layout".to_string(), layout_decl_to_value(layout));
    }
    Some(zone)
}

fn panel_slot_as_map(panel: &PanelDecl) -> Option<Map<String, Value>> {
    if let Some(slot) = panel.slot.as_ref().filter(|slot| panel_slot_decl_is_meaningful(slot)) {
        return Some(panel_slot_decl_to_map(slot));
    }
    let props = panel.props.as_object()?;
    if let Some(slot) = props.get("__mei_panel_slot").and_then(Value::as_object) {
        return Some(slot.clone());
    }
    let role = props
        .get("projection_role")
        .or_else(|| props.get("zone_role"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    let mut migrated = Map::new();
    migrated.insert("kind".to_string(), Value::String(role.to_string()));
    if let Some(source) = props
        .get("projection_source")
        .or_else(|| props.get("source"))
        .filter(|value| !value.is_null())
    {
        migrated.insert("source".to_string(), source.clone());
    }
    if let Some(selection) = props
        .get("selection_source")
        .or_else(|| props.get("selectionSource"))
        .filter(|value| !value.is_null())
    {
        migrated.insert("selection_from".to_string(), selection.clone());
    }
    if let Some(required) = props.get("projection_required") {
        migrated.insert("required".to_string(), required.clone());
    }
    if let Some(max) = props.get("projection_max") {
        migrated.insert("max".to_string(), max.clone());
    }
    if let Some(accepts) = props.get("projection_accepts").and_then(Value::as_array) {
        migrated.insert("accepts".to_string(), Value::Array(accepts.clone()));
    }
    Some(migrated)
}

fn panel_slot_decl_is_meaningful(slot: &PanelSlotDecl) -> bool {
    slot.kind
        .as_deref()
        .map(str::trim)
        .is_some_and(|value| !value.is_empty())
}

fn panel_slot_decl_to_map(slot: &PanelSlotDecl) -> Map<String, Value> {
    let value = serde_json::to_value(slot).unwrap_or(Value::Null);
    value.as_object().cloned().unwrap_or_default()
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

/// Manage/build 预览：用 scene `examples[0].params` 展开 projection_slots，供无 caller 时装配 filter/chart/detail。
pub(crate) fn enrich_scene_projection_assembly_preview(
    assembly: &mut Map<String, Value>,
    contract: &SceneContract,
    resources: &[crate::model::LoadedResource],
    target_file: &str,
) {
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
        return;
    };
    let link = Map::new();
    let Some(board_payload) =
        synthesize_scene_first_board_payload(&link, contract, &shell_contract, &params)
    else {
        return;
    };
    let world_hint = contract.scene.world.clone();
    let mut diagnostics = Vec::new();
    let Some(expanded) = expand_board_assembly(
        &board_payload,
        resources,
        world_hint.as_ref(),
        &mut diagnostics,
        target_file,
    ) else {
        return;
    };
    let (slots, filter_schema, _) = expanded;
    if slots.is_empty() {
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
    params.get("metric").is_some_and(|metric| {
        matches!(decode_ref_value(metric), Some(expr) if expr.kind == RefKind::Metric)
    })
    .then_some(params)
}
