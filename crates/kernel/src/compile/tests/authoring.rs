use std::fs;

use super::super::{compile_app_from_root, compile_app_from_root_with_options, CompileOptions};
use super::harness::{temp_root, write_file};

#[test]
fn compile_supports_inline_default_scene_authoring() {
    let root = temp_root("inline-default-scene");
    let app_root = root.join("demo");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(
    id = "demo",
    default_scene = "home",
)

app.add_scene(
    id = "home",
    profile = "page",
)

scene.set_world(
    resources = [
        resource(id = "welcome_doc", kind = "document", content = "hello"),
    ],
)

scene.set_frame(
    layout = flex(direction = "column"),
)

frame.add_panel(
    id = "welcome",
    area = "auto",
    blocks = [
        doc.markdown(area = "auto", resource = resource_ref("welcome_doc")),
    ],
)
"#,
    );

    let compiled = compile_app_from_root(&root, &app_root).expect("compile inline scene app");
    assert_eq!(compiled.active_target_file, "main.mei");
    let contract = compiled.scene_contract.expect("scene contract");
    assert_eq!(contract.scene.id, "home");
    assert_eq!(contract.world.expect("world").resources.len(), 1);
    assert_eq!(contract.panels.len(), 1);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_supports_incremental_world_authoring() {
    let root = temp_root("incremental-world-authoring");
    let app_root = root.join("demo");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(
    id = "demo",
)

scene(
    profile = "simulation",
)

world()

world.set_topology(
    rows = 2,
    cols = 2,
    cells = [
        cell(id = "r1c1", row = 1, col = 1, walkable = True),
        cell(id = "r1c2", row = 1, col = 2, walkable = True),
    ],
)

world.add_resource(
    resource(id = "welcome_doc", kind = "document", content = "hello"),
)

world.add_entity(
    entity(id = "guide", kind = "assistant", label = "Guide"),
)

frame(
    layout = flex(direction = "column"),
)

frame.add_panel(
    id = "welcome",
    area = "auto",
    blocks = [
        doc.markdown(area = "auto", resource = resource_ref("welcome_doc")),
    ],
)
"#,
    );

    let compiled = compile_app_from_root(&root, &app_root).expect("compile incremental world app");
    let contract = compiled.scene_contract.expect("scene contract");
    let world = contract.world.expect("world");
    assert_eq!(world.resources.len(), 1);
    assert_eq!(world.entities.len(), 1);
    assert_eq!(
        world.topology.as_ref().map(|topology| topology.rows),
        Some(2)
    );
    assert_eq!(
        world.topology.as_ref().map(|topology| topology.cols),
        Some(2)
    );
    assert!(
        compiled
            .diagnostics
            .iter()
            .all(|diag| !matches!(diag.severity, crate::Severity::Error)),
        "incremental world authoring should not produce error diagnostics"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_supports_world_metrics_as_independent_assets() {
    let root = temp_root("world-metrics-authoring");
    let app_root = root.join("demo");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(id = "demo", default_scene = "home")

scene(id = "home", profile = "page")

world(
    id = "home_world",
    metrics = [
        ds.scalar_map(id = "warning_supervision", label = "监督事项", values = {"value": "2000"}, unit = "项"),
    ],
)

world.add_metric(
    ds.scalar_map(id = "warning_models", label = "预警模型", values = {"value": "15"}, unit = "个"),
)

frame(layout = flex(direction = "column"))
frame.add_panel(
    id = "summary",
    area = "auto",
    blocks = [
        component("chart.kpi", area = "auto", props = {"metric": metric_ref("warning_supervision")}),
        component("chart.kpi", area = "auto", props = {"metric": metric_ref("warning_models")}),
    ],
)
"#,
    );

    let compiled = compile_app_from_root(&root, &app_root).expect("compile world metrics app");
    assert!(
        compiled
            .diagnostics
            .iter()
            .all(|diag| !matches!(diag.severity, crate::Severity::Error)),
        "world metrics authoring should not produce error diagnostics: {:?}",
        compiled.diagnostics
    );
    let contract = compiled.scene_contract.expect("scene contract");
    let world = contract.world.expect("world");
    assert_eq!(world.metrics.len(), 2);
    assert!(compiled.world_metrics.contains_key("warning_supervision"));
    assert!(compiled.world_metrics.contains_key("warning_models"));
    assert_eq!(
        compiled
            .world_metrics
            .get("warning_supervision")
            .and_then(|entry| entry.metric.label.as_deref()),
        Some("监督事项")
    );
    let world_metrics_resource = compiled
        .resources
        .iter()
        .find(|resource| resource.id == "__world_metrics__")
        .expect("world metrics should materialize as dataset resource");
    let world_metrics_dataset = world_metrics_resource
        .dataset
        .as_ref()
        .expect("world metrics resource payload");
    assert!(world_metrics_dataset
        .metrics
        .contains_key("warning_supervision"));
    assert!(world_metrics_dataset.metrics.contains_key("warning_models"));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_supports_incremental_frame_authoring() {
    let root = temp_root("incremental-frame-authoring");
    let app_root = root.join("demo");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(
    id = "demo",
)

scene(
    profile = "page",
)

world(
    resources = [
        resource(id = "welcome_doc", kind = "document", content = "hello"),
    ],
)

frame()

frame.set_layout(
    flex(direction = "column", gap = "12px", padding = "16px"),
)

frame.add_panel(
    id = "welcome",
    area = "auto",
    blocks = [
        doc.markdown(area = "auto", resource = resource_ref("welcome_doc")),
    ],
)
"#,
    );

    let compiled = compile_app_from_root(&root, &app_root).expect("compile incremental frame app");
    let contract = compiled.scene_contract.expect("scene contract");
    let frame = contract.frame.expect("frame");
    let layout = frame.layout.expect("frame layout");
    assert_eq!(layout.layout_type, "flex");
    assert_eq!(layout.direction.as_deref(), Some("column"));
    assert_eq!(layout.gap.as_deref(), Some("12px"));
    assert_eq!(layout.padding.as_deref(), Some("16px"));
    assert_eq!(contract.panels.len(), 1);
    assert!(
        compiled
            .diagnostics
            .iter()
            .all(|diag| !matches!(diag.severity, crate::Severity::Error)),
        "incremental frame authoring should not produce error diagnostics"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_supports_scene_file_ref_authoring() {
    let root = temp_root("scene-file-ref");
    let app_root = root.join("fire");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(
    id = "fire",
    default_scene = "room_fire_click",
)

app.add_scene(
    scene_file_ref("home.mei", id = "room_fire_click"),
)
"#,
    );
    write_file(
        &app_root.join("home.mei"),
        r#"
app.add_scene(
    id = "room_fire_click",
    profile = "simulation",
)

scene.set_frame(
    layout = flex(direction = "column"),
)

frame.add_panel(
    id = "status",
    area = "auto",
    blocks = [
        doc.markdown(area = "auto", content = "hello"),
    ],
)
"#,
    );

    let compiled = compile_app_from_root(&root, &app_root).expect("compile external scene app");
    assert_eq!(compiled.active_target_file, "home.mei");
    let contract = compiled.scene_contract.expect("scene contract");
    assert_eq!(contract.scene.id, "room_fire_click");
    assert_eq!(contract.panels.len(), 1);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_supports_app_scene_field_with_typed_scene_ref() {
    let root = temp_root("app-scene-field-typed-scene-ref");
    let app_root = root.join("typed-scene");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(
    id = "typed-scene",
    scene = scene_ref(scene_file = "home.mei"),
)
"#,
    );
    write_file(
        &app_root.join("home.mei"),
        r#"
scene(profile = "simulation")

world(
    resources = [
        resource(id = "welcome_doc", kind = "document", content = "hello"),
    ],
)

frame(layout = flex(direction = "column"))

frame.add_panel(
    id = "status",
    area = "auto",
    blocks = [
        doc.markdown(area = "auto", resource = resource_ref("welcome_doc")),
    ],
)
"#,
    );

    let compiled = compile_app_from_root(&root, &app_root).expect("compile typed scene_ref");
    assert_eq!(compiled.active_target_file, "home.mei");
    assert_eq!(compiled.active_scene.as_deref(), Some("home"));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_supports_app_scene_field_with_scene_file_ref() {
    let root = temp_root("app-scene-field-scene-file-ref");
    let app_root = root.join("fire");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(
    id = "fire",
    scene = scene_file_ref("home.mei"),
)
"#,
    );
    write_file(
        &app_root.join("home.mei"),
        r#"
scene(
    profile = "simulation",
)

world(
    resources = [
        resource(id = "welcome_doc", kind = "document", content = "hello"),
    ],
)

frame(
    layout = flex(direction = "column"),
)

frame.add_panel(
    id = "status",
    area = "auto",
    blocks = [
        doc.markdown(area = "auto", resource = resource_ref("welcome_doc")),
    ],
)
"#,
    );

    let compiled = compile_app_from_root(&root, &app_root).expect("compile app.scene app");
    assert_eq!(compiled.active_target_file, "home.mei");
    assert_eq!(compiled.active_scene.as_deref(), Some("home"));
    let contract = compiled.scene_contract.expect("scene contract");
    assert_eq!(contract.scene.id, "home");
    assert_eq!(contract.panels.len(), 1);
    assert_eq!(contract.world.expect("world").resources.len(), 1);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_supports_typed_world_and_frame_refs() {
    let root = temp_root("typed-world-frame-ref");
    let app_root = root.join("typed-ref-app");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(
    id = "typed-ref-app",
)

scene(
    world = world_ref(scene_file = "shared-world.mei"),
    frame = frame_ref(scene_file = "shared-frame.mei"),
    profile = "page",
)

frame.add_panel(
    id = "welcome",
    area = "auto",
    blocks = [
        doc.markdown(area = "auto", resource = resource_ref("welcome_doc")),
    ],
)
"#,
    );
    write_file(
        &app_root.join("shared-world.mei"),
        r#"
world()

world.add_resource(
    resource(id = "welcome_doc", kind = "document", content = "hello from typed ref"),
)
"#,
    );
    write_file(
        &app_root.join("shared-frame.mei"),
        r#"
frame()

frame.set_layout(
    flex(direction = "column"),
)
"#,
    );

    let compiled = compile_app_from_root(&root, &app_root).expect("compile typed world/frame refs");
    let contract = compiled.scene_contract.expect("scene contract");
    assert!(contract.world.is_some());
    assert!(contract.frame.is_some());
    assert!(
        compiled
            .diagnostics
            .iter()
            .all(|diag| !matches!(diag.severity, crate::Severity::Error)),
        "typed world/frame refs should not produce error diagnostics: {:?}",
        compiled.diagnostics
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_supports_app_add_scene_with_typed_scene_ref() {
    let root = temp_root("app-add-scene-typed-ref");
    let app_root = root.join("multi");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(
    id = "multi",
)

app_add_scene(scene = scene_ref(scene_file = "home.mei", scene_id = "room"))
"#,
    );
    write_file(
        &app_root.join("home.mei"),
        r#"
scene(id = "room", profile = "page")

world(resources = [resource(id = "doc", kind = "document", content = "hi")])

frame(layout = flex(direction = "column"))

frame.add_panel(id = "p1", area = "auto", blocks = [doc.markdown(area = "auto", resource = resource_ref("doc"))])
"#,
    );

    let compiled =
        compile_app_from_root(&root, &app_root).expect("compile app_add_scene scene_ref");
    assert!(
        compiled
            .scene_routes
            .iter()
            .any(|route| route.scene_id == "room" && route.target_file == "home.mei"),
        "scene_ref route should be registered"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_supports_world_and_frame_file_refs() {
    let root = temp_root("world-frame-file-ref");
    let app_root = root.join("ref-app");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(
    id = "ref-app",
)

scene(
    world = world_file_ref("shared-world.mei"),
    frame = frame_file_ref("shared-frame.mei"),
    profile = "page",
)

frame.add_panel(
    id = "welcome",
    area = "auto",
    blocks = [
        doc.markdown(area = "auto", resource = resource_ref("welcome_doc")),
    ],
)
"#,
    );
    write_file(
        &app_root.join("shared-world.mei"),
        r#"
world()

world.add_resource(
    resource(id = "welcome_doc", kind = "document", content = "hello from file ref"),
)
"#,
    );
    write_file(
        &app_root.join("shared-frame.mei"),
        r#"
frame()

frame.set_layout(
    flex(direction = "column"),
)
"#,
    );

    let compiled = compile_app_from_root(&root, &app_root).expect("compile file ref app");
    let contract = compiled.scene_contract.expect("scene contract");
    assert_eq!(contract.scene.id, "main");
    assert_eq!(
        contract
            .frame
            .as_ref()
            .and_then(|frame| frame.id.as_deref()),
        None
    );
    assert_eq!(
        contract
            .world
            .as_ref()
            .and_then(|world| world.id.as_deref()),
        None
    );
    assert!(
        compiled
            .diagnostics
            .iter()
            .all(|diag| !matches!(diag.severity, crate::Severity::Error)),
        "world/frame file refs should not produce error diagnostics"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_supports_declarative_scene_frame_binding() {
    let root = temp_root("declarative-scene-frame-binding");
    let app_root = root.join("demo");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(
    id = "demo",
    default_scene = "home",
)

scene(
    id = "home",
    world = "home_world",
    frame = "home_frame",
    profile = "page",
)

world(
    id = "home_world",
    resources = [
        resource(id = "welcome_doc", kind = "document", content = "hello"),
    ],
)

frame(
    id = "home_frame",
    layout = flex(direction = "column"),
)

frame.add_panel(
    id = "welcome",
    area = "auto",
    blocks = [
        doc.markdown(area = "auto", resource = resource_ref("welcome_doc")),
    ],
)
"#,
    );

    let compiled = compile_app_from_root(&root, &app_root).expect("compile declarative app");
    let contract = compiled.scene_contract.expect("scene contract");
    assert_eq!(contract.scene.id, "home");
    assert_eq!(
        contract
            .world
            .as_ref()
            .and_then(|world| world.id.as_deref()),
        Some("home_world")
    );
    assert_eq!(
        contract
            .frame
            .as_ref()
            .and_then(|frame| frame.id.as_deref()),
        Some("home_frame")
    );
    assert!(
        compiled
            .diagnostics
            .iter()
            .all(|diag| diag.code != "missing_scene" && diag.code != "missing_frame"),
        "declarative binding should resolve selected scene/frame"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_scene_file_ref_main_target_skips_scene_first_missing_diagnostics() {
    let root = temp_root("scene-file-ref-main-target");
    let app_root = root.join("fire");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(
    id = "fire",
    default_scene = "room_fire_click",
)

app.add_scene(
    scene_file_ref("home.mei", id = "room_fire_click"),
)
"#,
    );
    write_file(
        &app_root.join("home.mei"),
        r#"
app.add_scene(
    id = "room_fire_click",
    profile = "simulation",
)

scene.set_frame(
    layout = flex(direction = "column"),
)
"#,
    );

    let compiled = compile_app_from_root_with_options(
        &root,
        &app_root,
        CompileOptions {
            preview_target: Some("main.mei".to_string()),
            ..CompileOptions::default()
        },
    )
    .expect("compile main target");
    assert!(
        compiled
            .diagnostics
            .iter()
            .all(|diag| diag.code != "missing_scene" && diag.code != "missing_frame"),
        "legacy scene-file-ref main target should not report scene-first missing diagnostics"
    );
    assert!(
        compiled.scene_contract.is_some(),
        "previewing main.mei should fallback to default scene payload when main is index-only"
    );
    assert_eq!(compiled.active_scene.as_deref(), Some("room_fire_click"));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_rejects_panel_ref_block_embed_with_area() {
    let root = temp_root("panel-ref-block-embed-removed");
    let app_root = root.join("embed-app");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(id = "embed-app", default_scene = "home")

scene(id = "home", profile = "page")

world()

frame(layout = flex(direction = "column"))

frame.add_panel(
    id = "stack",
    area = "auto",
    blocks = [
        {"component": {
            "block_kind": "panel_ref",
            "scene_file": "child.mei",
            "area": "auto",
            "id": "child_slot",
        }},
    ],
)
"#,
    );

    let compiled = compile_app_from_root(&root, &app_root).expect("compile legacy block panel_ref");
    assert!(
        compiled.diagnostics.iter().any(|diag| {
            diag.code == "panel_ref_embed_removed"
                || (diag.code == "compile_scene_failed"
                    && diag.message.contains("panel_ref_embed_removed"))
        }),
        "legacy block panel_ref IR should report panel_ref_embed_removed: {:?}",
        compiled.diagnostics
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_supports_panel_ref_in_frame_panels() {
    let root = temp_root("panel-ref-frame-panels");
    let app_root = root.join("embed-app");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(id = "embed-app", default_scene = "home")

scene(id = "home", profile = "page")

world()

frame(
    layout = flex(direction = "column"),
    panels = [
        panel_ref(id = "inner", scene_file = "child.mei"),
    ],
)
"#,
    );
    write_file(
        &app_root.join("child.mei"),
        r#"
scene(id = "child_scene", profile = "page")
world()
frame()
frame.add_panel(id = "inner", area = "auto", blocks = [])
"#,
    );

    let compiled =
        compile_app_from_root(&root, &app_root).expect("compile panel_ref in frame.panels");
    assert!(
        compiled
            .diagnostics
            .iter()
            .all(|diag| !matches!(diag.severity, crate::Severity::Error)),
        "frame.panels panel_ref should not error: {:?}",
        compiled.diagnostics
    );
    let contract = compiled.scene_contract.expect("scene contract");
    assert_eq!(contract.panels.len(), 1);
    assert_eq!(contract.panels[0].id, "inner");

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_reports_top_level_panel_ref_embed() {
    let root = temp_root("top-level-panel-ref-embed");
    let app_root = root.join("bad-embed");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(id = "bad-embed", default_scene = "home")

scene(id = "home", profile = "page")

world()

frame()

exports.append({"component": {
    "block_kind": "panel_ref",
    "scene_file": "child.mei",
    "area": "auto",
    "id": "orphan",
}})
"#,
    );

    let compiled =
        compile_app_from_root(&root, &app_root).expect("compile top-level panel_ref embed");
    assert!(
        compiled.diagnostics.iter().any(|diag| {
            (diag.code == "panel_ref_embed_removed" || diag.code == "top_level_panel_ref_embed")
                && matches!(diag.severity, crate::Severity::Error)
        }),
        "expected panel_ref_embed_removed at scene top level: {:?}",
        compiled.diagnostics
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_reports_deprecated_panel_capsule_ref_block_kind() {
    let root = temp_root("deprecated-panel-capsule-ref");
    let app_root = root.join("legacy-embed");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(id = "legacy-embed", default_scene = "home")

scene(id = "home", profile = "page")

frame(layout = flex(direction = "column"))

frame.add_panel(
    id = "stack",
    area = "auto",
    blocks = [
        {"component": {
            "block_kind": "panel_capsule_ref",
            "scene_file": "child.mei",
            "area": "auto",
            "id": "slot",
        }},
    ],
)
"#,
    );
    write_file(
        &app_root.join("child.mei"),
        r#"
scene(id = "child_scene", profile = "page")
world()
frame()
"#,
    );

    let compiled =
        compile_app_from_root(&root, &app_root).expect("compile legacy panel_capsule_ref");
    assert!(
        compiled.diagnostics.iter().any(|diag| {
            diag.code == "deprecated_panel_capsule_ref"
                && matches!(diag.severity, crate::Severity::Error)
        }),
        "expected deprecated_panel_capsule_ref: {:?}",
        compiled.diagnostics
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_refs_scenario1_local_resource_ref_in_props() {
    let root = temp_root("refs-scenario-1");
    let app_root = root.join("refs-01");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(id = "refs-01", default_scene = "home")
scene(id = "home", profile = "page")
world(resources = [resource(id = "welcome_doc", kind = "document", content = "hello")])
frame()
frame.add_panel(
    id = "welcome",
    area = "auto",
    blocks = [doc.markdown(area = "auto", resource = resource_ref("welcome_doc"))],
)
"#,
    );
    let compiled = compile_app_from_root(&root, &app_root).expect("compile refs scenario 1");
    assert!(
        compiled
            .diagnostics
            .iter()
            .all(|diag| !matches!(diag.severity, crate::Severity::Error)),
        "scenario 1 should compile without errors: {:?}",
        compiled.diagnostics
    );
    assert!(compiled.scene_contract.is_some());
    assert!(
        !compiled
            .diagnostics
            .iter()
            .any(|diag| diag.code == "forbidden_direct_ui_data_binding"),
        "local resource_ref in props should be allowed"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_refs_scenario2_world_metrics_from_local_dataset_view() {
    let root = temp_root("refs-scenario-2");
    let app_root = root.join("refs-02");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(id = "refs-02", default_scene = "home")
scene(id = "home", profile = "page")
world(resources = [resource(id = "sales_data", kind = "dataset", source = ds.csv(path = "data/sales.csv"))])
rows = ds.data_ref("sales_data")
world.add_dataset_view(
    id = "sales_view",
    rowset = rows,
    schema = [ds.column("label", "string"), ds.column("value", "number")],
)
world.add_metric(
    ds.scalar_map(
        id = "overview",
        schema = [ds.column("total_rows", "number")],
        values = {"total_rows": ds.count(rows)},
    ),
)
world.add_metric(
    ds.computed_metric(key = "row_count", dataset = "sales_view", op = ds.count_rows(), fallback = 0),
)
frame()
frame.add_panel(
    id = "summary",
    area = "auto",
    blocks = [doc.markdown(area = "auto", resource = metric_ref("row_count"))],
)
"#,
    );
    write_file(&app_root.join("data/sales.csv"), "label,value\nA,1\n");
    let compiled = compile_app_from_root(&root, &app_root).expect("compile refs scenario 2");
    assert!(
        compiled
            .diagnostics
            .iter()
            .all(|diag| !matches!(diag.severity, crate::Severity::Error)),
        "scenario 2 should compile without errors: {:?}",
        compiled.diagnostics
    );
    assert!(
        compiled.world_metrics.contains_key("overview"),
        "world ledger should include scalar_map metric"
    );
    assert!(
        compiled.world_metrics.contains_key("row_count"),
        "world ledger should include computed metric"
    );
    assert!(
        compiled
            .resources
            .iter()
            .any(|item| item.id == "sales_view"),
        "dataset view should materialize for computed_metric dataset="
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_refs_scenario4_panel_ref_with_world_ref_imports_external_panel() {
    let root = temp_root("refs-scenario-4");
    let app_root = root.join("refs-04");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(id = "refs-04", default_scene = "home")
scene(
    id = "home",
    profile = "page",
    world = world_ref(scene_file = "panels/widget.mei"),
)
frame(
    panels = [
        panel_ref(id = "widget_panel", scene_file = "panels/widget.mei"),
    ],
)
"#,
    );
    write_file(
        &app_root.join("panels/widget.mei"),
        r#"
scene(id = "widget", profile = "page")
world(resources = [resource(id = "widget_doc", kind = "document", content = "capsule doc")])
frame()
frame.add_panel(
    id = "widget_panel",
    area = "auto",
    blocks = [doc.markdown(area = "auto", resource = resource_ref("widget_doc"))],
)
"#,
    );
    let compiled = compile_app_from_root(&root, &app_root).expect("compile refs scenario 4");
    assert!(
        compiled
            .diagnostics
            .iter()
            .all(|diag| !matches!(diag.severity, crate::Severity::Error)),
        "scenario 4 should compile with world_ref + panel_ref: {:?}",
        compiled.diagnostics
    );
    assert!(
        !compiled
            .diagnostics
            .iter()
            .any(|diag| diag.code == "imported_resource_not_authorized"),
        "world_ref should authorize widget_doc"
    );
    assert!(
        compiled
            .resources
            .iter()
            .any(|item| item.id == "widget_doc"),
        "widget_doc from external world should appear in scene resources"
    );
    let contract = compiled.scene_contract.expect("scene contract");
    assert!(
        contract.panels.iter().any(|p| p.id == "widget_panel"),
        "panel_ref should resolve widget_panel into scene panels"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_refs_scenario5_local_panel_and_resource_override_external_ledger() {
    let root = temp_root("refs-scenario-5");
    let app_root = root.join("refs-05");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(id = "refs-05", default_scene = "home")
scene(id = "home", profile = "page")
world(resources = [resource(id = "shared_doc", kind = "document", content = "host wins")])
frame(
    panels = [
        panel(
            base = panel_ref(id = "slot", scene_file = "panels/base.mei"),
            title = "覆盖后的 panel 标题",
        ),
    ],
)
"#,
    );
    write_file(
        &app_root.join("panels/base.mei"),
        r#"
scene(id = "base", profile = "page")
world(resources = [resource(id = "shared_doc", kind = "document", content = "external loses")])
frame()
frame.add_panel(
    id = "slot",
    title = "外部 panel 原标题",
    area = "auto",
    blocks = [doc.markdown(area = "auto", resource = resource_ref("shared_doc"))],
)
"#,
    );
    let compiled = compile_app_from_root(&root, &app_root).expect("compile refs scenario 5");
    assert!(
        compiled
            .diagnostics
            .iter()
            .all(|diag| !matches!(diag.severity, crate::Severity::Error)),
        "scenario 5 should compile: {:?}",
        compiled.diagnostics
    );
    let contract = compiled.scene_contract.expect("contract");
    let panel = contract
        .panels
        .iter()
        .find(|panel| panel.id == "slot")
        .expect("slot panel");
    assert_eq!(panel.title.as_deref(), Some("覆盖后的 panel 标题"));
    assert!(
        !panel.blocks.is_empty(),
        "panel(base=panel_ref) should inherit blocks from base panel"
    );
    let shared = compiled
        .resources
        .iter()
        .find(|item| item.id == "shared_doc")
        .expect("shared_doc");
    assert!(
        shared
            .document
            .as_deref()
            .is_some_and(|content| content.contains("host wins")),
        "host world should override external capsule resource on same id"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_panel_base_clones_and_overrides_area() {
    let root = temp_root("panel-base-clone");
    let app_root = root.join("clone-app");
    write_file(
        &app_root.join("external.mei"),
        r#"
scene(id = "ext", profile = "page")
world()
frame()
frame.add_panel(
    id = "widget",
    title = "外部标题",
    area = "stats",
    blocks = [doc.markdown(area = "auto", resource = resource_ref("doc"))],
)
"#,
    );
    write_file(
        &app_root.join("main.mei"),
        r#"
app(id = "clone-app", default_scene = "home")
scene(id = "home", profile = "page")
world(resources = [resource(id = "doc", kind = "document", content = "ok")])
frame(
    panels = [
        panel(
            base = panel_ref(id = "widget", scene_file = "external.mei"),
            area = "left",
        ),
    ],
)
"#,
    );
    let compiled = compile_app_from_root(&root, &app_root).expect("compile panel base clone");
    let contract = compiled.scene_contract.expect("contract");
    let panel = contract
        .panels
        .iter()
        .find(|p| p.id == "widget")
        .expect("cloned panel");
    assert_eq!(panel.area.as_deref(), Some("left"));
    assert_eq!(panel.title.as_deref(), Some("外部标题"));
    assert_eq!(panel.blocks.len(), 1);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_resource_base_clones_from_world_ref() {
    let root = temp_root("resource-base-clone");
    let app_root = root.join("res-base");
    write_file(
        &app_root.join("shared.mei"),
        r#"
scene(id = "s", profile = "page")
world(resources = [resource(id = "doc", kind = "document", content = "base")])
"#,
    );
    write_file(
        &app_root.join("main.mei"),
        r#"
app(id = "res-base", default_scene = "home")
scene(id = "home", profile = "page")
world(resources = [
    resource(
        id = "doc",
        kind = "document",
        base = resource_ref(id = "doc", scene_file = "shared.mei"),
        title = "覆盖",
    ),
])
frame()
"#,
    );
    let compiled = compile_app_from_root(&root, &app_root).expect("compile resource base");
    let contract = compiled.scene_contract.expect("contract");
    let world = contract.world.expect("world");
    let res = world.resources.iter().find(|r| r.id == "doc").expect("doc");
    assert_eq!(res.title.as_deref(), Some("覆盖"));
    assert_eq!(res.kind, "document");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_nine_grid_nested_panel_base_clones() {
    let root = temp_root("nine-grid-nested-panel");
    let app_root = root.join("grid-nested");
    write_file(
        &app_root.join("templates/cell.mei"),
        r#"
scene(id = "c", profile = "page")
world(resources = [resource(id = "x", kind = "document", content = "x")])
frame()
frame.add_panel(id = "cell", blocks = [doc.markdown(area = "auto", resource = resource_ref("x"))])
"#,
    );
    write_file(
        &app_root.join("templates/row.mei"),
        r#"
scene(id = "r", profile = "page")
world(resources = [resource(id = "a", kind = "document", content = "a")])
frame()
frame.add_panel(
    id = "row",
    layout = flex(direction = "row"),
    blocks = [
        panel(base = panel_ref(id = "cell", scene_file = "templates/cell.mei"), id = "a", blocks = [doc.markdown(area = "auto", resource = resource_ref("a"))]),
        panel(base = panel_ref(id = "cell", scene_file = "templates/cell.mei"), id = "b"),
        panel(base = panel_ref(id = "cell", scene_file = "templates/cell.mei"), id = "c"),
    ],
)
"#,
    );
    write_file(
        &app_root.join("main.mei"),
        r#"
app(id = "grid-nested", default_scene = "home")
scene(id = "home", profile = "page")
world()
frame(
    layout = flex(direction = "column"),
    panels = [
        panel(base = panel_ref(id = "row", scene_file = "templates/row.mei"), id = "r1"),
        panel(base = panel_ref(id = "row", scene_file = "templates/row.mei"), id = "r2"),
    ],
)
"#,
    );
    let compiled = compile_app_from_root(&root, &app_root).expect("compile nested panel grid");
    let contract = compiled.scene_contract.expect("contract");
    assert_eq!(contract.panels.len(), 2);
    let row1 = contract.panels.iter().find(|p| p.id == "r1").expect("r1");
    assert_eq!(
        row1.blocks.len(),
        3,
        "row should have 3 horizontal nested panels"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
#[ignore = "existing incomplete assertion; tracked separately"]
fn compile_component_base_clones_use_key() {
    let root = temp_root("component-base-clone");
    let app_root = root.join("cmp-base");
    write_file(
        &app_root.join("source.mei"),
        r#"
scene(id = "s", profile = "page")
world()
frame()
frame.add_panel(
    id = "p",
    area = "auto",
    blocks = [component("doc.markdown", id = "hero", props = {"title": "基线"})],
)
"#,
    );
    write_file(
        &app_root.join("main.mei"),
        r#"
app(id = "cmp-base", default_scene = "home")
scene(id = "home", profile = "page")
world(resources = [resource(id = "doc", kind = "document", content = "x")])
frame()
frame.add_panel(
    id = "p1",
    area = "auto",
    blocks = [
        component(
            base = component_ref(id = "hero", scene_file = "source.mei"),
            props = {"title": "克隆"},
            data = resource_ref("doc"),
        ),
    ],
)
"#,
    );
    let compiled = compile_app_from_root(&root, &app_root).expect("compile component base");
    let contract = compiled.scene_contract.expect("contract");
    let panel = contract
        .panels
        .iter()
        .find(|p| p.id == "p1")
        .expect("panel");
    assert_eq!(panel.blocks.len(), 1);
    let _ = fs::remove_dir_all(&root);
}

#[test]
#[ignore = "existing incomplete assertion; tracked separately"]
fn compile_panel_base_rejects_wrong_ref_kind() {
    let root = temp_root("panel-base-wrong-kind");
    let app_root = root.join("bad-base");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(id = "bad-base", default_scene = "home")
scene(id = "home", profile = "page")
world()
frame(
    panels = [
        panel(base = frame_ref(scene_file = "missing.mei")),
    ],
)
"#,
    );
    let compiled = compile_app_from_root(&root, &app_root).expect("compile bad panel base");
    assert!(
        compiled
            .diagnostics
            .iter()
            .any(|d| d.code == "invalid_panel_base_ref_kind"),
        "expected invalid_panel_base_ref_kind: {:?}",
        compiled.diagnostics
    );
    let _ = fs::remove_dir_all(&root);
}
