use super::{compile_app_from_root, compile_app_from_root_with_options, CompileOptions};
use crate::evaluate_mei_file;
use crate::MetricShape;
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

fn temp_root(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("mei-lang-kernel-{name}-{nonce}"))
}

fn write_file(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent dirs");
    }
    fs::write(path, content).expect("write file");
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("workspace root")
}

fn build_regression_workspace_root() -> PathBuf {
    let root = temp_root("regression-workspace");
    for app_id in [
        "ds-01-dataset-baseline",
        "cockpit-01-composition-shell",
        "cockpit-02-multi-entry",
        "sim-01-fire-baseline",
        "chart-01-echarts",
        "sim-02-fire-minimal",
        "sim-03-fire-spread",
        "sim-04-fire-multiroom",
    ] {
        write_file(
            &root.join(app_id).join("main.mei"),
            &format!(
                r#"
app(
    id = "{app_id}",
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
"#
            ),
        );
    }
    write_file(
        &root.join("cockpit-02-multi-entry").join("default.mei"),
        r#"
scene(
    id = "default_compare",
    profile = "page",
)

frame(
    id = "default_compare_frame",
    layout = flex(direction = "column"),
)
"#,
    );
    root
}

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
    scene = scene_file_ref("home.mei", id = "room_fire_click"),
)
"#,
    );
    write_file(
        &app_root.join("home.mei"),
        r#"
scene(
    id = "room_fire_click",
    profile = "simulation",
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
    assert_eq!(compiled.active_entry.as_deref(), Some("room_fire_click"));
    let contract = compiled.scene_contract.expect("scene contract");
    assert_eq!(contract.scene.id, "room_fire_click");
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
    default_scene = "home",
)

scene(
    id = "home",
    world = world_file_ref("shared-world.mei", id = "shared_world"),
    frame = frame_file_ref("shared-frame.mei", id = "shared_frame"),
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
world(
    id = "shared_world",
    resources = [
        resource(id = "welcome_doc", kind = "document", content = "hello from file ref"),
    ],
)
"#,
    );
    write_file(
        &app_root.join("shared-frame.mei"),
        r#"
frame(
    id = "shared_frame",
    layout = flex(direction = "column"),
)
"#,
    );

    let compiled = compile_app_from_root(&root, &app_root).expect("compile file ref app");
    let contract = compiled.scene_contract.expect("scene contract");
    assert_eq!(contract.scene.id, "home");
    assert_eq!(
        contract
            .frame
            .as_ref()
            .and_then(|frame| frame.id.as_deref()),
        Some("shared_frame")
    );
    assert_eq!(
        contract
            .world
            .as_ref()
            .and_then(|world| world.id.as_deref()),
        Some("shared_world")
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

#[test]
fn compile_declarative_main_preview_target_falls_back_to_default_entry_payload() {
    let root = temp_root("declarative-main-preview-fallback");
    let app_root = root.join("declarative-preview");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(
    id = "declarative-preview",
    default_scene = "room_fire_click",
    entries = [
        entry(id = "room_fire_click", scene = "home.mei"),
    ],
)
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
    assert_eq!(compiled.entry_target, "main.mei");
    assert_eq!(compiled.active_entry.as_deref(), Some("room_fire_click"));
    let contract = compiled.scene_contract.expect("scene contract");
    assert_eq!(contract.scene.id, "room_fire_click");
    assert_eq!(contract.panels.len(), 1);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_reports_missing_scene_for_declarative_entry_without_scene_decl() {
    let root = temp_root("declarative-missing-scene");
    let app_root = root.join("missing-scene");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(
    id = "missing-scene",
    default_scene = "home",
    entries = [
        entry(id = "home", scene = "home", frame = "home_frame"),
    ],
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
fn compile_collects_scene_entry_registry() {
    let root = temp_root("entry-registry");
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
    scene_file_ref("default.mei", id = "home_default"),
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
    assert_eq!(compiled.active_entry.as_deref(), Some("home"));
    assert_eq!(compiled.entry_target, "main.mei");
    assert_eq!(compiled.entries.len(), 2);
    assert!(compiled.entries.iter().any(|entry| entry.entry_id == "home"
        && entry.scene_id == "home"
        && entry.target_file == "main.mei"
        && entry.kind == "inline"
        && entry.is_default));
    assert!(compiled
        .entries
        .iter()
        .any(|entry| entry.entry_id == "home_default"
            && entry.scene_id == "home_default"
            && entry.target_file == "default.mei"
            && entry.kind == "file_ref"));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_selects_requested_entry_from_registry() {
    let root = temp_root("entry-select");
    let app_root = root.join("entry-select");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(
    id = "entry-select",
    default_scene = "home",
)

app.add_scene(
    id = "home",
    profile = "page",
)

app.add_scene(
    scene_file_ref("default.mei", id = "home_default"),
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
            entry: Some("home_default".to_string()),
            preview_target: None,
        },
    )
    .expect("compile requested entry");
    assert_eq!(compiled.active_entry.as_deref(), Some("home_default"));
    assert_eq!(compiled.entry_target, "default.mei");
    let contract = compiled.scene_contract.expect("scene contract");
    assert_eq!(contract.scene.id, "home_default");

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_supports_preview_target_for_non_entry_mei_file() {
    let root = temp_root("preview-target");
    let app_root = root.join("preview-target");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(
    id = "preview-target",
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
    id = "home_panel",
    area = "auto",
    blocks = [
        doc.markdown(area = "auto", content = "home"),
    ],
)
"#,
    );
    write_file(
        &app_root.join("scratch.mei"),
        r#"
scene(
    id = "scratch",
)

scene.set_frame(
    layout = flex(direction = "column"),
)

frame.add_panel(
    id = "scratch_panel",
    area = "auto",
    blocks = [
        doc.markdown(area = "auto", content = "scratch"),
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
            entry: None,
            preview_target: Some("scratch.mei".to_string()),
        },
    )
    .expect("compile preview target");
    assert_eq!(compiled.active_entry, None);
    assert_eq!(compiled.entry_target, "scratch.mei");
    let contract = compiled.scene_contract.expect("scene contract");
    assert_eq!(contract.scene.id, "scratch");

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_materializes_dataset_view_and_metrics() {
    let root = temp_root("dataset-view-metrics");
    let app_root = root.join("analytics");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(
    id = "analytics",
    default_scene = "home",
)

app.add_scene(
    id = "home",
    profile = "page",
)

scene.set_world(
    resources = [
        resource(
            id = "sales_data",
            kind = "dataset",
            source = ds.csv(path = "data/sales.csv"),
        ),
    ],
)

rows = ds.data_ref("sales_data")

ds.dataset_view(
    id = "sales_metrics",
    title = "销售指标视图",
    rowset = rows,
    schema = [
        ds.column("label", "string"),
        ds.column("value", "number", unit = "元"),
        ds.column("unit", "string"),
    ],
    metrics = [
        ds.scalar_map(
            id = "overview",
            schema = [
                ds.column("total_rows", "number"),
                ds.column("total_value", "number"),
                ds.column("avg_value", "number"),
            ],
            values = {
                "total_rows": ds.count(rows),
                "total_value": ds.sum(ds.number(rows, "value")),
                "avg_value": ds.avg(ds.number(rows, "value")),
            },
        ),
        ds.dataframe(
            id = "ranking",
            schema = [
                ds.column("label", "string"),
                ds.column("value", "number"),
            ],
            value = ds.group_by(rows, by = "label", value = "value", agg = "sum"),
        ),
    ],
)

scene.set_frame(
    layout = flex(direction = "column"),
)

frame.add_panel(
    id = "table",
    area = "auto",
    blocks = [
        component("dataset.table", area = "auto", props = {"data": ds.data_ref("sales_metrics")}),
    ],
)
"#,
    );
    write_file(
        &app_root.join("data/sales.csv"),
        "label,value,unit\nA,100,元\nB,200,元\nC,300,元\n",
    );

    let compiled = compile_app_from_root(&root, &app_root).expect("compile dataset view app");
    let view_resource = compiled
        .resources
        .iter()
        .find(|resource| resource.id == "sales_metrics")
        .expect("derived dataset view resource");
    let dataset = view_resource
        .dataset
        .as_ref()
        .expect("dataset view payload");
    assert_eq!(dataset.rows.len(), 3);
    assert_eq!(dataset.columns, vec!["label", "value", "unit"]);
    let overview = dataset
        .metrics
        .get("overview")
        .expect("scalar metric should exist");
    assert_eq!(overview.shape, MetricShape::Scalar);
    assert!(overview.value.get("total_rows").is_some());
    let ranking = dataset
        .metrics
        .get("ranking")
        .expect("dataframe metric should exist");
    assert_eq!(ranking.shape, MetricShape::Dataframe);
    assert!(ranking.value.as_array().is_some());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_supports_nested_panels() {
    let root = temp_root("nested-panels");
    let app_root = root.join("nested");
    write_file(
        &root.join("_components/manifest.json"),
        r#"
{
  "components": {
    "markdown": { "tag": "mei-doc-markdown", "script": "doc-markdown.js" },
    "dataset.table": { "tag": "mei-dataset-table", "script": "dataset-table.js" },
    "chart.bar-mini": { "tag": "mei-chart-bar-mini", "script": "chart-bar-mini.js" }
  }
}
"#,
    );
    write_file(
        &app_root.join("main.mei"),
        r#"
app(
    id = "nested",
    default_scene = "home",
)

app.add_scene(
    id = "home",
    profile = "page",
)

scene.set_frame(
    layout = grid(
        columns = ["1fr"],
        rows = ["1fr"],
        areas = [["body"]],
    ),
)

frame.add_panel(
    id = "body",
    area = "body",
    layout = grid(
        columns = ["1fr", "1fr"],
        rows = ["1fr"],
        areas = [["left_col", "right_col"]],
    ),
    blocks = [
        panel(
            id = "left_col",
            area = "left_col",
            layout = grid(
                columns = ["1fr"],
                rows = ["1fr", "1fr"],
                areas = [["top"], ["bottom"]],
            ),
            blocks = [
                panel(
                    id = "top",
                    area = "top",
                    blocks = [
                        component("markdown", area = "auto", props = {"content": "top"}),
                    ],
                ),
                panel(
                    id = "bottom",
                    area = "bottom",
                    blocks = [
                        component("dataset.table", area = "auto", props = {"data": {"rows": []}}),
                    ],
                ),
            ],
        ),
        panel(
            id = "right_col",
            area = "right_col",
            blocks = [
                component("chart.bar-mini", area = "auto", props = {"series": []}),
            ],
        ),
    ],
)
"#,
    );

    let compiled = compile_app_from_root(&root, &app_root).expect("compile nested panel app");
    let contract = compiled.scene_contract.expect("scene contract");
    assert_eq!(contract.panels.len(), 1);
    assert_eq!(contract.panels[0].blocks.len(), 2);
    assert!(compiled
        .component_assets
        .iter()
        .any(|asset| asset.key == "markdown"));
    assert!(compiled
        .component_assets
        .iter()
        .any(|asset| asset.key == "dataset.table"));
    assert!(compiled
        .component_assets
        .iter()
        .any(|asset| asset.key == "chart.bar-mini"));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_supports_theme_declarations() {
    let root = temp_root("theme-decls");
    let app_root = root.join("theme-app");
    write_file(
        &app_root.join("main.mei"),
        r##"
app(
    id = "theme-app",
    default_scene = "home",
)

app.add_scene(
    id = "home",
    profile = "page",
    theme = "cockpit",
)

theme(
    id = "cockpit",
    font = {
        "1": "12px",
        "2": "14px",
    },
    tokens = {
        "color": {
            "text_primary": "#e2e8f0",
        },
    },
)

scene.set_frame(
    layout = flex(direction = "column"),
)

frame.add_panel(
    id = "body",
    area = "auto",
    variant = "container",
    blocks = [
        component("markdown", area = "auto", props = {"content": "hello"}),
    ],
)
"##,
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

    let compiled = compile_app_from_root(&root, &app_root).expect("compile theme app");
    let contract = compiled.scene_contract.expect("scene contract");
    assert_eq!(contract.scene.theme.as_deref(), Some("cockpit"));
    assert_eq!(contract.themes.len(), 1);
    assert_eq!(contract.themes[0].id, "cockpit");
    assert_eq!(
        contract.themes[0]
            .font
            .get("1")
            .and_then(|value| value.as_str()),
        Some("12px")
    );
    assert_eq!(
        contract.panels[0]
            .props
            .get("chrome")
            .and_then(|value| value.as_str()),
        Some("bare")
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_examples_regressions() {
    let examples = build_regression_workspace_root();
    for app_id in [
        "ds-01-dataset-baseline",
        "cockpit-01-composition-shell",
        "cockpit-02-multi-entry",
        "sim-01-fire-baseline",
        "chart-01-echarts",
        "sim-02-fire-minimal",
        "sim-03-fire-spread",
        "sim-04-fire-multiroom",
    ] {
        let app_root = examples.join(app_id);
        let compiled = compile_app_from_root(&examples, &app_root)
            .unwrap_or_else(|error| panic!("compile {app_id} failed: {error}"));
        assert!(
            compiled
                .diagnostics
                .iter()
                .all(|diag| !matches!(diag.severity, crate::Severity::Error)),
            "example {app_id} should not produce error diagnostics"
        );
        assert!(
            compiled.scene_contract.is_some(),
            "example {app_id} should contain scene contract"
        );
    }
    let _ = fs::remove_dir_all(&examples);
}

#[test]
fn compile_core_examples_single_file_baselines() {
    let root = workspace_root();
    let source_root = root.join("workspaces/examples/core");
    for app_id in ["01-single-file-doc", "02-single-scene-resource"] {
        let app_root = source_root.join(app_id);
        let compiled = compile_app_from_root(&source_root, &app_root)
            .unwrap_or_else(|error| panic!("compile {app_id} failed: {error}"));
        assert!(
            compiled
                .diagnostics
                .iter()
                .all(|diag| !matches!(diag.severity, crate::Severity::Error)),
            "example {app_id} should not produce error diagnostics"
        );
        assert!(
            compiled.scene_contract.is_some(),
            "example {app_id} should produce a scene contract"
        );
    }
}

#[test]
fn compile_core_invalid_examples_report_expected_errors() {
    let root = workspace_root();
    let source_root = root.join("workspaces/examples/core/_invalid");
    let cases = [
        ("01-multiple-apps", "multiple_apps"),
        ("02-multiple-scenes", "multiple_scenes"),
        ("03-multiple-worlds", "multiple_worlds"),
        ("04-multiple-frames", "multiple_frames"),
        ("05-scene-missing-world", "missing_world"),
        ("06-scene-missing-frame", "missing_frame"),
        ("07-app-missing-scene", "missing_app_scene"),
        (
            "08-scene-external-world-without-world_file_ref",
            "missing_bound_world",
        ),
        (
            "09-scene-external-frame-without-frame_file_ref",
            "missing_bound_frame",
        ),
    ];

    for (app_id, expected_code) in cases {
        let app_root = source_root.join(app_id);
        let compiled = compile_app_from_root(&source_root, &app_root)
            .unwrap_or_else(|error| panic!("compile {app_id} failed: {error}"));
        assert!(
            compiled
                .diagnostics
                .iter()
                .any(|diag| diag.code == expected_code
                    && matches!(diag.severity, crate::Severity::Error)),
            "example {app_id} should report `{expected_code}`"
        );
    }
}

#[test]
fn parse_cockpit_default_compare_scene_file() {
    let root = build_regression_workspace_root();
    let path = root.join("cockpit-02-multi-entry/default.mei");
    let value = evaluate_mei_file(&path).expect("parse default compare scene");
    let values = value.as_array().expect("scene file exports array");
    assert!(
        values
            .iter()
            .any(|item| item.get("kind").and_then(|value| value.as_str()) == Some("scene")),
        "default.mei should declare a scene"
    );
    assert!(
        values
            .iter()
            .any(|item| item.get("kind").and_then(|value| value.as_str()) == Some("frame")),
        "default.mei should declare a frame"
    );
    let _ = fs::remove_dir_all(&root);
}
