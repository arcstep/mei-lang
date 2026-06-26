use std::fs;

use super::{compile_app_from_root, temp_root, write_file};

#[test]
fn compile_scene_first_analytics_board_from_target_bindings() {
    let root = temp_root("scene-first-analytics-board");
    let app_root = root.join("scene-first-analytics-board");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(id = "scene-first-analytics-board", scene = scene_ref(scene_file = "home.mei", scene_id = "home"))
app_add_scene(scene = scene_ref(scene_file = "board.mei", scene_id = "analytics_board"))
"#,
    );
    write_file(
        &app_root.join("home.mei"),
        r#"
scene(id = "home", profile = "page")
world(resources = [])
world.add_metric(
    ds.scalar_map(
        id = "sales_total",
        label = "销售总额",
        values = {"value": 1},
        schema = [
            ds.column("category", "string"),
            ds.column("value", "number"),
        ],
        explain = [
            ds.composition(id = "composition", label = "分类构成", by = "category"),
            ds.detail(id = "detail", label = "明细", fields = ["category", "value"]),
        ],
    ),
)
frame()
frame.add_panel(
    id = "launch",
    area = "auto",
    props = {
        "popup": link(
            type = "popup",
            projection = "overlay",
            scene = scene_ref(scene_file = "board.mei", scene_id = "analytics_board"),
            params = {
                "metric": metric_ref("sales_total"),
                "rowset_dataset_id": "sales_metrics",
            },
        ),
    },
    blocks = [],
)
"#,
    );
    write_file(
        &app_root.join("board.mei"),
        r#"
scene(
    id = "analytics_board",
    profile = "cockpit",
    params = {
        "metric": param(type = "metric", required = True),
        "rowset_dataset_id": param(type = "string"),
    },
    bindings = {
        "filter_schema": {
            "rowset_dataset_id": param_ref("rowset_dataset_id"),
            "fields": [filter_field(key = "category", label = "分类", column = "category")],
        },
        "chart": [
            build_view(
                kind = "chart",
                source = explain_ref("composition"),
                chart_kind = "column",
            ),
        ],
        "detail": build_view(
            kind = "table",
            source = explain_ref("detail"),
        ),
    },
)
world(resources = [])
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
        panel(
            id = "detail",
            area = "detail",
            slot = panel_slot(kind = "slots", accepts = ["data_table"], required = True),
            blocks = [],
        ),
    ],
)
"#,
    );
    let compiled =
        compile_app_from_root(&root, &app_root).expect("compile scene-first analytics board");
    let contract = compiled.scene_contract.expect("contract");
    let popup = contract.panels[0].props.get("popup").expect("popup");
    assert_eq!(
        popup
            .get("filter_schema")
            .and_then(|value| value.get("rowset_dataset_id"))
            .and_then(|value| value.as_str()),
        Some("sales_metrics")
    );
    let slots = popup
        .get("projection_slots")
        .and_then(|value| value.as_array())
        .expect("projection slots");
    assert!(
        slots.iter().any(|slot| {
            slot.get("layout_zone").and_then(|value| value.as_str()) == Some("chart")
        }),
        "expected chart zone slot, got {:?}",
        slots
    );
    assert!(
        slots.iter().any(|slot| {
            slot.get("layout_zone").and_then(|value| value.as_str()) == Some("detail")
        }),
        "expected detail zone slot, got {:?}",
        slots
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_scene_first_list_preview_board_from_target_bindings() {
    let root = temp_root("scene-first-list-preview-board");
    let app_root = root.join("scene-first-list-preview-board");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(id = "scene-first-list-preview-board", scene = scene_ref(scene_file = "home.mei", scene_id = "home"))
app_add_scene(scene = scene_ref(scene_file = "board.mei", scene_id = "list_preview_board"))
"#,
    );
    write_file(
        &app_root.join("home.mei"),
        r#"
scene(id = "home", profile = "page")
world(resources = [])
world.add_metric(
    ds.scalar_map(
        id = "issue_total",
        label = "问题总数",
        values = {"value": 1},
        schema = [
            ds.column("status", "string"),
            ds.column("value", "number"),
        ],
        explain = [
            ds.detail(id = "detail", label = "问题明细", fields = ["status", "value"]),
        ],
    ),
)
frame()
frame.add_panel(
    id = "launch",
    area = "auto",
    props = {
        "popup": link(
            type = "popup",
            projection = "overlay",
            scene = scene_ref(scene_file = "board.mei", scene_id = "list_preview_board"),
            params = {
                "metric": metric_ref("issue_total"),
                "rowset_dataset_id": "warning_list",
            },
        ),
    },
    blocks = [],
)
"#,
    );
    write_file(
        &app_root.join("board.mei"),
        r#"
scene(
    id = "list_preview_board",
    profile = "cockpit",
    params = {
        "metric": param(type = "metric", required = True),
        "rowset_dataset_id": param(type = "string"),
    },
    bindings = {
        "filter_schema": {
            "rowset_dataset_id": param_ref("rowset_dataset_id"),
            "fields": [filter_field(key = "status", label = "状态", column = "status")],
        },
        "list": build_view(
            kind = "table",
            source = explain_ref("detail"),
        ),
        "preview": build_view(
            kind = "summary",
            source = explain_ref("detail"),
        ),
    },
)
world(resources = [])
frame(
    layout = grid(
        columns = ["minmax(180px, 1fr)", "minmax(0, 2.2fr)", "minmax(220px, 1.1fr)"],
        rows = ["minmax(0, 1fr)"],
        areas = [["filter", "list", "preview"]],
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
    id = "list",
    area = "list",
    slot = panel_slot(kind = "slots", accepts = ["data_table"], required = True),
    blocks = [],
)
frame.add_panel(
    id = "preview",
    area = "preview",
    slot = panel_slot(kind = "row_preview", accepts = ["summary"], selection_from = "list"),
    blocks = [],
)
"#,
    );
    let compiled =
        compile_app_from_root(&root, &app_root).expect("compile scene-first list preview board");
    let contract = compiled.scene_contract.expect("contract");
    let popup = contract.panels[0].props.get("popup").expect("popup");
    assert_eq!(
        popup
            .get("filter_schema")
            .and_then(|value| value.get("rowset_dataset_id"))
            .and_then(|value| value.as_str()),
        Some("warning_list")
    );
    let slots = popup
        .get("projection_slots")
        .and_then(|value| value.as_array())
        .expect("projection slots");
    assert!(
        slots.iter().any(|slot| {
            slot.get("layout_zone").and_then(|value| value.as_str()) == Some("list")
        }),
        "expected list zone slot, got {:?}",
        slots
    );
    assert!(
        slots.iter().any(|slot| {
            slot.get("layout_zone").and_then(|value| value.as_str()) == Some("preview")
        }),
        "expected preview zone slot, got {:?}",
        slots
    );
    let _ = fs::remove_dir_all(&root);
}

