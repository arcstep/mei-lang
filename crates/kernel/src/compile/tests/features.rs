use std::{collections::BTreeMap, fs};

use super::super::{
    compile_app_from_root, compile_app_from_root_with_options, evaluate_runtime_metric_defs,
    CompileOptions,
};
use super::harness::{temp_root, workspace_root, write_file};
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
fn compile_spbjw_preview_typical_cases_dataset_mei_has_no_missing_scene() {
    let root = workspace_root();
    let source_root = root.join("workspaces");
    let app_root = source_root.join("spbjw");
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/5_典型案例/监督典型案例.mei".to_string()),
        },
    )
    .expect("compile spbjw with dataset mei preview");
    let contract = compiled.scene_contract.as_ref().unwrap_or_else(|| {
        panic!(
            "preview should yield scene contract, diagnostics: {:?}",
            compiled.diagnostics
        )
    });
    assert!(
        !contract.panels.is_empty(),
        "preview needs frame.add_panel blocks; got 0 panels"
    );
    let path_id = "typical_cases";
    let row_count = compiled
        .resources
        .iter()
        .find(|r| r.id == path_id)
        .and_then(|r| r.dataset.as_ref())
        .map(|d| d.rows.len())
        .unwrap_or(0);
    assert!(
        row_count > 0,
        "expected rows from xlsx for typical_cases, got {row_count}"
    );
    assert!(
        compiled
            .diagnostics
            .iter()
            .all(|d| !matches!(d.severity, crate::Severity::Error)),
        "unexpected errors: {:?}",
        compiled.diagnostics
    );
    assert!(
        compiled.scene_routes.iter().any(|r| {
            r.scene_id == "typical_cases" && r.target_file == "scenes/5_典型案例/监督典型案例.mei"
        }),
        "expected typical_cases in app route registry for access/manage deep links, got: {:?}",
        compiled
            .scene_routes
            .iter()
            .map(|r| (r.scene_id.as_str(), r.target_file.as_str()))
            .collect::<Vec<_>>()
    );
}

#[test]
fn compile_spbjw_select_typical_cases_scene_resolves_dataset_entry() {
    let root = workspace_root();
    let source_root = root.join("workspaces");
    let app_root = source_root.join("spbjw");
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: Some("typical_cases".to_string()),
            preview_target: None,
        },
    )
    .expect("compile spbjw with typical_cases scene (access-style)");
    assert_eq!(
        compiled.active_target_file.as_str(),
        "scenes/5_典型案例/监督典型案例.mei"
    );
    assert_eq!(compiled.active_scene.as_deref(), Some("typical_cases"));
}

#[test]
fn compile_spbjw_select_enterprise_complaints_scene_resolves_dataset_entry() {
    let root = workspace_root();
    let source_root = root.join("workspaces");
    let app_root = source_root.join("spbjw");
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: Some("enterprise_complaints".to_string()),
            preview_target: None,
        },
    )
    .expect("compile spbjw with enterprise_complaints scene (discovered route)");
    assert_eq!(
        compiled.active_target_file.as_str(),
        "scenes/2_行政检查/企业投诉.mei"
    );
    assert_eq!(
        compiled.active_scene.as_deref(),
        Some("enterprise_complaints")
    );
}

#[test]
fn compile_spbjw_preview_enforcement_whitelist_dataset_mei_has_no_missing_scene() {
    let root = workspace_root();
    let source_root = root.join("workspaces");
    let app_root = source_root.join("spbjw");
    let target = "scenes/1_执法要素/企业白名单.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some(target.to_string()),
        },
    )
    .expect("compile spbjw enterprise whitelist preview");
    let contract = compiled.scene_contract.as_ref().unwrap_or_else(|| {
        panic!(
            "preview should yield scene contract, diagnostics: {:?}",
            compiled.diagnostics
        )
    });
    assert!(
        !contract.panels.is_empty(),
        "preview needs frame.add_panel blocks; got 0 panels"
    );
    let row_count = compiled
        .resources
        .iter()
        .find(|r| r.id == "enterprise_whitelist")
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
fn compile_spbjw_dataset_preview_with_wrong_scene_query_still_resolves_entry_scene() {
    let root = workspace_root();
    let source_root = root.join("workspaces");
    let app_root = source_root.join("spbjw");
    let target = "scenes/1_执法要素/企业白名单.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: Some("企业白名单".to_string()),
            preview_target: Some(target.to_string()),
        },
    )
    .expect("compile spbjw whitelist with filename-like scene query");
    assert_eq!(compiled.active_target_file, target);
    assert_eq!(
        compiled.active_scene.as_deref(),
        Some("enterprise_whitelist")
    );
    assert!(
        !compiled
            .diagnostics
            .iter()
            .any(|d| d.code == "unknown_scene"),
        "preview_target route should satisfy scene anchor without unknown_scene: {:?}",
        compiled.diagnostics
    );
}

