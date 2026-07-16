//! Normalize v2 `page_instance` payloads for frontend drilldown (`scene_projection_assembly_by_id`).

use mei_lang_kernel::{Diagnostic, Severity};
use serde_json::{json, Map, Value};

use crate::frame_derive::{
    derive_frame_for_page_instance, frame_exports_are_isomorphic, has_plane_layout_for_key,
    is_standard_analytics_plane, legacy_bindings_analytics_frame_export, FrameDeriveContext,
    FrameDeriveResult,
};

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

pub fn normalize_page_instance_payload(
    mut payload: Value,
    derive_ctx: Option<&FrameDeriveContext<'_>>,
    diagnostics: &mut Vec<Diagnostic>,
    source_path: Option<&str>,
) -> Value {
    let Some(map) = payload.as_object_mut() else {
        return payload;
    };
    apply_frame_derivation(map, derive_ctx, diagnostics, source_path);
    unwrap_frame_export(map);
    if let Some(shell_contract) = build_shell_contract(map) {
        map.insert("shell_contract".to_string(), shell_contract);
    }
    Value::Object(std::mem::take(map))
}

fn apply_frame_derivation(
    map: &mut Map<String, Value>,
    derive_ctx: Option<&FrameDeriveContext<'_>>,
    diagnostics: &mut Vec<Diagnostic>,
    source_path: Option<&str>,
) {
    let frame_policy = map
        .get("frame_policy")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("");
    if frame_policy == "override" {
        // Author-declared special shell; keep explicit frame as-is.
        return;
    }

    let Some(page_key) = map
        .get("key")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
    else {
        // No key: still allow bindings fallback when frame omitted.
        if derive_ctx.is_none() {
            legacy_bindings_fallback(map, diagnostics, source_path);
        }
        return;
    };
    let Some(ctx) = derive_ctx else {
        legacy_bindings_fallback(map, diagnostics, source_path);
        return;
    };

    let derived = derive_frame_for_page_instance(ctx, page_key.as_str());
    let derived_frame = match &derived {
        FrameDeriveResult::Derived(frame) => Some(frame.clone()),
        FrameDeriveResult::Unavailable => None,
    };
    let plane_is_standard = is_standard_analytics_plane(ctx, page_key.as_str());

    if let Some(frame) = map.get("frame").cloned() {
        if is_default_analytics_frame_ref(&frame) {
            if let Some(derived_frame) = derived_frame {
                map.insert("frame".to_string(), derived_frame);
            }
            return;
        }
        if let Some(derived_frame) = derived_frame {
            if !frame_exports_are_isomorphic(&frame, &derived_frame) {
                if is_standard_analytics_bindings(map) {
                    push_diagnostic(
                        diagnostics,
                        Severity::Error,
                        "frame_derivation_conflict",
                        format!(
                            "page_instance `{page_key}` explicit frame conflicts with plane-derived topology"
                        ),
                        source_path,
                    );
                } else {
                    push_diagnostic(
                        diagnostics,
                        Severity::Warning,
                        "frame_topology_split",
                        format!(
                            "special page_instance `{page_key}` frame differs from plane; declare frame_policy=\"override\" when intentional"
                        ),
                        source_path,
                    );
                }
            } else {
                // Prefer plane-derived ids / spacing when isomorphic.
                map.insert("frame".to_string(), derived_frame);
            }
        }
        return;
    }

    match derived {
        FrameDeriveResult::Derived(frame) => {
            map.insert("frame".to_string(), frame);
        }
        FrameDeriveResult::Unavailable => {
            if has_plane_layout_for_key(ctx, page_key.as_str()) && !plane_is_standard {
                push_diagnostic(
                    diagnostics,
                    Severity::Warning,
                    "frame_topology_split",
                    format!(
                        "page_instance `{page_key}` has non-standard plane topology without explicit frame override"
                    ),
                    source_path,
                );
            }
            legacy_bindings_fallback(map, diagnostics, source_path);
        }
    }
}

fn legacy_bindings_fallback(
    map: &mut Map<String, Value>,
    diagnostics: &mut Vec<Diagnostic>,
    source_path: Option<&str>,
) {
    if !is_standard_analytics_bindings(map) || map.contains_key("frame") {
        return;
    }
    push_diagnostic(
        diagnostics,
        Severity::Warning,
        "frame_topology_plane_missing",
        "standard analytics page_instance missing matching plane_layout(tier=t2); using bindings fallback"
            .to_string(),
        source_path,
    );
    map.insert(
        "frame".to_string(),
        legacy_bindings_analytics_frame_export(),
    );
}

fn is_standard_analytics_bindings(map: &Map<String, Value>) -> bool {
    let Some(bindings) = map.get("bindings").and_then(Value::as_object) else {
        return false;
    };
    bindings.contains_key("filter_schema")
        && bindings.contains_key("chart")
        && bindings.contains_key("detail")
}

