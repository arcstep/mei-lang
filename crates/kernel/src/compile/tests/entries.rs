use std::fs;

use super::super::{compile_app_from_root, compile_app_from_root_with_options, CompileOptions};
use super::harness::{temp_root, write_file};

#[test]
fn compile_declarative_main_preview_target_falls_back_to_default_scene_payload() {
    let root = temp_root("declarative-main-preview-fallback");
    let app_root = root.join("declarative-preview");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(
    id = "declarative-preview",
    default_scene = "room_fire_click",
)
app_add_scene(scene = scene_ref(scene_file = "home.mei", scene_id = "room_fire_click"))
"#,
    );
    write_file(
        &app_root.join("home.mei"),
        r#"
scene(
    id = "room_fire_click",
    frame = "home_frame",
)

frame(
    id = "home_frame",
    layout = flex(direction = "column"),
)

frame.add_panel(
    id = "status",
    area = "auto",
    blocks = [
        doc.markdown(area = "auto", content = "hello declarative"),
    ],
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
    .expect("compile declarative main target");
    assert_eq!(compiled.active_target_file, "main.mei");
    assert_eq!(compiled.active_scene.as_deref(), Some("room_fire_click"));
    let contract = compiled.scene_contract.expect("scene contract");
    assert_eq!(contract.scene.id, "room_fire_click");
    assert_eq!(contract.panels.len(), 1);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_reports_missing_scene_for_declarative_route_without_scene_decl() {
    let root = temp_root("declarative-missing-scene");
    let app_root = root.join("missing-scene");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(
    id = "missing-scene",
    default_scene = "home",
    scene = "home",
)

frame(
    id = "home_frame",
    layout = flex(direction = "column"),
)
"#,
    );

    let compiled = compile_app_from_root(&root, &app_root).expect("compile missing scene app");
    assert!(
        compiled
            .diagnostics
            .iter()
            .any(|diag| diag.code == "missing_scene"
                && matches!(diag.severity, crate::Severity::Error)),
        "declarative binding without scene declaration should report missing_scene"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_supports_nested_component_manifests() {
    let root = temp_root("nested-component-manifests");
    let app_root = root.join("nested-manifests");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(
    id = "nested-manifests",
    default_scene = "home",
)

app.add_scene(
    id = "home",
    profile = "page",
)

scene.set_frame(
    layout = flex(direction = "column"),
)

frame.add_panel(
    id = "body",
    area = "auto",
    blocks = [
        component("dataset.table", area = "auto", props = {"dataset": {"rows": []}}),
        component("chart.donut", area = "auto", props = {"dataset": {"rows": []}}),
    ],
)
"#,
    );
    write_file(
        &root.join("_components/dataset/manifest.json"),
        r#"
{
  "components": {
    "dataset.table": { "tag": "mei-dataset-table", "script": "table.js" }
  }
}
"#,
    );
    write_file(
        &root.join("_components/chart/echarts/manifest.json"),
        r#"
{
  "components": {
    "chart.donut": { "tag": "mei-chart-donut", "script": "donut.js" }
  }
}
"#,
    );
    write_file(
        &root.join("_components/dataset/table.js"),
        "customElements.define('mei-dataset-table', class extends HTMLElement {});\n",
    );
    write_file(
        &root.join("_components/chart/echarts/donut.js"),
        "customElements.define('mei-chart-donut', class extends HTMLElement {});\n",
    );

    let compiled = compile_app_from_root(&root, &app_root).expect("compile nested manifest app");
    assert!(compiled
        .component_assets
        .iter()
        .any(|asset| asset.key == "dataset.table" && asset.script == "dataset/table.js"));
    assert!(compiled
        .component_assets
        .iter()
        .any(|asset| asset.key == "chart.donut" && asset.script == "chart/echarts/donut.js"));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_collects_scene_route_registry() {
    let root = temp_root("scene-route-registry");
    let app_root = root.join("registry");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(
    id = "registry",
    default_scene = "home",
)

app.add_scene(
    id = "home",
    profile = "page",
)

app.add_scene(
    scene = scene_ref(scene_file = "default.mei", scene_id = "home_default"),
)

scene.set_frame(
    layout = flex(direction = "column"),
)

frame.add_panel(
    id = "home_panel",
    area = "auto",
    blocks = [
        doc.markdown(area = "auto", content = "home"),
    ],
)
"#,
    );
    write_file(
        &app_root.join("default.mei"),
        r#"
app.add_scene(
    id = "home_default",
)

scene.set_frame(
    layout = flex(direction = "column"),
)

frame.add_panel(
    id = "default_panel",
    area = "auto",
    blocks = [
        doc.markdown(area = "auto", content = "default"),
    ],
)
"#,
    );
    write_file(
        &root.join("_components/manifest.json"),
        r#"
{
  "components": {
    "markdown": { "tag": "mei-doc-markdown", "script": "doc-markdown.js" }
  }
}
"#,
    );

    let compiled = compile_app_from_root(&root, &app_root).expect("compile registry app");
    assert_eq!(compiled.active_scene.as_deref(), Some("home"));
    assert_eq!(compiled.active_target_file, "main.mei");
    assert_eq!(compiled.scene_routes.len(), 2);
    assert!(compiled
        .scene_routes
        .iter()
        .any(|route| route.scene_id == "home"
            && route.target_file == "main.mei"
            && route.kind == "inline"
            && route.is_default));
    assert!(compiled
        .scene_routes
        .iter()
        .any(|route| route.scene_id == "home_default"
            && route.target_file == "default.mei"
            && route.kind == "file_ref"));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_selects_requested_scene_from_registry() {
    let root = temp_root("scene-select");
    let app_root = root.join("scene-select");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(
    id = "scene-select",
    default_scene = "home",
)

app.add_scene(
    id = "home",
    profile = "page",
)

app.add_scene(
    scene = scene_ref(scene_file = "default.mei", scene_id = "home_default"),
)

scene.set_frame(
    layout = flex(direction = "column"),
)

frame.add_panel(
    id = "home_panel",
    area = "auto",
    blocks = [
        doc.markdown(area = "auto", content = "home"),
    ],
)
"#,
    );
    write_file(
        &app_root.join("default.mei"),
        r#"
app.add_scene(
    id = "home_default",
)

scene.set_frame(
    layout = flex(direction = "column"),
)

frame.add_panel(
    id = "default_panel",
    area = "auto",
    blocks = [
        doc.markdown(area = "auto", content = "default"),
    ],
)
"#,
    );
    write_file(
        &root.join("_components/manifest.json"),
        r#"
{
  "components": {
    "markdown": { "tag": "mei-doc-markdown", "script": "doc-markdown.js" }
  }
}
"#,
    );

    let compiled = compile_app_from_root_with_options(
        &root,
        &app_root,
        CompileOptions {
            scene: Some("home_default".to_string()),
            preview_target: None,
        },
    )
    .expect("compile requested scene");
    assert_eq!(compiled.active_scene.as_deref(), Some("home_default"));
    assert_eq!(compiled.active_target_file, "default.mei");
    let contract = compiled.scene_contract.expect("scene contract");
    assert_eq!(contract.scene.id, "home_default");

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn preview_fragment_without_scene_contract_skips_discovered_route() {
    let root = temp_root("fragment-no-discover-route");
    let app_root = root.join("frag-preview");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(
    id = "frag-preview",
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