#[test]
fn compile_spbjw_dataset_preview_with_explicit_scene_and_focus_stays_preview_only() {
    let root = workspace_root();
    let source_root = root.join("workspaces");
    let app_root = source_root.join("spbjw");
    let target = "scenes/1_执法要素/企业白名单.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: Some("enterprise_whitelist".to_string()),
            preview_target: Some(target.to_string()),
        },
    )
    .expect("compile spbjw whitelist scene+focus");
    assert_eq!(
        compiled.active_scene.as_deref(),
        Some("enterprise_whitelist")
    );
    assert_eq!(compiled.active_target_file, target);
    assert!(
        !compiled
            .diagnostics
            .iter()
            .any(|d| d.code == "unknown_scene"),
        "explicit scene+focus should not warn unknown_scene: {:?}",
        compiled.diagnostics
    );
}

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
    metric_label = {"font": "2"},
    metric_value = {"font": "4"},
    metric_unit = {"font": "1"},
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
        contract.themes[0]
            .metric_value
            .get("font")
            .and_then(|value| value.as_str()),
        Some("4")
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
fn compile_spbjw_preview_widget_elements_succeeds() {
    let root = workspace_root();
    let source_root = root.join("workspaces");
    let app_root = source_root.join("spbjw");
    let started = std::time::Instant::now();
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/layouts/左栏.mei".to_string()),
        },
    )
    .expect("compile spbjw layout left preview");
    let elapsed = started.elapsed();
    assert_eq!(compiled.active_target_file, "scenes/layouts/左栏.mei");
    let errors: Vec<_> = compiled
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, crate::Severity::Error))
        .collect();
    assert!(
        errors.is_empty(),
        "widget elements preview errors: {:?}",
        errors
    );
    let contract = compiled
        .scene_contract
        .as_ref()
        .expect("preview scene contract");
    assert_eq!(contract.scene.id, "layout_left");
    assert!(
        contract.panels.len() >= 3,
        "layout left should resolve frame.panels panel_ref slots, got {}",
        contract.panels.len()
    );
    assert!(
        contract.panels.iter().any(|p| !p.blocks.is_empty()),
        "layout left panels should carry blocks from external panel lookup"
    );
    let stats = contract
        .panels
        .iter()
        .find(|p| p.id == "enforcement_elements_stats")
        .expect("enforcement stats panel from panel_ref");
    let panel_layout = stats
        .layout
        .as_ref()
        .expect("panel_ref must preserve panel.layout from source");
    assert_eq!(panel_layout.layout_type, "grid");
    assert!(
        !stats.blocks.is_empty(),
        "stats panel should carry title + metrics body blocks"
    );
    assert!(
        compiled
            .resources
            .iter()
            .any(|resource| resource.dataset.is_some()),
        "layout left preview needs selective dataset catalog, got ids: {:?}",
        compiled
            .resources
            .iter()
            .map(|r| r.id.as_str())
            .collect::<Vec<_>>()
    );
    let dataset_resources: Vec<_> = compiled
        .resources
        .iter()
        .filter(|r| r.dataset.is_some())
        .collect();
    assert!(
        dataset_resources.len() <= 14,
        "manage widget preview should use selective catalog, not full scan (got {}): {:?}",
        dataset_resources.len(),
        dataset_resources
            .iter()
            .map(|r| r.id.as_str())
            .collect::<Vec<_>>()
    );
    assert!(
        elapsed.as_secs() < 9,
        "manage widget preview should not compile home + full catalog (21 xlsx), took {:?}",
        elapsed
    );
}

