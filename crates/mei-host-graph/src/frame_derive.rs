//! 0335: derive standard analytics T2 `frame_export` from `plane_layout(tier=t2)` topology.

use std::path::Path;

use mei_lang_kernel::HIERARCHY_PX_1;
use serde_json::{json, Map, Value};

use crate::import::load_block_artifact;
use crate::mcg::registry::McgRegistry;
use crate::types::GraphNodeKind;

pub struct FrameDeriveContext<'a> {
    pub app_root: &'a Path,
    pub registry: &'a McgRegistry,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameDeriveResult {
    Derived(Value),
    Unavailable,
}

#[derive(Debug, Clone)]
struct AnalyticsTopology {
    outer_layout: Value,
    main_stack_layout: Value,
    outer_gap: String,
    outer_padding: String,
    main_gap: String,
}

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

fn string_field_map<'a>(map: Option<&'a Map<String, Value>>, keys: &[&str]) -> Option<&'a str> {
    let map = map?;
    keys.iter()
        .filter_map(|key| map.get(*key))
        .find_map(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn string_field<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a str> {
    value
        .as_object()
        .and_then(|obj| string_field_map(Some(obj), keys))
}

fn plane_body(value: &Value) -> Value {
    if v2_call_name(value) == Some("plane_layout") {
        return v2_call_args(value)
            .cloned()
            .map(Value::Object)
            .unwrap_or_else(|| value.clone());
    }
    value.clone()
}

fn layout_areas_flat(layout: &Value) -> Vec<String> {
    let Some(args) = v2_call_args(layout) else {
        return Vec::new();
    };
    args.get("areas")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|row| row.as_array())
                .flat_map(|cells| {
                    cells
                        .iter()
                        .filter_map(|cell| cell.as_str().map(str::trim))
                        .filter(|cell| !cell.is_empty())
                        .map(str::to_string)
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn layout_has_areas(layout: &Value, required: &[&str]) -> bool {
    let areas = layout_areas_flat(layout);
    required
        .iter()
        .all(|needle| areas.iter().any(|area| area == needle))
}

fn section_area(section: &Value) -> Option<&str> {
    v2_call_args(section).and_then(|args| string_field_map(Some(args), &["area"]))
}

fn panel_area(panel: &Value) -> Option<&str> {
    v2_call_args(panel).and_then(|args| string_field_map(Some(args), &["area"]))
}

fn plane_regions(plane: &Value) -> Option<Vec<Value>> {
    let body = plane_body(plane);
    let args = body.as_object()?;
    args.get("regions")
        .and_then(Value::as_array)
        .cloned()
        .filter(|items| !items.is_empty())
}

fn layout_spacing(layout: &Value, key: &str) -> Option<String> {
    v2_call_args(layout)
        .and_then(|args| args.get(key))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub fn normalize_css_spacing(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed == "1" {
        HIERARCHY_PX_1.to_string()
    } else {
        trimmed.to_string()
    }
}

fn normalize_layout_spacing(layout: &Value) -> Value {
    let Some(args) = layout.get("__args").and_then(Value::as_object).cloned() else {
        return layout.clone();
    };
    let mut normalized = args;
    for key in ["gap", "padding"] {
        if let Some(raw) = normalized
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            normalized.insert(key.to_string(), json!(normalize_css_spacing(raw)));
        }
    }
    json!({
        "__call": v2_call_name(layout).unwrap_or("grid"),
        "__args": normalized,
    })
}

fn find_plane_layout_payload(ctx: &FrameDeriveContext<'_>, key: &str) -> Option<Value> {
    let key = key.trim();
    if key.is_empty() {
        return None;
    }
    for node in ctx.registry.nodes_of_kind(GraphNodeKind::SemanticGraph) {
        let Some(pref) = node.payload_ref.as_ref() else {
            continue;
        };
        let Ok(Some(artifact)) = load_block_artifact(ctx.app_root, pref) else {
            continue;
        };
        if artifact.get("kind").and_then(Value::as_str) != Some("plane_layout") {
            continue;
        }
        let payload = artifact.get("payload").cloned().unwrap_or(Value::Null);
        let body = plane_body(&payload);
        let plane_key = string_field(&body, &["key"]).unwrap_or(node.id.key.as_str());
        if plane_key != key && node.id.key != key {
            continue;
        }
        let tier = string_field_map(body.as_object(), &["tier", "id"]).unwrap_or("");
        if tier != "t2" {
            continue;
        }
        return Some(body);
    }
    None
}

pub fn has_plane_layout_for_key(ctx: &FrameDeriveContext<'_>, key: &str) -> bool {
    find_plane_layout_payload(ctx, key).is_some()
}

fn extract_analytics_topology(plane: &Value) -> Option<AnalyticsTopology> {
    let regions = plane_regions(plane)?;
    for region in regions {
        let region_args = v2_call_args(&region)?;
        let region_layout = region_args.get("layout")?;
        if !layout_has_areas(region_layout, &["filter", "main"]) {
            continue;
        }
        let sections = region_args.get("sections").and_then(Value::as_array)?;
        let has_filter = sections
            .iter()
            .any(|section| section_area(section) == Some("filter"));
        let main_section = sections
            .iter()
            .find(|section| section_area(section) == Some("main"))?;
        if !has_filter {
            continue;
        }
        let main_args = v2_call_args(main_section)?;
        let shell = main_args.get("shell")?;
        let shell_args = v2_call_args(shell)?;
        let stack_layout = shell_args.get("layout").cloned().unwrap_or(Value::Null);
        let blocks = shell_args
            .get("blocks")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let has_chart = blocks
            .iter()
            .any(|panel| panel_area(panel) == Some("chart"))
            || layout_has_areas(&stack_layout, &["chart"]);
        let has_detail = blocks
            .iter()
            .any(|panel| panel_area(panel) == Some("detail"))
            || layout_has_areas(&stack_layout, &["detail"]);
        if !has_chart || !has_detail {
            continue;
        }
        let main_stack_layout = if layout_has_areas(&stack_layout, &["chart", "detail"]) {
            stack_layout
        } else {
            legacy_bindings_analytics_frame_export()
                .get("__args")
                .and_then(|args| args.get("panels"))
                .and_then(Value::as_array)
                .and_then(|panels| panels.get(1))
                .and_then(|main| v2_call_args(main))
                .and_then(|args| args.get("layout"))
                .cloned()
                .unwrap_or(stack_layout)
        };
        let outer_gap = layout_spacing(region_layout, "gap")
            .map(|gap| normalize_css_spacing(gap.as_str()))
            .unwrap_or_else(|| HIERARCHY_PX_1.to_string());
        let outer_padding = layout_spacing(region_layout, "padding")
            .map(|padding| normalize_css_spacing(padding.as_str()))
            .unwrap_or_else(|| HIERARCHY_PX_1.to_string());
        let main_gap = layout_spacing(&main_stack_layout, "gap")
            .map(|gap| normalize_css_spacing(gap.as_str()))
            .unwrap_or_else(|| HIERARCHY_PX_1.to_string());
        return Some(AnalyticsTopology {
            outer_layout: region_layout.clone(),
            main_stack_layout,
            outer_gap,
            outer_padding,
            main_gap,
        });
    }
    None
}

pub fn is_standard_analytics_plane(ctx: &FrameDeriveContext<'_>, page_key: &str) -> bool {
    find_plane_layout_payload(ctx, page_key)
        .as_ref()
        .is_some_and(|plane| extract_analytics_topology(plane).is_some())
}

fn build_frame_export(topo: &AnalyticsTopology) -> Value {
    let mut outer_args = v2_call_args(&topo.outer_layout)
        .cloned()
        .unwrap_or_default();
    outer_args.insert("gap".to_string(), json!(topo.outer_gap));
    outer_args.insert("padding".to_string(), json!(topo.outer_padding));
    let outer_layout = json!({
        "__call": v2_call_name(&topo.outer_layout).unwrap_or("grid"),
        "__args": outer_args,
    });

    let mut main_args = v2_call_args(&topo.main_stack_layout)
        .cloned()
        .unwrap_or_default();
    main_args.insert("gap".to_string(), json!(topo.main_gap));
    let main_layout = json!({
        "__call": v2_call_name(&topo.main_stack_layout).unwrap_or("grid"),
        "__args": main_args,
    });

    json!({
        "__call": "frame_export",
        "__args": {
            "layout": outer_layout,
            "panels": [
                {
                    "__call": "panel",
                    "__args": {
                        "id": "filter",
                        "area": "filter",
                        "slot": { "kind": "filter", "source": "filter_schema" },
                        "blocks": []
                    }
                },
                {
                    "__call": "panel",
                    "__args": {
                        "id": "main",
                        "area": "main",
                        "layout": main_layout,
                        "slot": { "kind": "container" },
                        "blocks": [
                            {
                                "__call": "panel",
                                "__args": {
                                    "id": "chart",
                                    "area": "chart",
                                    "slot": { "kind": "slots", "accepts": ["chart"], "max": 3 },
                                    "blocks": []
                                }
                            },
                            {
                                "__call": "panel",
                                "__args": {
                                    "id": "detail",
                                    "area": "detail",
                                    "slot": {
                                        "kind": "slots",
                                        "accepts": ["data_table"],
                                        "required": true
                                    },
                                    "blocks": []
                                }
                            }
                        ]
                    }
                }
            ]
        }
    })
}

pub fn derive_frame_for_page_instance(
    ctx: &FrameDeriveContext<'_>,
    page_key: &str,
) -> FrameDeriveResult {
    let Some(plane) = find_plane_layout_payload(ctx, page_key) else {
        return FrameDeriveResult::Unavailable;
    };
    let Some(topo) = extract_analytics_topology(&plane) else {
        return FrameDeriveResult::Unavailable;
    };
    FrameDeriveResult::Derived(build_frame_export(&topo))
}

pub fn legacy_bindings_analytics_frame_export() -> Value {
    json!({
        "__call": "frame_export",
        "__args": {
            "layout": {
                "__call": "grid",
                "__args": {
                    "columns": ["minmax(180px, 1fr)", "minmax(0, 5fr)"],
                    "rows": ["minmax(0, 1fr)"],
                    "areas": [["filter", "main"]],
                    "gap": HIERARCHY_PX_1,
                    "padding": HIERARCHY_PX_1,
                }
            },
            "panels": [
                {
                    "__call": "panel",
                    "__args": {
                        "id": "filter",
                        "area": "filter",
                        "slot": { "kind": "filter", "source": "filter_schema" },
                        "blocks": []
                    }
                },
                {
                    "__call": "panel",
                    "__args": {
                        "id": "main",
                        "area": "main",
                        "layout": {
                            "__call": "grid",
                            "__args": {
                                "columns": ["1fr"],
                                "rows": ["minmax(0, 2fr)", "minmax(0, 3fr)"],
                                "areas": [["chart"], ["detail"]],
                                "gap": HIERARCHY_PX_1
                            }
                        },
                        "slot": { "kind": "container" },
                        "blocks": [
                            {
                                "__call": "panel",
                                "__args": {
                                    "id": "chart",
                                    "area": "chart",
                                    "slot": { "kind": "slots", "accepts": ["chart"], "max": 3 },
                                    "blocks": []
                                }
                            },
                            {
                                "__call": "panel",
                                "__args": {
                                    "id": "detail",
                                    "area": "detail",
                                    "slot": {
                                        "kind": "slots",
                                        "accepts": ["data_table"],
                                        "required": true
                                    },
                                    "blocks": []
                                }
                            }
                        ]
                    }
                }
            ]
        }
    })
}

fn unwrap_frame_export_template(frame: &Value) -> Option<&Value> {
    if v2_call_name(frame) == Some("frame_export") {
        return Some(frame);
    }
    if v2_call_name(frame) == Some("frame_ref") {
        return v2_call_args(frame).and_then(|args| args.get("template"));
    }
    None
}

fn frame_export_signature(frame: &Value) -> Option<Value> {
    let template = unwrap_frame_export_template(frame)?;
    let args = v2_call_args(template)?;
    let layout = args.get("layout").map(normalize_layout_spacing)?;
    let panels = args.get("panels")?;
    Some(json!({
        "layout": layout,
        "panels": panels,
    }))
}

pub fn frame_exports_are_isomorphic(left: &Value, right: &Value) -> bool {
    match (frame_export_signature(left), frame_export_signature(right)) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content_store::SEMANTIC_SCENE;
    use crate::mcg::registry::McgNodeRecord;
    use crate::types::{GraphNodeId, MaterialState, PayloadRef};
    use std::fs;
    use std::path::PathBuf;

    fn optional_external_workspace() -> Option<PathBuf> {
        let raw = std::env::var("MEI_TEST_WORKSPACE").ok()?;
        let path = PathBuf::from(raw.trim());
        if path.as_os_str().is_empty() || !path.is_dir() {
            return None;
        }
        Some(path.canonicalize().unwrap_or(path))
    }

    fn symlink_env_current(app_root: &Path) {
        let env_dir = app_root.join("env/WS-20260101.0");
        let current = app_root.join("env/current");
        fs::create_dir_all(env_dir.join("build/store/content/semantic_scene")).expect("store");
        fs::create_dir_all(current.parent().expect("env parent")).expect("env root");
        #[cfg(unix)]
        {
            if current.exists() {
                fs::remove_file(&current).ok();
            }
            std::os::unix::fs::symlink(&env_dir, &current).expect("symlink env/current");
        }
        #[cfg(not(unix))]
        {
            fs::create_dir_all(&current).expect("mkdir env/current");
        }
    }

    fn sample_analytics_plane_payload(key: &str) -> Value {
        json!({
            "id": "p-warnings",
            "key": key,
            "tier": "t2",
            "regions": [{
                "__call": "region_layout",
                "__args": {
                    "layout": {
                        "__call": "grid",
                        "__args": {
                            "columns": ["minmax(180px, 1fr)", "minmax(0, 5fr)"],
                            "rows": ["minmax(0, 1fr)"],
                            "areas": [["filter", "main"]],
                            "gap": "1"
                        }
                    },
                    "sections": [
                        {
                            "__call": "section_layout",
                            "__args": {
                                "area": "filter",
                                "shell": { "__call": "content_panel", "__args": { "blocks": [] } }
                            }
                        },
                        {
                            "__call": "section_layout",
                            "__args": {
                                "area": "main",
                                "shell": {
                                    "__call": "content_panel",
                                    "__args": {
                                        "layout": {
                                            "__call": "grid",
                                            "__args": {
                                                "columns": ["1fr"],
                                                "rows": ["minmax(0, 2fr)", "minmax(0, 3fr)"],
                                                "areas": [["chart"], ["detail"]],
                                                "gap": "1"
                                            }
                                        },
                                        "blocks": [
                                            { "__call": "content_panel", "__args": { "area": "chart", "blocks": [] } },
                                            { "__call": "content_panel", "__args": { "area": "detail", "blocks": [] } }
                                        ]
                                    }
                                }
                            }
                        }
                    ]
                }
            }]
        })
    }

    fn write_plane_fixture(app_root: &Path, plane_payload: Value, hash: &str) -> McgRegistry {
        symlink_env_current(app_root);
        let artifact = json!({
            "kind": "plane_layout",
            "payload": plane_payload,
        });
        let store = app_root.join("env/WS-20260101.0/build/store/content/semantic_scene");
        fs::write(
            store.join(format!("{hash}.json")),
            serde_json::to_string(&artifact).expect("json"),
        )
        .expect("write plane artifact");
        let key = plane_payload
            .get("key")
            .and_then(Value::as_str)
            .unwrap_or("plane-key")
            .to_string();
        McgRegistry {
            schema_version: String::new(),
            app_id: "mini-data".to_string(),
            registry_revision: String::new(),
            updated_at_ms: 0,
            nodes: vec![McgNodeRecord {
                id: GraphNodeId::new(GraphNodeKind::SemanticGraph, key.clone()),
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
    fn derive_frame_from_plane_topology_normalizes_spacing() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let app_root = tmp.path().join("apps/mini-data");
        let key = "mini-data/home/t1/region-right-rail/section-warning/plane-warnings";
        let registry = write_plane_fixture(
            app_root.as_path(),
            sample_analytics_plane_payload(key),
            "planehash001",
        );
        let ctx = FrameDeriveContext {
            app_root: app_root.as_path(),
            registry: &registry,
        };
        let FrameDeriveResult::Derived(frame) = derive_frame_for_page_instance(&ctx, key) else {
            panic!("expected derived frame");
        };
        assert_eq!(
            frame["__args"]["layout"]["__args"]["gap"]
                .as_str()
                .expect("gap"),
            HIERARCHY_PX_1
        );
        assert!(is_standard_analytics_plane(&ctx, key));
        assert!(has_plane_layout_for_key(&ctx, key));
    }

    #[test]
    fn frame_exports_are_isomorphic_for_legacy_and_plane_derived() {
        let legacy = legacy_bindings_analytics_frame_export();
        let tmp = tempfile::tempdir().expect("tempdir");
        let app_root = tmp.path().join("apps/mini-data");
        let key = "mini-data/home/t1/region-right-rail/section-warning/plane-warnings";
        let registry = write_plane_fixture(
            app_root.as_path(),
            sample_analytics_plane_payload(key),
            "planehash002",
        );
        let ctx = FrameDeriveContext {
            app_root: app_root.as_path(),
            registry: &registry,
        };
        let FrameDeriveResult::Derived(derived) = derive_frame_for_page_instance(&ctx, key) else {
            panic!("expected derived frame");
        };
        assert!(frame_exports_are_isomorphic(&legacy, &derived));
    }

    #[test]
    fn derive_from_workspace_plane_when_available() {
        let Some(workspace) = optional_external_workspace() else {
            eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
            return;
        };
        let app_root = workspace.join("apps/mini-data");
        if !app_root.is_dir() {
            eprintln!("skip: mini-data app root missing under MEI_TEST_WORKSPACE");
            return;
        }
        let registry =
            crate::mcg::registry::McgRegistryWriter::load(workspace.as_path(), "mini-data");
        let ctx = FrameDeriveContext {
            app_root: app_root.as_path(),
            registry: &registry,
        };
        let key = "mini-data/home/t1/region-right-rail/section-warning/plane-warnings";
        if !has_plane_layout_for_key(&ctx, key) {
            eprintln!("skip: plane_layout fixture missing in workspace build store");
            return;
        }
        let FrameDeriveResult::Derived(frame) = derive_frame_for_page_instance(&ctx, key) else {
            panic!("expected derived frame from workspace plane");
        };
        assert_eq!(
            frame["__args"]["layout"]["__args"]["areas"][0][0].as_str(),
            Some("filter")
        );
    }
}
