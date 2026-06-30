use std::fs;

use super::{
    compile_app_from_root, temp_root, write_file,
};

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
    assert!(
        compiled
            .diagnostics
            .iter()
            .any(|diag| diag.code == "deprecated_scene_file_ref"),
        "legacy scene_file_ref should emit migration warning: {:?}",
        compiled.diagnostics
    );

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
    assert!(
        compiled
            .diagnostics
            .iter()
            .any(|diag| diag.code == "deprecated_scene_file_ref"),
        "app.scene = scene_file_ref(...) should emit migration warning: {:?}",
        compiled.diagnostics
    );

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
    assert!(
        compiled
            .diagnostics
            .iter()
            .any(|diag| diag.code == "deprecated_world_file_ref"),
        "legacy world_file_ref should emit migration warning: {:?}",
        compiled.diagnostics
    );
    assert!(
        compiled
            .diagnostics
            .iter()
            .any(|diag| diag.code == "deprecated_frame_file_ref"),
        "legacy frame_file_ref should emit migration warning: {:?}",
        compiled.diagnostics
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
fn compile_collects_scene_bindings_for_scene_contracts() {
    let root = temp_root("scene-bindings-scene-contract");
    let app_root = root.join("binding-app");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(id = "binding-app", scene = scene_ref(scene_file = "home.mei"))
"#,
    );
    write_file(
        &app_root.join("home.mei"),
        r#"
scene(
    profile = "page",
    bindings = {
        "detail": metric_ref("sales_total", from_dataset = "sales"),
    },
    examples = [
        {
            "id": "default",
            "bindings": {
                "detail": metric_ref("sales_total", from_dataset = "sales"),
            },
        },
    ],
)
world(
    resources = [
        ds.dataset_resource(
            id = "sales",
            source = ds.csv("sales.csv"),
            metrics = [
                ds.metric(id = "sales_total", value = 42),
            ],
        ),
    ],
)
frame(layout = flex(direction = "column"))
"#,
    );
    write_file(&app_root.join("sales.csv"), "name,value\nA,42\n");

    let compiled = compile_app_from_root(&root, &app_root).expect("compile scene bindings");
    assert_eq!(
        compiled
            .scene_bindings_by_id
            .get("home")
            .and_then(|value| value.get("detail"))
            .and_then(|value| value.get("__ref"))
            .and_then(|value| value.as_str()),
        Some("metric"),
        "scene_bindings_by_id = {:?}",
        compiled.scene_bindings_by_id
    );
    assert_eq!(
        compiled
            .scene_examples_by_id
            .get("home")
            .and_then(|value| value.as_array())
            .map(|items| items.len()),
        Some(1)
    );

    let _ = fs::remove_dir_all(&root);
}