#[test]
fn compile_spbjw_preview_widget_metrics_system_succeeds() {
    let root = workspace_root();
    let source_root = root.join("workspaces");
    let app_root = source_root.join("spbjw");
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/4_监督和问题办理/预警模型.mei".to_string()),
        },
    )
    .expect("compile spbjw warning models preview");
    let errors: Vec<_> = compiled
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, crate::Severity::Error))
        .collect();
    assert!(
        errors.is_empty(),
        "metrics widget preview errors: {:?}",
        errors
    );
    assert_eq!(
        compiled.active_target_file,
        "scenes/4_监督和问题办理/预警模型.mei"
    );
    assert!(
        compiled.resources.iter().any(|r| r.id == "warning_models"),
        "expected warning_models dataset in resources"
    );
}

#[test]
fn compile_spbjw_preview_widget_supervision_warning_succeeds() {
    let root = workspace_root();
    let source_root = root.join("workspaces");
    let app_root = source_root.join("spbjw");
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/layouts/右栏.mei".to_string()),
        },
    )
    .expect("compile spbjw layout right preview");
    assert_eq!(compiled.active_target_file, "scenes/layouts/右栏.mei");
    let errors: Vec<_> = compiled
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, crate::Severity::Error))
        .collect();
    assert!(
        errors.is_empty(),
        "supervision widget preview errors: {:?}",
        errors
    );
    let contract = compiled
        .scene_contract
        .as_ref()
        .expect("preview scene contract");
    assert_eq!(contract.scene.id, "layout_right");
    assert!(
        contract.panels.len() >= 4,
        "layout right should resolve multiple panel_ref slots, got {}",
        contract.panels.len()
    );
    assert!(
        contract.panels.iter().any(|p| !p.blocks.is_empty()),
        "layout right panels should carry blocks from external panel lookup"
    );
    assert!(
        compiled.resources.iter().any(|r| r.dataset.is_some()),
        "layout right preview should materialize datasets from referenced panels, got: {:?}",
        compiled
            .resources
            .iter()
            .filter(|r| r.dataset.is_some())
            .map(|r| r.id.as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn compile_spbjw_preview_widget_typical_cases_succeeds() {
    let root = workspace_root();
    let source_root = root.join("workspaces");
    let app_root = source_root.join("spbjw");
    let started = std::time::Instant::now();
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/5_典型案例/监督典型案例.mei".to_string()),
        },
    )
    .expect("compile spbjw typical cases preview");
    let elapsed = started.elapsed();
    assert_eq!(
        compiled.active_target_file,
        "scenes/5_典型案例/监督典型案例.mei"
    );
    let errors: Vec<_> = compiled
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, crate::Severity::Error))
        .collect();
    assert!(
        errors.is_empty(),
        "typical cases widget preview errors: {:?}",
        errors
    );
    let contract = compiled
        .scene_contract
        .as_ref()
        .expect("preview scene contract");
    assert_eq!(contract.scene.id, "typical_cases");
    assert!(
        !contract.panels[0].blocks.is_empty(),
        "typical_cases preview should render blocks"
    );
    let dataset_resources: Vec<_> = compiled
        .resources
        .iter()
        .filter(|r| r.dataset.is_some())
        .collect();
    assert!(
        !dataset_resources.is_empty(),
        "typical_cases preview should materialize dataset resources, got: {:?}",
        dataset_resources
            .iter()
            .map(|r| r.id.as_str())
            .collect::<Vec<_>>()
    );
    assert!(
        compiled.resources.iter().any(|r| r.id == "typical_cases"),
        "missing typical_cases resource"
    );
    assert!(
        elapsed.as_secs() < 8,
        "widget preview with selective catalog should compile faster than full scan, took {:?}",
        elapsed
    );
}

