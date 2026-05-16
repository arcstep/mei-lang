use std::fs;

use super::harness::{temp_root, write_file};
use super::super::{compile_app_from_root, compile_app_from_root_with_options, CompileOptions};

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
        doc.markdown(area = "auto", resource = world_ref("welcome_doc")),
    ],
)
"#,
    );

    let compiled = compile_app_from_root(&root, &app_root).expect("compile inline scene app");
    assert_eq!(compiled.entry_target, "main.mei");
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
        doc.markdown(area = "auto", resource = world_ref("welcome_doc")),
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
        doc.markdown(area = "auto", resource = world_ref("welcome_doc")),
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
    assert_eq!(compiled.entry_target, "home.mei");
    let contract = compiled.scene_contract.expect("scene contract");
    assert_eq!(contract.scene.id, "room_fire_click");
    assert_eq!(contract.panels.len(), 1);

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
        doc.markdown(area = "auto", resource = world_ref("welcome_doc")),
    ],
)
"#,
    );

    let compiled = compile_app_from_root(&root, &app_root).expect("compile app.scene app");
    assert_eq!(compiled.entry_target, "home.mei");
    assert_eq!(compiled.active_entry.as_deref(), Some("home"));
    let contract = compiled.scene_contract.expect("scene contract");
    assert_eq!(contract.scene.id, "home");
    assert_eq!(contract.panels.len(), 1);
    assert_eq!(contract.world.expect("world").resources.len(), 1);

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
        doc.markdown(area = "auto", resource = world_ref("welcome_doc")),
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
    entries = [
        entry(id = "home", scene = "home", frame = "home_frame"),
    ],
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
        doc.markdown(area = "auto", resource = world_ref("welcome_doc")),
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
        "previewing main.mei should fallback to default entry payload when main is index-only"
    );
    assert_eq!(compiled.active_entry.as_deref(), Some("room_fire_click"));

    let _ = fs::remove_dir_all(&root);
}