fn is_default_analytics_frame_ref(frame: &Value) -> bool {
    if v2_call_name(frame) != Some("frame_ref") {
        return false;
    }
    let Some(template) = v2_call_args(frame).and_then(|args| args.get("template")) else {
        return false;
    };
    matches!(
        v2_call_name(template),
        Some("analytics_frame") | Some("frame_export")
    ) || template
        .get("__template")
        .and_then(Value::as_str)
        .is_some_and(|name| name == "analytics_frame")
}

fn push_diagnostic(
    diagnostics: &mut Vec<Diagnostic>,
    severity: Severity,
    code: &str,
    message: impl Into<String>,
    source_path: Option<&str>,
) {
    diagnostics.push(Diagnostic {
        severity,
        code: code.to_string(),
        message: message.into(),
        source_path: source_path.map(str::to_string),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content_store::SEMANTIC_SCENE;
    use crate::frame_derive::FrameDeriveContext;
    use crate::mcg::registry::{McgNodeRecord, McgRegistry};
    use crate::types::{GraphNodeId, GraphNodeKind, MaterialState, PayloadRef};
    use mei_lang_kernel::HIERARCHY_PX_1;
    use std::path::Path;
    use tempfile::TempDir;

    fn analytics_plane_payload(key: &str) -> Value {
        json!({
            "__call": "plane_layout",
            "__args": {
                "id": "p-analytics",
                "key": key,
                "tier": "t2",
                "layout": {
                    "__call": "grid",
                    "__args": {
                        "rows": ["minmax(0, 1fr)"],
                        "columns": ["1fr"],
                        "areas": [["main"]],
                    }
                },
                "regions": [{
                    "__call": "region_layout",
                    "__args": {
                        "id": "main",
                        "area": "main",
                        "layout": {
                            "__call": "grid",
                            "__args": {
                                "columns": ["minmax(180px, 1fr)", "minmax(0, 5fr)"],
                                "rows": ["minmax(0, 1fr)"],
                                "areas": [["filter", "main"]],
                                "gap": "1px",
                                "padding": "1px",
                            }
                        },
                        "sections": [
                            {
                                "__call": "section_layout",
                                "__args": {
                                    "id": "filter",
                                    "area": "filter",
                                    "shell": {
                                        "__call": "content_panel",
                                        "__args": {
                                            "id": "filter_body",
                                            "area": "body",
                                            "blocks": []
                                        }
                                    }
                                }
                            },
                            {
                                "__call": "section_layout",
                                "__args": {
                                    "id": "main",
                                    "area": "main",
                                    "shell": {
                                        "__call": "content_panel",
                                        "__args": {
                                            "id": "main_body",
                                            "area": "body",
                                            "layout": {
                                                "__call": "grid",
                                                "__args": {
                                                    "columns": ["1fr"],
                                                    "rows": ["minmax(0, 2fr)", "minmax(0, 3fr)"],
                                                    "areas": [["chart"], ["detail"]],
                                                    "gap": "1px",
                                                }
                                            },
                                            "blocks": [
                                                {
                                                    "__call": "content_panel",
                                                    "__args": {
                                                        "id": "chart",
                                                        "area": "chart",
                                                        "blocks": []
                                                    }
                                                },
                                                {
                                                    "__call": "content_panel",
                                                    "__args": {
                                                        "id": "detail",
                                                        "area": "detail",
                                                        "blocks": []
                                                    }
                                                }
                                            ]
                                        }
                                    }
                                }
                            }
                        ]
                    }
                }]
            }
        })
    }

    fn registry_with_plane(app_root: &Path, key: &str) -> McgRegistry {
        let hash = "plane-test-hash";
        let env_dir = app_root.join("env/WS-20260101.0");
        let current = app_root.join("env/current");
        std::fs::create_dir_all(env_dir.join("build/store/content/semantic_scene")).expect("mkdir");
        std::fs::create_dir_all(current.parent().expect("env parent")).expect("env root");
        #[cfg(unix)]
        {
            if current.exists() {
                std::fs::remove_file(&current).ok();
            }
            std::os::unix::fs::symlink(&env_dir, &current).expect("symlink env/current");
        }
        #[cfg(not(unix))]
        {
            std::fs::create_dir_all(&current).expect("mkdir env/current");
        }
        std::fs::write(
            env_dir
                .join("build/store/content/semantic_scene")
                .join(format!("{hash}.json")),
            serde_json::to_string(&json!({
                "kind": "plane_layout",
                "payload": analytics_plane_payload(key),
            }))
            .unwrap(),
        )
        .expect("write");
        McgRegistry {
            schema_version: String::new(),
            app_id: "fx".to_string(),
            registry_revision: String::new(),
            updated_at_ms: 0,
            nodes: vec![McgNodeRecord {
                id: GraphNodeId::new(GraphNodeKind::SemanticGraph, key),
                revision: String::new(),
                state: MaterialState::Ready,
                layer: "test".to_string(),
                payload_ref: Some(PayloadRef::new(
                    SEMANTIC_SCENE,
                    hash,
                    "mei-scene-layout-fragment-v1",
                )),
                deps: Vec::new(),
                owner_resource_id: None,
                assembly_inputs: Vec::new(),
            }],
        }
    }

    #[test]
    fn derives_analytics_frame_from_plane_when_frame_omitted() {
        let key = "fx/home/plane-analytics";
        let tmp = TempDir::new().expect("tempdir");
        let app_root = tmp.path().join("apps/fx");
        std::fs::create_dir_all(&app_root).expect("mkdir");
        let registry = registry_with_plane(app_root.as_path(), key);
        let ctx = FrameDeriveContext {
            app_root: app_root.as_path(),
            registry: &registry,
        };
        let payload = json!({
            "key": key,
            "scene": "warnings_analytics_page",
            "bindings": {
                "filter_schema": { "fields": [] },
                "chart": [],
                "detail": { "kind": "table" }
            }
        });
        let mut diagnostics = Vec::new();
        let normalized = normalize_page_instance_payload(payload, Some(&ctx), &mut diagnostics, None);
        let shell = normalized.get("shell_contract").expect("shell_contract");
        assert_eq!(
            shell.get("layout_mode").and_then(Value::as_str),
            Some("analytics")
        );
        assert!(normalized.get("layout").is_some());
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn bindings_fallback_when_plane_missing() {
        let tmp = TempDir::new().expect("tempdir");
        let app_root = tmp.path().join("apps/fx");
        std::fs::create_dir_all(&app_root).expect("mkdir");
        let registry = McgRegistry {
            schema_version: String::new(),
            app_id: "fx".to_string(),
            registry_revision: String::new(),
            updated_at_ms: 0,
            nodes: vec![],
        };
        let ctx = FrameDeriveContext {
            app_root: app_root.as_path(),
            registry: &registry,
        };
        let payload = json!({
            "key": "fx/home/missing-plane",
            "scene": "warnings_analytics_page",
            "bindings": {
                "filter_schema": { "fields": [] },
                "chart": [],
                "detail": { "kind": "table" }
            }
        });
        let mut diagnostics = Vec::new();
        let normalized = normalize_page_instance_payload(payload, Some(&ctx), &mut diagnostics, None);
        assert!(normalized.get("shell_contract").is_some());
        assert!(diagnostics.iter().any(|d| d.code == "frame_topology_plane_missing"));
        assert_eq!(
            normalized
                .pointer("/layout/gap")
                .and_then(Value::as_str),
            Some(HIERARCHY_PX_1)
        );
    }

    #[test]
    fn explicit_isomorphic_frame_passes_without_conflict() {
        let key = "fx/home/plane-analytics";
        let tmp = TempDir::new().expect("tempdir");
        let app_root = tmp.path().join("apps/fx");
        std::fs::create_dir_all(&app_root).expect("mkdir");
        let registry = registry_with_plane(app_root.as_path(), key);
        let ctx = FrameDeriveContext {
            app_root: app_root.as_path(),
            registry: &registry,
        };
        let FrameDeriveResult::Derived(explicit_frame) =
            derive_frame_for_page_instance(&ctx, key)
        else {
            panic!("expected plane-derived frame for isomorphic explicit test");
        };
        let payload = json!({
            "key": key,
            "scene": "warnings_analytics_page",
            "frame": explicit_frame,
            "bindings": {
                "filter_schema": { "fields": [] },
                "chart": [],
                "detail": { "kind": "table" }
            }
        });
        let mut diagnostics = Vec::new();
        let normalized = normalize_page_instance_payload(payload, Some(&ctx), &mut diagnostics, None);
        assert!(normalized.get("shell_contract").is_some());
        assert!(
            !diagnostics
                .iter()
                .any(|d| d.code == "frame_derivation_conflict"),
            "isomorphic explicit frame should not conflict: {diagnostics:?}"
        );
    }

    #[test]
    fn conflict_diagnostic_when_explicit_frame_differs_from_plane() {
        let key = "fx/home/plane-analytics";
        let tmp = TempDir::new().expect("tempdir");
        let app_root = tmp.path().join("apps/fx");
        std::fs::create_dir_all(&app_root).expect("mkdir");
        let registry = registry_with_plane(app_root.as_path(), key);
        let ctx = FrameDeriveContext {
            app_root: app_root.as_path(),
            registry: &registry,
        };
        let payload = json!({
            "key": key,
            "scene": "warnings_analytics_page",
            "bindings": {
                "filter_schema": { "fields": [] },
                "chart": [],
                "detail": { "kind": "table" }
            },
            "frame": {
                "__call": "frame_export",
                "__args": {
                    "layout": {
                        "__call": "grid",
                        "__args": {
                            "columns": ["1fr"],
                            "rows": ["1fr"],
                            "areas": [["preview"]],
                            "gap": HIERARCHY_PX_1,
                            "padding": HIERARCHY_PX_1,
                        }
                    },
                    "panels": []
                }
            }
        });
        let mut diagnostics = Vec::new();
        let _normalized =
            normalize_page_instance_payload(payload, Some(&ctx), &mut diagnostics, None);
        assert!(diagnostics.iter().any(|d| d.code == "frame_derivation_conflict"));
    }
}