#[test]
fn compile_spbjw_overview_preview_materializes_imported_metrics() {
    let root = workspace_root();
    let source_root = root.join("workspaces");
    let app_root = source_root.join("spbjw");
    let target = "scenes/layouts/左栏.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some(target.to_string()),
        },
    )
    .expect("compile spbjw layout left preview");
    let errors: Vec<_> = compiled
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, crate::Severity::Error))
        .collect();
    assert!(
        errors.is_empty(),
        "layout left preview errors: {:?}",
        errors
    );
    let ids: Vec<_> = compiled.resources.iter().map(|r| r.id.as_str()).collect();
    let units = compiled
        .resources
        .iter()
        .find(|r| r.id == "enforcement_units")
        .unwrap_or_else(|| panic!("expected enforcement_units in catalog, got {ids:?}"));
    let dataset = units.dataset.as_ref().expect("enforcement_units dataset");
    assert!(
        !dataset.runtime_metric_defs.is_empty(),
        "imported dataset should carry runtime metric defs"
    );
    assert!(
        dataset.metrics.contains_key("enforcement_units_count"),
        "expected enforcement_units_count metric"
    );
}

#[test]
fn compile_spbjw_preview_home_scene_succeeds() {
    let root = workspace_root();
    let source_root = root.join("workspaces");
    let app_root = source_root.join("spbjw");
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/home.mei".to_string()),
        },
    )
    .expect("compile spbjw home preview");
    assert_eq!(compiled.active_target_file, "scenes/home.mei");
    let errors: Vec<_> = compiled
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, crate::Severity::Error))
        .collect();
    assert!(errors.is_empty(), "home preview errors: {:?}", errors);
    let contract = compiled
        .scene_contract
        .as_ref()
        .expect("preview scene contract");
    assert_eq!(contract.scene.id, "home");
    let frame = contract.frame.as_ref().expect("home should have frame");
    assert!(
        frame.layout.is_some(),
        "home should expose frame grid layout"
    );
    assert!(
        contract.panels.len() >= 10,
        "home should flatten panel_ref slots into scene panels, got {}",
        contract.panels.len()
    );
    for area in [
        "header",
        "left_1",
        "left_2",
        "left_3",
        "center_top",
        "center_bottom",
        "right_1",
        "right_4",
    ] {
        assert!(
            contract
                .panels
                .iter()
                .any(|panel| panel.area.as_deref() == Some(area)),
            "missing grid area panel: {area}"
        );
    }
    let overview = contract
        .panels
        .iter()
        .find(|p| p.id == "enforcement_elements_stats")
        .expect("enforcement elements stats from panel(base=panel_ref)");
    assert!(
        !overview.blocks.is_empty(),
        "home panel(base=panel_ref) should inherit blocks from external panel"
    );
    let resource_ids: Vec<_> = compiled.resources.iter().map(|r| r.id.as_str()).collect();
    assert!(
        compiled
            .resources
            .iter()
            .any(|r| r.id == "enforcement_units"),
        "home preview catalog should materialize panel_ref datasets, got {resource_ids:?}"
    );
    let viewport = frame
        .props
        .get("viewport")
        .and_then(|value| value.as_object())
        .expect("home frame should declare viewport props");
    assert_eq!(
        viewport.get("design_width").and_then(|v| v.as_i64()),
        Some(1920)
    );
    assert_eq!(
        viewport.get("design_height").and_then(|v| v.as_i64()),
        Some(1080)
    );
    assert_eq!(contract.themes.len(), 1);
    assert_eq!(contract.themes[0].id, "cockpit");
    let inspection = compiled
        .resources
        .iter()
        .find(|r| r.id == "administrative_inspection")
        .and_then(|r| r.dataset.as_ref())
        .expect("administrative_inspection from 行政检查.mei");
    assert!(inspection.metrics.contains_key("inspections_total_count"));
    assert!(inspection.metrics.contains_key("park_inspection_count"));
}

