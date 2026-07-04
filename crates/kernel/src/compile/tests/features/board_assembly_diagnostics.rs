use super::{
    compile_app_from_root, compile_app_from_root_with_options, temp_root, write_file,
    CompileOptions,
};

#[test]
fn compile_board_assembly_rejects_missing_data_table_zone() {
    let source_root = temp_root("reject-scene-shell-zone");
    let app_root = source_root.join("demo");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(
    id = "demo",
    default_scene = "home",
)

scene(id = "home", profile = "page")

scene.set_world(
    resources = [
        resource(
            id = "sales_metrics",
            kind = "dataset",
            source = ds.csv(path = "data/sales_metrics.csv"),
        ),
    ],
)

rows = ds.data_ref("sales_metrics")

world.add_dataset_view(
    id = "metric_rows",
    rowset = rows,
    schema = [
        ds.column("category", "string"),
        ds.column("amount", "number"),
    ],
    metrics = [
        ds.dataframe(
            id = "detail",
            schema = [
                ds.column("category", "string"),
                ds.column("amount", "number"),
            ],
            value = rows,
        ),
    ],
)

frame()

frame.add_panel(
    id = "card",
    title = "Bad",
    blocks = [
        component(
            "mei-card",
            area = "auto",
            props = {
                "title": "Bad",
                "value": 1,
                "popup": link(
                    type = "popup",
                    projection = "overlay",
                    scene = scene_ref(scene_id = "broken_board", scene_file = "shell.mei"),
                    params = {"metric": metric_ref("detail")},
                ),
            },
        ),
    ],
)
"#,
    );
    write_file(
        &app_root.join("data/sales_metrics.csv"),
        "category,amount\nA,1\nB,2\n",
    );
    write_file(
        &app_root.join("shell.mei"),
        r#"
scene(
    id = "broken_board",
    profile = "cockpit",
    params = {
        "metric": param(type = "metric", required = True),
    },
    bindings = {
        "filter_schema": {"fields": []},
        "chart": [
            build_view(kind = "chart", source = explain_ref("composition_by_category"), chart_kind = "column"),
        ],
    },
    local_nav = {
        "kind": "analytics_drilldown_board",
        "scene_id": "broken_board",
        "overlay_size": "large",
    },
)
frame(
    layout = grid(
        columns = ["minmax(180px, 1fr)", "minmax(0, 5fr)"],
        rows = ["minmax(0, 1fr)"],
        areas = [["filter", "main"]],
        gap = "12px",
        padding = "12px",
    ),
)
frame.add_panel(
    id = "filter",
    area = "filter",
    slot = panel_slot(kind = "filter", source = "filter_schema"),
    blocks = [],
)
frame.add_panel(
    id = "main",
    area = "main",
    layout = grid(
        columns = ["1fr"],
        rows = ["auto", "minmax(0, 1fr)"],
        areas = [["chart"], ["detail"]],
        gap = "12px",
    ),
    slot = panel_slot(kind = "container"),
    blocks = [
        panel(
            id = "chart",
            area = "chart",
            slot = panel_slot(kind = "slots", accepts = ["chart"], max = 3),
            blocks = [],
        ),
    ],
)
"#,
    );
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("shell.mei".to_string()),
            ..Default::default()
        },
    )
    .expect("compile broken shell should finish with diagnostics");
    let has_board_issue = compiled.diagnostics.iter().any(|d| {
        matches!(
            d.code.as_str(),
            "scene_shell_zone_missing"
                | "board_assembly_missing_detail"
                | "analytics_projection_missing_detail"
                | "layout_eval_degenerate_rows"
        )
    });
    assert!(
        has_board_issue,
        "broken analytics shell should surface layout/assembly issues, got: {:?}",
        compiled.diagnostics
    );
}

#[test]
fn compile_rejects_explain_chart_kind_in_composition() {
    let source_root = temp_root("reject-explain-chart-kind");
    let app_root = source_root.join("demo");
    write_file(
        &app_root.join("main.mei"),
        r#"
BAD = ds.composition(id = "c", by = "category", chart_kind = "bar")

app(
    id = "demo",
    default_scene = "home",
)

scene(
    id = "home",
    profile = "page",
)

world()
frame()
"#,
    );

    let result = compile_app_from_root(&source_root, &app_root);
    assert!(
        result.is_err(),
        "expected compile to fail for explain chart_kind ban"
    );
    let message = result
        .err()
        .map(|error| error.to_string())
        .unwrap_or_default();
    assert!(
        message.contains("chart_kind") || message.contains("unexpected named argument"),
        "expected error to mention chart_kind rejection, got: {message}"
    );

    let _ = std::fs::remove_dir_all(&source_root);
}
