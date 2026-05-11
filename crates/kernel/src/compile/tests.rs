use super::compile_app_from_root;
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

fn repo_examples_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples")
        .canonicalize()
        .expect("resolve examples root")
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
    let dataset = view_resource.dataset.as_ref().expect("dataset view payload");
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
fn compile_examples_regressions() {
    let examples = repo_examples_root();
    for app_id in ["02-dataset", "03-cockpit", "05-chart"] {
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
}