#[test]
fn compile_spbjw_preview_main_mei_keeps_inspection_and_penalty_cockpit_metrics() {
    let root = workspace_root();
    let source_root = root.join("workspaces");
    let app_root = source_root.join("spbjw");
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("main.mei".to_string()),
        },
    )
    .expect("compile spbjw main preview");
    let inspection = compiled
        .resources
        .iter()
        .find(|r| r.id == "administrative_inspection")
        .and_then(|r| r.dataset.as_ref())
        .expect("administrative_inspection");
    assert!(inspection.metrics.contains_key("inspections_total_count"));
    assert!(inspection
        .metrics
        .contains_key("inspections_6m_count_trend"));
    let penalty = compiled
        .resources
        .iter()
        .find(|r| r.id == "penalty_result_list")
        .and_then(|r| r.dataset.as_ref())
        .expect("penalty_result_list");
    assert!(penalty.metrics.contains_key("penalties_today_count"));
    assert!(penalty.metrics.contains_key("penalties_6m_amount_trend"));
    assert!(
        inspection.metrics.contains_key("park_inspection_count"),
        "catalog should merge park metrics without dropping cockpit defs"
    );
}

#[test]
fn compile_spbjw_preview_logistics_park_vector_succeeds() {
    let root = workspace_root();
    let source_root = root.join("workspaces");
    let app_root = source_root.join("spbjw");
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/6_物流园区/园区统计.mei".to_string()),
        },
    )
    .expect("compile spbjw logistics preview");
    let errors: Vec<_> = compiled
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, crate::Severity::Error))
        .collect();
    assert!(
        errors.is_empty(),
        "logistics_park_vector preview errors: {:?}",
        errors
    );
    let contract = compiled
        .scene_contract
        .as_ref()
        .expect("preview scene contract");
    assert!(
        !contract.panels.is_empty(),
        "expected stats/charts/table panels"
    );
    let logistics = compiled
        .resources
        .iter()
        .find(|r| r.id == "logistics_park_vector")
        .and_then(|r| r.dataset.as_ref())
        .expect("logistics_park_vector dataset");
    assert!(logistics.metrics.contains_key("logistics_parks_count"));
    assert_eq!(
        logistics.rows.len(),
        3,
        "geojson FeatureCollection should yield 3 park rows"
    );
    let inspection = compiled
        .resources
        .iter()
        .find(|r| r.id == "administrative_inspection")
        .and_then(|r| r.dataset.as_ref())
        .expect("administrative_inspection dataset");
    assert!(
        inspection.metrics.contains_key("park_inspection_count"),
        "catalog should merge 园区统计 park metrics into administrative_inspection"
    );
    let inspection_by_park = inspection
        .metrics
        .get("park_inspection_count")
        .expect("park_inspection_count metric");
    let by_park_rows = inspection_by_park
        .value
        .as_array()
        .or_else(|| {
            inspection_by_park
                .value
                .get("value")
                .and_then(|v| v.as_array())
        })
        .unwrap_or_else(|| {
            panic!(
                "dataframe metric rows expected array, got: {}",
                inspection_by_park.value
            );
        });
    assert!(
        !by_park_rows.is_empty(),
        "park_inspection_count should have grouped rows, got {by_park_rows:?}"
    );
    assert!(
        by_park_rows[0]
            .get("园区名称")
            .and_then(|v| v.as_str())
            .is_some(),
        "group_by should use 园区名称 field, not label: {:?}",
        by_park_rows[0]
    );
    let total = inspection
        .metrics
        .get("park_inspection_total")
        .and_then(|m| {
            m.value
                .get("value")
                .and_then(|v| v.as_f64())
                .or_else(|| m.value.as_f64())
        })
        .unwrap_or(-1.0);
    assert!(
        total > 0.0 && total < 100.0,
        "park_inspection_total should be enterprise-matched inspections on preview rows, got {total}"
    );
}

