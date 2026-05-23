use std::fs;

use super::{
    compile_app_from_root, compile_app_from_root_with_options, evaluate_runtime_metric_defs,
    temp_root, write_file, workspace_root, BTreeMap, CompileOptions, MetricShape,
};

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
            scene: None,
            preview_target: Some("scratch.mei".to_string()),
        },
    )
    .expect("compile preview target");
    assert_eq!(compiled.active_scene.as_deref(), Some("scratch"));
    assert_eq!(compiled.active_target_file, "scratch.mei");
    let contract = compiled.scene_contract.expect("scene contract");
    assert_eq!(contract.scene.id, "scratch");

    let _ = fs::remove_dir_all(&root);
}

/// `data/dataset/**` 下带 `scene` + `frame.add_panel` 的入口应能编译并产出可加载的数据集资源。
#[test]
fn compile_world_only_rejects_top_level_dataset_decl() {
    let root = temp_root("world-only-top-level-dataset");
    let app_root = root.join("world-only-top-level-dataset");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(
    id = "world-only-top-level-dataset",
    default_scene = "home",
)
"#,
    );
    write_file(
        &app_root.join("legacy.mei"),
        r#"
scene(id = "legacy")
world()
frame()

dataset(
    id = "legacy_rows",
    source = ds.csv(path = "data/legacy.csv"),
)

frame.add_panel(
    id = "table",
    area = "auto",
    blocks = [
        component("dataset.table", area = "auto", props = {"data": dataset_ref("legacy_rows")}),
    ],
)
"#,
    );
    write_file(&app_root.join("data/legacy.csv"), "label,value\nA,1\n");
    write_file(
        &root.join("_components/manifest.json"),
        r#"{ "components": { "dataset.table": { "tag": "mei-dataset-table", "script": "dataset-table.js" } } }"#,
    );

    let compiled = compile_app_from_root_with_options(
        &root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("legacy.mei".to_string()),
        },
    )
    .expect("compile legacy preview");

    assert!(
        compiled
            .diagnostics
            .iter()
            .any(|d| d.code == "forbidden_top_level_dataset_decls"),
        "expected world-only top-level dataset diagnostic, got {:?}",
        compiled.diagnostics
    );
}

#[test]
fn compile_world_only_rejects_legacy_resource_ids_and_unknown_world_ref() {
    let root = temp_root("world-only-id-policy");
    let app_root = root.join("world-only-id-policy");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(
    id = "world-only-id-policy",
    default_scene = "home",
)
"#,
    );
    write_file(
        &app_root.join("invalid.mei"),
        r#"
scene(id = "invalid")
world(
    resources = [
        resource(id = "__source_path__", kind = "dataset", source = ds.csv(path = "data/rows.csv")),
    ],
)
frame()
frame.add_panel(
    id = "table",
    area = "auto",
    blocks = [
        component("dataset.table", area = "auto", props = {"data": dataset_ref("missing_rows")}),
    ],
)
"#,
    );
    write_file(&app_root.join("data/rows.csv"), "label,value\nA,1\n");
    write_file(
        &root.join("_components/manifest.json"),
        r#"{ "components": { "dataset.table": { "tag": "mei-dataset-table", "script": "dataset-table.js" } } }"#,
    );

    let compiled = compile_app_from_root_with_options(
        &root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("invalid.mei".to_string()),
        },
    )
    .expect("compile invalid id preview");

    assert!(
        compiled
            .diagnostics
            .iter()
            .any(|d| d.code == "forbidden_legacy_resource_id"),
        "expected forbidden legacy resource id diagnostic, got {:?}",
        compiled.diagnostics
    );
    assert!(
        compiled
            .diagnostics
            .iter()
            .any(|d| d.code == "invalid_resource_ref"),
        "expected invalid resource ref diagnostic, got {:?}",
        compiled.diagnostics
    );
}

#[test]
fn compile_rejects_misused_world_ref_and_external_dataset_ref_in_props() {
    let root = temp_root("typed-ref-props-policy");
    let app_root = root.join("typed-ref-props-policy");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(id = "typed-ref-props-policy", default_scene = "home")
scene(id = "home")
world(resources = [resource(id = "local_ds", kind = "dataset", source = ds.csv(path = "data/a.csv"))])
frame()
frame.add_panel(
    id = "p1",
    area = "auto",
    blocks = [
        component("dataset.table", area = "auto", props = {"data": world_ref("local_ds")}),
        component("dataset.table", area = "auto", props = {"data": dataset_ref("remote_ds", scene_file = "other.mei")}),
    ],
)
"#,
    );
    write_file(&app_root.join("data/a.csv"), "x\n1\n");
    write_file(
        &root.join("_components/manifest.json"),
        r#"{ "components": { "dataset.table": { "tag": "mei-dataset-table", "script": "dataset-table.js" } } }"#,
    );
    let compiled = compile_app_from_root_with_options(
        &root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("main.mei".to_string()),
        },
    )
    .expect("compile");
    let codes: Vec<_> = compiled
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, crate::Severity::Error))
        .map(|d| d.code.as_str())
        .collect();
    assert!(
        codes.contains(&"misused_world_ref_in_props"),
        "expected misused_world_ref_in_props, got {codes:?}"
    );
    assert!(
        codes.contains(&"external_ref_requires_world_import"),
        "expected external_ref_requires_world_import, got {codes:?}"
    );
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

world.add_dataset_view(
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

world.add_metric_pack(
    id = "sales_pack",
    metrics = [
        ds.computed_metric(
            key = "pack_total_rows",
            dataset = "sales_metrics",
            op = ds.count_rows(),
            fallback = 0,
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
        component("dataset.table", area = "auto", props = {"data": dataset_ref("sales_metrics")}),
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
    assert!(dataset.runtime_metric_defs.contains_key("overview"));
    assert!(dataset.runtime_metric_defs.contains_key("ranking"));
    let pack_resource = compiled
        .resources
        .iter()
        .find(|resource| resource.id == "sales_pack")
        .expect("metric pack dataset resource");
    let pack_dataset = pack_resource
        .dataset
        .as_ref()
        .expect("metric pack as dataset");
    assert!(
        pack_dataset.metrics.contains_key("pack_total_rows"),
        "metric pack should materialize computed metrics"
    );

    let filtered_rows = dataset
        .rows
        .iter()
        .filter(|row| row.get("label").and_then(|value| value.as_str()) != Some("A"))
        .cloned()
        .collect::<Vec<_>>();
    let mut filtered_dataset = dataset.clone();
    filtered_dataset.rows = filtered_rows.clone();
    let mut datasets = compiled
        .resources
        .iter()
        .filter_map(|resource| {
            resource
                .dataset
                .clone()
                .map(|dataset| (resource.id.clone(), dataset))
        })
        .collect::<BTreeMap<_, _>>();
    datasets.insert(dataset.id.clone(), filtered_dataset.clone());
    let runtime_metrics = evaluate_runtime_metric_defs(
        &dataset.runtime_metric_defs,
        &filtered_rows,
        &datasets,
        Some(&["overview".to_string(), "ranking".to_string()]),
    )
    .expect("evaluate runtime metric defs");
    assert_eq!(
        runtime_metrics.get("overview").map(|metric| metric.shape),
        Some(MetricShape::Scalar)
    );
    assert_eq!(
        runtime_metrics.get("ranking").map(|metric| metric.shape),
        Some(MetricShape::Dataframe)
    );

    let _ = fs::remove_dir_all(&root);
}

