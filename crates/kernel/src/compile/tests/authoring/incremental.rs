use std::fs;

use super::{compile_app_from_root, compile_app_from_root_with_options, evaluate_runtime_metric_defs, temp_root, write_file, CompileOptions};

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