#[test]
fn compile_refs_scenario3_world_ref_imports_external_resources_for_props() {
    let root = temp_root("refs-scenario-3");
    let app_root = root.join("refs-03");
    write_file(
        &app_root.join("shared-world.mei"),
        r#"
world(resources = [resource(id = "shared_doc", kind = "document", content = "from external world")])
"#,
    );
    write_file(
        &app_root.join("shared-frame.mei"),
        r#"
scene(id = "shared", profile = "page")
world()
frame()
"#,
    );
    write_file(
        &app_root.join("main.mei"),
        r#"
app(id = "refs-03", default_scene = "home")
scene(
    id = "home",
    profile = "page",
    world = world_ref(scene_file = "shared-world.mei"),
    frame = frame_ref(scene_file = "shared-frame.mei"),
)
frame.add_panel(
    id = "welcome",
    area = "auto",
    blocks = [doc.markdown(area = "auto", resource = resource_ref("shared_doc"))],
)
"#,
    );
    let compiled = compile_app_from_root(&root, &app_root).expect("compile refs scenario 3");
    assert!(
        compiled
            .diagnostics
            .iter()
            .all(|diag| !matches!(diag.severity, crate::Severity::Error)),
        "scenario 3 should compile with imported world: {:?}",
        compiled.diagnostics
    );
    assert!(
        compiled
            .resources
            .iter()
            .any(|item| item.id == "shared_doc"),
        "world_ref should make shared_doc available to props"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn compile_builtin_text_shorthand() {
    let root = temp_root("mei-text-shorthand");
    let app_root = root.join("text-app");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(id = "text-app", default_scene = "home")

scene(id = "home", profile = "page")

world(id = "home_world", resources = [])

frame(layout = flex(direction = "column"))

frame.add_panel(
    id = "p",
    area = "auto",
    blocks = [
        text("我是文本"),
        text(html = "<b>加粗</b>"),
    ],
)
"#,
    );
    write_file(
        &root.join("_components/mei/manifest.json"),
        r#"
{
  "components": {
    "mei.text": { "tag": "mei-text", "script": "text.js" }
  }
}
"#,
    );
    write_file(
        &root.join("_components/mei/text.js"),
        "// stub for compile asset resolution",
    );

    let compiled = compile_app_from_root(&root, &app_root).expect("compile text shorthand");
    assert!(
        compiled
            .diagnostics
            .iter()
            .all(|diag| !matches!(diag.severity, crate::Severity::Error)),
        "text shorthand should compile: {:?}",
        compiled.diagnostics
    );
    let contract = compiled.scene_contract.expect("scene contract");
    let panel = contract
        .panels
        .iter()
        .find(|p| p.id == "p")
        .expect("panel p");
    assert_eq!(panel.blocks.len(), 2);
    for (idx, expected) in ["mei.text", "mei.text"].iter().enumerate() {
        match &panel.blocks[idx] {
            crate::UiNodeDecl::Block(block) => assert_eq!(block.use_key, *expected),
            other => panic!("block {idx} should be Block, got {other:?}"),
        }
    }
    assert!(
        compiled
            .component_assets
            .iter()
            .any(|a| a.key == "mei.text"),
        "mei.text should be in component_assets"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_panel_normalizes_title_to_head_slot() {
    let root = temp_root("panel-head-slot");
    let app_root = root.join("head-app");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(id = "head-app", default_scene = "home")

scene(id = "home", profile = "page")

world(id = "home_world", resources = [])

frame(layout = flex(direction = "column"))

frame.add_panel(
    id = "p",
    area = "auto",
    title = "标题",
    blocks = [
        text("正文", area = "auto"),
    ],
)
"#,
    );
    write_file(
        &root.join("_components/mei/manifest.json"),
        r#"
{
  "components": {
    "mei.text": { "tag": "mei-text", "script": "text.js" }
  }
}
"#,
    );
    write_file(&root.join("_components/mei/text.js"), "// stub");

    let compiled = compile_app_from_root(&root, &app_root).expect("compile panel head");
    let contract = compiled.scene_contract.expect("scene contract");
    let panel = contract.panels.iter().find(|p| p.id == "p").expect("panel");
    assert!(crate::panel_resolved_has_head(panel));
    let layout = panel.layout.as_ref().expect("layout");
    assert!(layout
        .areas
        .as_ref()
        .is_some_and(|rows| rows.iter().flatten().any(|cell| cell == "head")));
    assert!(panel.blocks.iter().any(|node| matches!(
        node,
        crate::UiNodeDecl::Block(block)
            if block.area.as_deref() == Some("head")
                && block.props.get("content").and_then(|v| v.as_str()) == Some("标题")
    )));
    assert!(panel.blocks.iter().any(|node| matches!(
        node,
        crate::UiNodeDecl::Block(block) if block.area.as_deref() == Some("body")
    )));
    let _ = fs::remove_dir_all(&root);
}
