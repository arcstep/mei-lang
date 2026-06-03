use serde_json::{json, Map, Value};

use crate::compile::materialize::{
    imported_world_metrics_resource_id, resolve_runtime_metric_def_key,
};
use crate::model::{Diagnostic, PanelDecl, Severity, UiNodeDecl};

/// Lower `link.tabs` assembly placeholders into `projection_slots` on board_link nodes.
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
    let Some(tabs_value) = link.get("tabs").cloned() else {
        return;
    };
    let world_hint = resolve_world_hint(link.get("world"), import_scope, target_file);
    let default_slot = link
        .get("default_slot")
        .and_then(Value::as_u64)
        .map(|v| v as usize)
        .or_else(|| {
            tabs_value
                .as_object()
                .and_then(|map| map.get("default_slot"))
                .and_then(Value::as_u64)
                .map(|v| v as usize)
        });
    let title = link
        .get("title")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    let slots = match expand_tabs_value(
        &tabs_value,
        resources,
        world_hint.as_ref(),
        diagnostics,
        target_file,
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
    };

    let mut projection_slots = Vec::new();
    for (index, slot) in slots.into_iter().enumerate() {
        let mut slot_obj = slot;
        let is_default = default_slot
            .map(|preferred| preferred == index)
            .unwrap_or(index == 0);
        slot_obj.insert("default".to_string(), Value::Bool(is_default));
        projection_slots.push(Value::Object(slot_obj));
    }

    link.insert(
        "projection_slots".to_string(),
        Value::Array(projection_slots),
    );
    if let Some(title) = title {
        link.insert("title".to_string(), Value::String(title));
    }
    link.remove("world");
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
) -> Option<Vec<Map<String, Value>>> {
    if let Some(map) = tabs.as_object() {
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
            if let Some(mut slots) =
                expand_tabs_value(item, resources, world_hint, diagnostics, target_file)
            {
                out.append(&mut slots);
            }
        }
        return Some(out);
    }
    None
}

fn expand_drilldown_tabs(
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

fn lower_projection_slot(
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
            .unwrap_or("")
            .to_string();
        if matches!(support_role.as_str(), "note" | "definition") {
            continue;
        }
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
        let slot_dataset_id = block_map
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
            })
            .unwrap_or_else(|| dataset_id.to_string());
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
        if let Some(chart_kind) = block_map.get("chart_kind") {
            slot.insert("chart_kind".to_string(), chart_kind.clone());
        }
        if let Some(mapping) = block_map.get("mapping") {
            slot.insert("mapping".to_string(), mapping.clone());
        }
        if let Some(block_id) = block_map.get("id").and_then(Value::as_str) {
            slot.insert(
                "explain_block_id".to_string(),
                Value::String(block_id.to_string()),
            );
        }
        slots.push(slot);
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
    if let Some(kind) = block.get("chart_kind").and_then(Value::as_str) {
        if !kind.trim().is_empty() {
            return "chart".to_string();
        }
    }
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
