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
) -> Option<(Vec<Map<String, Value>>, Option<Value>, bool)> {
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

    let analytics_layout = board_assembly_is_analytics_layout(payload);

    let slots = if analytics_layout {
        expand_board_analytics_slots(
            metric_id,
            &dataset_id,
            contract.as_ref(),
            payload.get("charts"),
            payload.get("detail"),
            include_hero,
            resources,
            world_hint,
            diagnostics,
            target_file,
        )?
    } else {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "board_assembly_unsupported_shell".to_string(),
            message: format!(
                "board assembly for context metric `{metric_id}` requires a supported board shell (V1: analytics_drilldown_board)"
            ),
            source_path: Some(target_file.to_string()),
        });
        return None;
    };

    let filter_schema = if analytics_layout {
        Some(build_analytics_filter_schema(
            &slots,
            rowset_dataset_id.as_deref(),
            contract.as_ref(),
            payload.get("filters"),
        ))
    } else {
        None
    };
    Some((slots, filter_schema, analytics_layout))
}

fn board_assembly_is_analytics_layout(payload: &Map<String, Value>) -> bool {
    let Some(scene) = payload.get("scene").and_then(Value::as_object) else {
        return payload.contains_key("charts") || payload.contains_key("detail");
    };
    if scene
        .get("scene_id")
        .and_then(Value::as_str)
        .is_some_and(|id| id == "analytics_drilldown_board")
    {
        return true;
    }
    scene
        .get("scene_file")
        .and_then(Value::as_str)
        .is_some_and(|path| path.contains("analytics-drilldown-board"))
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
    charts: Option<&Value>,
    detail: Option<&Value>,
    include_hero: bool,
    resources: &[crate::model::LoadedResource],
    world_hint: Option<&Value>,
    diagnostics: &mut Vec<Diagnostic>,
    target_file: &str,
) -> Option<Vec<Map<String, Value>>> {
    let empty_charts: &[Value] = &[];
    let chart_entries = charts
        .and_then(Value::as_array)
        .map(|items| items.as_slice())
        .unwrap_or(empty_charts);
    if chart_entries.len() > 3 {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "board_assembly_too_many_charts".to_string(),
            message: format!(
                "board assembly for metric `{root_metric_id}` allows at most 3 charts, got {}",
                chart_entries.len()
            ),
            source_path: Some(target_file.to_string()),
        });
        return None;
    }

    let mut slots = Vec::new();
    if include_hero {
        let mut hero = build_root_metric_slot(
            root_metric_id,
            root_dataset_id,
            contract,
            "metric_card",
        );
        hero.insert(
            "layout_zone".to_string(),
            Value::String("hero".to_string()),
        );
        slots.push(hero);
    }

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
            "chart",
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
            Value::String("chart".to_string()),
        );
        slots.push(slot);
    }

    let detail_value = match detail {
        Some(value) => value.clone(),
        None => default_detail_view(contract, diagnostics, root_metric_id, target_file)?,
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
        "detail",
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
        Value::String("detail".to_string()),
    );
    detail_slot.insert("default".to_string(), Value::Bool(true));
    slots.push(detail_slot);

    validate_analytics_slots(root_metric_id, &slots, diagnostics, target_file)?;
    Some(slots)
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
    if let Some(columns) = map.get("columns") {
        slot.insert("fields".to_string(), columns.clone());
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

pub(super) fn expand_analytics_drilldown_tabs(
    payload: &Map<String, Value>,
    resources: &[crate::model::LoadedResource],
    world_hint: Option<&Value>,
    diagnostics: &mut Vec<Diagnostic>,
    target_file: &str,
) -> Option<(Vec<Map<String, Value>>, Value)> {
    let metric_ref = payload.get("source")?;
    let metric_id = parse_metric_ref_id(metric_ref)?;
    let include_hero = payload
        .get("include_hero")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let rowset_dataset_id = payload
        .get("rowset_dataset_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let (dataset_id, contract) =
        lookup_metric_contract(metric_id, resources, world_hint, diagnostics, target_file)?;

    let slots = if payload.contains_key("charts") {
        expand_explicit_analytics_slots(
            metric_id,
            &dataset_id,
            contract.as_ref(),
            payload.get("charts"),
            payload.get("detail"),
            include_hero,
            resources,
            world_hint,
            diagnostics,
            target_file,
        )?
    } else {
        expand_inferred_analytics_slots(
            metric_id,
            &dataset_id,
            contract.as_ref(),
            include_hero,
            diagnostics,
            target_file,
        )?
    };

    let filter_schema = build_analytics_filter_schema(
        &slots,
        rowset_dataset_id,
        contract.as_ref(),
        None,
    );
    Some((slots, filter_schema))
}

fn expand_inferred_analytics_slots(
    metric_id: &str,
    dataset_id: &str,
    contract: Option<&Map<String, Value>>,
    include_hero: bool,
    diagnostics: &mut Vec<Diagnostic>,
    target_file: &str,
) -> Option<Vec<Map<String, Value>>> {
    let mut slots = Vec::new();
    if include_hero {
        let mut hero = build_root_metric_slot(
            metric_id,
            dataset_id,
            contract,
            "metric_card",
        );
        hero.insert(
            "layout_zone".to_string(),
            Value::String("hero".to_string()),
        );
        slots.push(hero);
    }
    let explain_slots = build_explain_slots(metric_id, dataset_id, contract);
    let mut chart_count = 0usize;
    for mut slot in explain_slots {
        let component = slot
            .get("component")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let support_role = slot
            .get("support_role")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if component == "chart"
            && matches!(support_role.as_str(), "composition" | "trend" | "attribution")
            && chart_count >= 3
        {
            continue;
        }
        let layout_zone = if component == "chart"
            && matches!(support_role.as_str(), "composition" | "trend" | "attribution")
        {
            chart_count += 1;
            "chart"
        } else if component == "data_table" || support_role == "detail" {
            "detail"
        } else {
            "detail"
        };
        slot.insert(
            "layout_zone".to_string(),
            Value::String(layout_zone.to_string()),
        );
        if layout_zone == "detail" {
            slot.insert("default".to_string(), Value::Bool(true));
        }
        slots.push(slot);
    }

    validate_analytics_slots(metric_id, &slots, diagnostics, target_file)?;
    Some(slots)
}

fn expand_explicit_analytics_slots(
    root_metric_id: &str,
    root_dataset_id: &str,
    contract: Option<&Map<String, Value>>,
    charts: Option<&Value>,
    detail: Option<&Value>,
    include_hero: bool,
    resources: &[crate::model::LoadedResource],
    world_hint: Option<&Value>,
    diagnostics: &mut Vec<Diagnostic>,
    target_file: &str,
) -> Option<Vec<Map<String, Value>>> {
    let empty_charts: &[Value] = &[];
    let chart_entries = charts
        .and_then(Value::as_array)
        .map(|items| items.as_slice())
        .unwrap_or(empty_charts);
    if chart_entries.len() > 3 {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "analytics_projection_too_many_charts".to_string(),
            message: format!(
                "analytics drilldown for metric `{root_metric_id}` allows at most 3 charts, got {}",
                chart_entries.len()
            ),
            source_path: Some(target_file.to_string()),
        });
        return None;
    }

    let mut slots = Vec::new();
    if include_hero {
        let mut hero = build_root_metric_slot(
            root_metric_id,
            root_dataset_id,
            contract,
            "metric_card",
        );
        hero.insert(
            "layout_zone".to_string(),
            Value::String("hero".to_string()),
        );
        slots.push(hero);
    }

    for (index, entry) in chart_entries.iter().enumerate() {
        let Some(mut slot) = resolve_analytics_layout_entry(
            entry,
            root_metric_id,
            root_dataset_id,
            contract,
            resources,
            world_hint,
            diagnostics,
            target_file,
            "chart",
        ) else {
            return None;
        };
        if slot.get("component").and_then(Value::as_str) != Some("chart") {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "analytics_projection_chart_component".to_string(),
                message: format!(
                    "analytics chart slot #{index} for metric `{root_metric_id}` must project as chart"
                ),
                source_path: Some(target_file.to_string()),
            });
            return None;
        }
        slot.insert(
            "layout_zone".to_string(),
            Value::String("chart".to_string()),
        );
        slots.push(slot);
    }

    let detail_value = match detail {
        Some(value) => value.clone(),
        None => default_detail_entry(contract)?,
    };
    let Some(mut detail_slot) = resolve_analytics_layout_entry(
        &detail_value,
        root_metric_id,
        root_dataset_id,
        contract,
        resources,
        world_hint,
        diagnostics,
        target_file,
        "detail",
    ) else {
        if detail.is_none() {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "analytics_projection_missing_detail".to_string(),
                message: format!(
                    "analytics drilldown for metric `{root_metric_id}` requires detail=... or an explain detail block"
                ),
                source_path: Some(target_file.to_string()),
            });
        }
        return None;
    };
    if detail_slot.get("component").and_then(Value::as_str) != Some("data_table") {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "analytics_projection_detail_component".to_string(),
            message: format!(
                "analytics detail slot for metric `{root_metric_id}` must project as data_table"
            ),
            source_path: Some(target_file.to_string()),
        });
        return None;
    }
    detail_slot.insert(
        "layout_zone".to_string(),
        Value::String("detail".to_string()),
    );
    detail_slot.insert("default".to_string(), Value::Bool(true));
    slots.push(detail_slot);

    validate_analytics_slots(root_metric_id, &slots, diagnostics, target_file)?;
    Some(slots)
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

