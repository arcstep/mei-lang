//! Normalize v2 `page_instance` payloads for frontend drilldown (`scene_projection_assembly_by_id`).
//!
//! Compiled artifacts keep `frame_ref(template=frame_export(...))` and v2 `__call` panel AST.
//! Access drilldown JS expects `shell_contract` with `layout_mode: analytics` and zone roles.

use serde_json::{json, Map, Value};

fn v2_call_name(value: &Value) -> Option<&str> {
    value
        .get("__call")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
}

fn v2_call_args(value: &Value) -> Option<&Map<String, Value>> {
    value.get("__args").and_then(Value::as_object)
}

fn v2_slot_object(slot: &Value) -> Option<&Map<String, Value>> {
    if let Some(args) = v2_call_args(slot) {
        return Some(args);
    }
    slot.as_object()
}

fn v2_slot_kind(slot: &Value) -> Option<String> {
    let map = v2_slot_object(slot)?;
    map.get("kind")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn v2_string_list(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::trim).filter(|s| !s.is_empty()))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn v2_layout_to_value(layout: &Value) -> Option<Value> {
    if let Some(obj) = layout.as_object() {
        if obj.contains_key("areas") || obj.contains_key("columns") {
            return Some(layout.clone());
        }
    }
    let name = v2_call_name(layout)?;
    let args = v2_call_args(layout)?;
    match name {
        "grid" | "layout_metric_stack" => Some(json!({
            "columns": args.get("columns").cloned().unwrap_or(json!([])),
            "rows": args.get("rows").cloned().unwrap_or(json!([])),
            "areas": args.get("areas").cloned().unwrap_or(json!([])),
            "gap": args.get("gap").cloned().unwrap_or(Value::Null),
            "padding": args.get("padding").cloned().unwrap_or(Value::Null),
        })),
        _ => None,
    }
}

fn infer_layout_mode(zones: &[Value]) -> String {
    let roles: std::collections::BTreeSet<&str> = zones
        .iter()
        .filter_map(|zone| zone.get("role").and_then(Value::as_str))
        .collect();
    if roles.contains("tab_bar") && roles.contains("tab_content") {
        return "generic_tabs".to_string();
    }
    if roles.contains("row_preview") {
        return "list_preview".to_string();
    }
    if roles.contains("filter") && roles.contains("slots") {
        return "analytics".to_string();
    }
    String::new()
}

