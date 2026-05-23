use std::fs;

use super::{compile_app_from_root, compile_app_from_root_with_options, evaluate_runtime_metric_defs, temp_root, write_file, CompileOptions};

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

