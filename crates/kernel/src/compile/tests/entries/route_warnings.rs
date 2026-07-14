use std::fs;

use super::super::super::{compile_app_from_root, compile_app_from_root_with_options, CompileOptions};
use super::super::harness::{temp_root, write_file};

#[test]
fn preview_fragment_without_scene_contract_skips_discovered_route() {
    let root = temp_root("fragment-no-discover-route");
    let app_root = root.join("frag-preview");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(
    id = "frag-preview",
    default_stage = "home",
)

scene(
    id = "home",
    world = "home_world",
    frame = "home_frame",
    profile = "page",
)

world(
    id = "home_world",
)

frame(
    id = "home_frame",
    layout = flex(direction = "column"),
)
"#,
    );
    write_file(
        &app_root.join("widget.mei"),
        r#"
frame(
    id = "widget_frame",
    layout = flex(direction = "column"),
)

frame.add_panel(
    id = "body",
    area = "auto",
    blocks = [
        doc.markdown(area = "auto", content = "fragment widget"),
    ],
)
"#,
    );

    let compiled = compile_app_from_root_with_options(
        &root,
        &app_root,
        CompileOptions {
            preview_target: Some("widget.mei".to_string()),
            ..CompileOptions::default()
        },
    )
    .expect("compile fragment preview");
    assert!(
        !compiled
            .scene_routes
            .iter()
            .any(|route| route.target_file == "widget.mei"),
        "fragment without scene(...) must not be auto-discovered as a route"
    );
    assert!(
        compiled
            .diagnostics
            .iter()
            .any(|diag| diag.code == "public_fragment_file_deprecated"),
        "expected public_fragment_file_deprecated warning"
    );
    assert!(
        compiled
            .diagnostics
            .iter()
            .any(|diag| diag.code == "missing_scene"),
        "expected missing_scene error"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_warns_when_multi_scene_routes_omit_default_scene() {
    let root = temp_root("implicit-default-scene");
    let app_root = root.join("implicit-default-scene");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(
    id = "implicit-default-scene",
    scene = scene_ref(scene_file = "home.mei", scene_id = "home"),
)

app_add_scene(scene = scene_ref(scene_file = "detail.mei", scene_id = "detail"))
"#,
    );
    write_file(
        &app_root.join("home.mei"),
        r#"
scene(id = "home", profile = "page")

frame(
    id = "home_frame",
    layout = flex(direction = "column"),
)

frame.add_panel(
    id = "home_panel",
    area = "auto",
    blocks = [doc.markdown(area = "auto", content = "home")],
)
"#,
    );
    write_file(
        &app_root.join("detail.mei"),
        r#"
scene(id = "detail", profile = "page")

frame(
    id = "detail_frame",
    layout = flex(direction = "column"),
)

frame.add_panel(
    id = "detail_panel",
    area = "auto",
    blocks = [doc.markdown(area = "auto", content = "detail")],
)
"#,
    );

    let compiled = compile_app_from_root(&root, &app_root).expect("compile implicit default app");
    assert_eq!(compiled.active_scene.as_deref(), Some("home"));
    assert!(
        compiled
            .diagnostics
            .iter()
            .any(|diag| diag.code == "implicit_default_stage"),
        "multi-scene app without default_scene should emit explicit warning: {:?}",
        compiled.diagnostics
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_warns_when_scene_route_is_redeclared() {
    let root = temp_root("duplicate-scene-route");
    let app_root = root.join("duplicate-scene-route");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(
    id = "duplicate-scene-route",
    default_stage = "home",
)

app.add_scene(
    id = "home",
    profile = "page",
)

app_add_scene(scene = scene_ref(scene_file = "override.mei", scene_id = "home"))

scene.set_frame(
    layout = flex(direction = "column"),
)

frame.add_panel(
    id = "home_panel",
    area = "auto",
    blocks = [doc.markdown(area = "auto", content = "inline home")],
)
"#,
    );
    write_file(
        &app_root.join("override.mei"),
        r#"
scene(id = "home", profile = "page")

frame(
    id = "override_frame",
    layout = flex(direction = "column"),
)

frame.add_panel(
    id = "override_panel",
    area = "auto",
    blocks = [doc.markdown(area = "auto", content = "override home")],
)
"#,
    );

    let compiled =
        compile_app_from_root(&root, &app_root).expect("compile duplicate scene route app");
    assert_eq!(compiled.active_scene.as_deref(), Some("home"));
    assert_eq!(compiled.active_target_file, "override.mei");
    assert_eq!(compiled.scene_routes.len(), 1);
    assert!(
        compiled
            .diagnostics
            .iter()
            .any(|diag| diag.code == "duplicate_scene_route"),
        "redeclared scene route should emit override warning: {:?}",
        compiled.diagnostics
    );

    let _ = fs::remove_dir_all(&root);
}