fn collect_v2_shell_zones(panels: &[Value], parent: &str, out: &mut Vec<Value>) {
    for panel in panels {
        let Some(args) = v2_call_args(panel) else {
            continue;
        };
        if v2_call_name(panel) != Some("panel") {
            continue;
        }
        let panel_id = args
            .get("id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_default();
        let area = args
            .get("area")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_default();
        let slot = args.get("slot");
        let role = slot
            .and_then(v2_slot_kind)
            .or_else(|| args.get("role").and_then(Value::as_str).map(str::to_string))
            .unwrap_or_default();
        if !panel_id.is_empty() && !role.is_empty() {
            let slot_map = slot.and_then(v2_slot_object);
            let mut zone = json!({
                "id": panel_id.as_str(),
                "role": role.as_str(),
                "area": area.as_str(),
                "parent": parent,
            });
            if let Some(map) = zone.as_object_mut() {
                if let Some(source) = slot_map
                    .and_then(|m| m.get("source"))
                    .and_then(Value::as_str)
                    .filter(|value| !value.is_empty())
                {
                    map.insert("source".to_string(), json!(source));
                }
                let accepts = v2_string_list(slot_map.and_then(|m| m.get("accepts")));
                if !accepts.is_empty() {
                    map.insert("accepts".to_string(), json!(accepts));
                }
                if let Some(max) = slot_map.and_then(|m| m.get("max")).and_then(Value::as_u64) {
                    map.insert("max".to_string(), json!(max));
                }
                if slot_map
                    .and_then(|m| m.get("required"))
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                {
                    map.insert("required".to_string(), json!(true));
                }
                if let Some(layout) = args.get("layout").and_then(v2_layout_to_value) {
                    map.insert("layout".to_string(), layout);
                }
            }
            out.push(zone);
        }
        let child_panels: Vec<Value> = args
            .get("blocks")
            .and_then(Value::as_array)
            .map(|blocks| {
                blocks
                    .iter()
                    .filter(|block| v2_call_name(block) == Some("panel"))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        let next_parent = if panel_id.is_empty() {
            parent
        } else {
            panel_id.as_str()
        };
        collect_v2_shell_zones(&child_panels, next_parent, out);
    }
}

fn unwrap_frame_export(payload: &mut Map<String, Value>) {
    let Some(frame) = payload.get("frame").cloned() else {
        return;
    };
    let template = if v2_call_name(&frame) == Some("frame_ref") {
        v2_call_args(&frame).and_then(|args| args.get("template").cloned())
    } else if v2_call_name(&frame) == Some("frame_export") {
        Some(frame)
    } else {
        None
    };
    let Some(template) = template else {
        return;
    };
    let Some(args) = v2_call_args(&template) else {
        return;
    };
    if let Some(layout) = args.get("layout").and_then(v2_layout_to_value) {
        payload.insert("layout".to_string(), layout.clone());
        payload.insert(
            "frame".to_string(),
            json!({
                "layout": layout,
                "id": args.get("id").cloned().unwrap_or(Value::Null),
            }),
        );
    }
    if let Some(panels) = args.get("panels").cloned() {
        payload.insert("panels".to_string(), panels);
    }
}

fn build_shell_contract(payload: &Map<String, Value>) -> Option<Value> {
    let panels = payload
        .get("panels")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut zones = Vec::new();
    collect_v2_shell_zones(&panels, "", &mut zones);
    let layout = payload
        .get("layout")
        .cloned()
        .or_else(|| {
            payload
                .get("frame")
                .and_then(|frame| frame.get("layout"))
                .cloned()
        })
        .and_then(|layout| v2_layout_to_value(&layout));
    if zones.is_empty() && layout.is_none() {
        return None;
    }
    let layout_mode = infer_layout_mode(&zones);
    let overlay_size = payload
        .get("local_nav")
        .and_then(|nav| nav.get("overlay_size").or_else(|| nav.get("overlaySize")))
        .and_then(Value::as_str)
        .map(str::to_string);
    let mut contract = json!({
        "__kind": "scene_shell_contract",
        "zones": zones,
    });
    if let Some(map) = contract.as_object_mut() {
        if !layout_mode.is_empty() {
            map.insert("layout_mode".to_string(), json!(layout_mode));
        }
        if let Some(layout) = layout {
            map.insert("layout".to_string(), layout);
        }
        if let Some(overlay_size) = overlay_size.filter(|value| !value.is_empty()) {
            map.insert("overlay_size".to_string(), json!(overlay_size));
        }
    }
    Some(contract)
}

pub fn normalize_page_instance_payload(mut payload: Value) -> Value {
    let Some(map) = payload.as_object_mut() else {
        return payload;
    };
    unwrap_frame_export(map);
    if let Some(shell_contract) = build_shell_contract(map) {
        map.insert("shell_contract".to_string(), shell_contract);
    }
    Value::Object(std::mem::take(map))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn normalize_warnings_analytics_board_shell_contract() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../workspaces/ws-demo-v2/apps/data-demo/build/active/store/content/projection_assembly/",
            "852976c410b8a28c298ab14e7698f16adf0cf80950f0058152989b308fa319d2.json"
        );
        if !std::path::Path::new(path).is_file() {
            return;
        }
        let raw = fs::read_to_string(path).expect("read fixture");
        let artifact: Value = serde_json::from_str(&raw).expect("parse fixture");
        let payload = artifact.get("payload").cloned().expect("payload");
        let normalized = normalize_page_instance_payload(payload);
        let shell = normalized.get("shell_contract").expect("shell_contract");
        assert_eq!(
            shell.get("layout_mode").and_then(Value::as_str),
            Some("analytics")
        );
        let zones = shell.get("zones").and_then(Value::as_array).expect("zones");
        assert!(zones
            .iter()
            .any(|zone| zone.get("role") == Some(&json!("filter"))));
        assert!(zones
            .iter()
            .any(|zone| zone.get("role") == Some(&json!("slots"))));
    }
}
