use super::{build_slot_from_root, default_detail_entry, find_explain_block, lookup_metric_contract, lower_projection_slot, parse_metric_ref_id, slot_from_explain_block};

use serde_json::{Map, Value};

use crate::model::{Diagnostic, Severity};

pub(super) fn default_preview_view(contract: Option<&Map<String, Value>>) -> Option<Value> {
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

pub(super) fn default_detail_view(
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

pub(super) fn slot_from_board_view(
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
    slot.insert(
        "component".to_string(),
        Value::String(component.to_string()),
    );
    if let Some(label) = map
        .get("label")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
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
            let block_id = source_map
                .get("id")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())?;
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
            return lower_projection_slot(
                source_map,
                resources,
                world_hint,
                diagnostics,
                target_file,
            );
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

