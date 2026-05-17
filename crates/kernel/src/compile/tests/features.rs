use std::{collections::BTreeMap, fs};

use super::harness::{temp_root, workspace_root, write_file};
use super::super::{
    compile_app_from_root, compile_app_from_root_with_options, evaluate_runtime_metric_defs,
    CompileOptions,
};
use crate::MetricShape;

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

/// `data/dataset/**` 下带 `scene` + `frame.add_panel` 的入口应能编译并产出可加载的数据集资源。
#[test]
fn compile_spbjw_preview_typical_cases_dataset_mei_has_no_missing_scene() {
    let root = workspace_root();
    let source_root = root.join("workspaces");
    let app_root = source_root.join("spbjw");
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            entry: None,
            preview_target: Some("data/dataset/典型案例/监督典型案例.mei".to_string()),
        },
    )
    .expect("compile spbjw with dataset mei preview");
    let contract = compiled
        .scene_contract
        .as_ref()
        .expect("preview should yield scene contract");
    assert!(
        !contract.panels.is_empty(),
        "preview needs frame.add_panel blocks; got 0 panels"
    );
    let path_id = "data/dataset/典型案例/监督典型案例.mei";
    let row_count = compiled
        .resources
        .iter()
        .find(|r| r.id == path_id)
        .and_then(|r| r.dataset.as_ref())
        .map(|d| d.rows.len())
        .unwrap_or(0);
    assert!(
        row_count > 0,
        "data_ref uses Mei path as resource id; expected rows from xlsx, got {row_count}"
    );
    assert!(
        compiled
            .diagnostics
            .iter()
            .all(|d| !matches!(d.severity, crate::Severity::Error)),
        "unexpected errors: {:?}",
        compiled.diagnostics
    );
}

#[test]
fn compile_spbjw_preview_enforcement_whitelist_dataset_mei_has_no_missing_scene() {
    let root = workspace_root();
    let source_root = root.join("workspaces");
    let app_root = source_root.join("spbjw");
    let target = "data/dataset/执法要素/企业白名单.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            entry: None,
            preview_target: Some(target.to_string()),
        },
    )
    .expect("compile spbjw enterprise whitelist preview");
    let contract = compiled
        .scene_contract
        .as_ref()
        .expect("preview should yield scene contract");
    assert!(
        !contract.panels.is_empty(),
        "preview needs frame.add_panel blocks; got 0 panels"
    );
    let row_count = compiled
        .resources
        .iter()
        .find(|r| r.id == target)
        .and_then(|r| r.dataset.as_ref())
        .map(|d| d.rows.len())
        .unwrap_or(0);
    assert!(
        row_count > 0,
        "expected xlsx rows for whitelist dataset, got {row_count}"
    );
    assert!(
        compiled
            .diagnostics
            .iter()
            .all(|d| !matches!(d.severity, crate::Severity::Error)),
        "unexpected errors: {:?}",
        compiled.diagnostics
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
    assert!(dataset.runtime_metric_defs.contains_key("overview"));
    assert!(dataset.runtime_metric_defs.contains_key("ranking"));

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
