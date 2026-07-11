use super::slot_from_explain_block;

use serde_json::{Map, Value};

use crate::compile::materialize::{
    imported_world_metrics_resource_id, resolve_runtime_metric_def_key,
};
use crate::model::{Diagnostic, Severity};

pub(crate) fn lower_projection_slot(
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

pub(super) fn build_explain_slots(
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

pub(super) fn build_root_metric_slot(
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

pub(super) fn build_slot_from_root(
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

pub(super) fn component_for_support_role(support_role: &str, block: &Map<String, Value>) -> String {
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

pub(super) fn lookup_metric_contract(
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

pub(super) fn build_analytics_filter_schema(
    slots: &[Map<String, Value>],
    rowset_dataset_id: Option<&str>,
    contract: Option<&Map<String, Value>>,
    board_filters: Option<&Value>,
) -> Value {
    if let Some(explicit) = board_filters_explicit_fields(board_filters) {
        let mut payload = serde_json::Map::new();
        payload.insert("fields".to_string(), Value::Array(explicit));
        if let Some(dataset_id) = rowset_dataset_id.map(str::trim).filter(|s| !s.is_empty()) {
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
        let (key, label) =
            analytics_filter_key_for_column(column, slot.get("label").and_then(Value::as_str));
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
                        let (key, label) = analytics_filter_key_for_column(column, Some(column));
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
    if let Some(dataset_id) = rowset_dataset_id.map(str::trim).filter(|s| !s.is_empty()) {
        payload.insert(
            "rowset_dataset_id".to_string(),
            Value::String(dataset_id.to_string()),
        );
    } else if let Some(detail_slot) = slots
        .iter()
        .find(|slot| slot.get("layout_zone").and_then(Value::as_str) == Some("detail"))
    {
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

pub(crate) fn build_generic_rowset_filter_schema(
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

pub(crate) fn parse_metric_ref_id(value: &Value) -> Option<&str> {
    let map = value.as_object()?;
    match map.get("__ref").and_then(Value::as_str) {
        Some("metric") => map
            .get("id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty()),
        Some("metric_ref") => map
            .get("__args")
            .and_then(|args| args.get("arg0").or_else(|| args.get("id")))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty()),
        _ => None,
    }
}
