use super::component_for_support_role;

use serde_json::{Map, Value};

use crate::compile::materialize::imported_world_metrics_resource_id;
use crate::model::{Diagnostic, Severity};

pub(super) fn validate_analytics_slots(
    metric_id: &str,
    slots: &[Map<String, Value>],
    diagnostics: &mut Vec<Diagnostic>,
    target_file: &str,
) -> Option<()> {
    if slots
        .iter()
        .all(|slot| slot.get("layout_zone").and_then(Value::as_str) != Some("detail"))
    {
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

pub(super) fn default_detail_entry(contract: Option<&Map<String, Value>>) -> Option<Value> {
    let blocks = contract?.get("blocks").and_then(Value::as_array)?;
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

pub(super) fn find_explain_block<'a>(
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

pub(super) fn slot_from_explain_block(
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
    let shape = block_map
        .get("shape")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_ascii_lowercase();
    let component = component_for_support_role(&support_role, block_map);
    let label = block_map
        .get("label")
        .and_then(Value::as_str)
        .map(str::to_string);
    // 合约侧 dataframe product 常被写成 support_role=detail，但必须仍挂子 metric。
    // 否则客户端会回退 parent::__scalar_rowset__，健全机制清单错拿 issue 结果行。
    let needs_scoped_child = support_role == "composition"
        || support_role == "trend"
        || support_role == "dataframe"
        || shape == "dataframe";
    let scoped_from_block_id = || {
        block_map
            .get("id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|block_id| format!("{}::{}", metric_id.trim(), block_id))
    };
    let explicit_metric_id = block_map
        .get("analysis_scoped_id")
        .or_else(|| block_map.get("metric_id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let slot_metric_id = if let Some(explicit) = explicit_metric_id {
        // 已是 parent::child 则沿用；若合约误写成裸 local id，在需要作用域时补全。
        if explicit.contains("::") || !needs_scoped_child {
            explicit
        } else {
            scoped_from_block_id().unwrap_or(explicit)
        }
    } else if needs_scoped_child {
        scoped_from_block_id().unwrap_or_else(|| metric_id.to_string())
    } else {
        metric_id.to_string()
    };
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
    if let Some(date_field) = block_map
        .get("date_field")
        .or_else(|| block_map.get("dateField"))
    {
        slot.insert("date_field".to_string(), date_field.clone());
    }
    if let Some(grain) = block_map.get("grain") {
        slot.insert("grain".to_string(), grain.clone());
    }
    if let Some(mapping) = block_map.get("mapping") {
        slot.insert("mapping".to_string(), mapping.clone());
    }
    if let Some(top_n) = block_map.get("top_n") {
        slot.insert("top_n".to_string(), top_n.clone());
    }
    if let Some(value_field) = block_map
        .get("value_field")
        .or_else(|| block_map.get("valueField"))
    {
        slot.insert("value_field".to_string(), value_field.clone());
    }
    if let Some(agg) = block_map.get("agg") {
        slot.insert("agg".to_string(), agg.clone());
    }
    if let Some(block_id) = block_map.get("id").and_then(Value::as_str) {
        slot.insert(
            "explain_block_id".to_string(),
            Value::String(block_id.to_string()),
        );
    }
    slot
}