fn resolve_analytics_layout_entry(
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
    if let Some(block_ref) = entry.as_str().map(str::trim).filter(|s| !s.is_empty()) {
        let Some(block) = find_explain_block(contract, block_ref) else {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "analytics_projection_unknown_block".to_string(),
                message: format!(
                    "analytics {zone} entry `{block_ref}` for metric `{root_metric_id}` does not match any explain block id"
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
    let map = entry.as_object()?;
    if map.get("__kind").and_then(Value::as_str) == Some("projection_slot") {
        return lower_projection_slot(map, resources, world_hint, diagnostics, target_file);
    }
    diagnostics.push(Diagnostic {
        severity: Severity::Error,
        code: "analytics_projection_invalid_entry".to_string(),
        message: format!(
            "analytics {zone} entry for metric `{root_metric_id}` must be an explain block id string or slot(...)"
        ),
        source_path: Some(target_file.to_string()),
    });
    None
}

fn find_explain_block<'a>(
    contract: Option<&'a Map<String, Value>>,
    block_ref: &str,
) -> Option<&'a Map<String, Value>> {
    let blocks = contract?.get("blocks").and_then(Value::as_array)?;
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
        if support_role == block_ref {
            return Some(block_map);
        }
    }
    None
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
    for (column, key, label) in ANALYTICS_FILTER_COLUMNS {
        if seen.insert((*key).to_string()) {
            fields.push(serde_json::json!({
                "key": key,
                "label": label,
                "column": column,
            }));
        }
    }
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

const ANALYTICS_FILTER_COLUMNS: &[(&str, &str, &str)] = &[
    ("预警等级", "warningLevel", "预警等级"),
    ("主责单位", "agency", "主责单位"),
    ("问题分类名称", "category", "问题分类"),
    ("预警类型", "warningType", "预警类型"),
];

fn analytics_filter_key_for_column(column: &str, label: Option<&str>) -> (String, String) {
    for (known_column, key, known_label) in ANALYTICS_FILTER_COLUMNS {
        if *known_column == column {
            return (key.to_string(), known_label.to_string());
        }
    }
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
