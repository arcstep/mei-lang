use std::fs;

use super::{compile_app_from_root, temp_root, write_file};

#[test]
fn compile_link_params_passthrough() {
    let root = temp_root("link-params-passthrough");
    let app_root = root.join("link-params-passthrough");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(id = "link-params-passthrough", default_stage = "home")
scene(id = "home", profile = "page")
world(resources = [])
frame()
frame.add_panel(
    id = "launch",
    area = "auto",
    props = {
        "popup": link(
            type = "popup",
            projection = "overlay",
            scene = scene_ref(scene_file = "detail.mei", scene_id = "detail"),
            params = {
                "entry": "overview",
                "tab": "chart",
            },
        ),
    },
    blocks = [],
)
"#,
    );
    write_file(
        &app_root.join("detail.mei"),
        r#"
scene(
    id = "detail",
    profile = "page",
    params = {
        "entry": param(type = "string"),
        "tab": param(type = "string"),
    },
)
world(resources = [])
frame()
"#,
    );
    let compiled =
        compile_app_from_root(&root, &app_root).expect("compile link params passthrough");
    let contract = compiled.scene_contract.expect("contract");
    let popup = contract.panels[0].props.get("popup").expect("popup");
    assert_eq!(
        popup
            .get("params")
            .and_then(|value| value.get("entry"))
            .and_then(|value| value.as_str()),
        Some("overview")
    );
    assert_eq!(
        popup
            .get("params")
            .and_then(|value| value.get("tab"))
            .and_then(|value| value.as_str()),
        Some("chart")
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_generic_drilldown_link_rowset_contract() {
    let root = temp_root("generic-drilldown-rowset");
    let app_root = root.join("generic-drilldown-rowset");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(id = "generic-drilldown-rowset", default_stage = "home")
scene(id = "home", profile = "page")
world(resources = [])
world.add_metric(
    ds.scalar_map(
        id = "sales_total",
        label = "销售总额",
        values = {"value": 1},
        schema = [ds.column("value", "number")],
        explain = [
            ds.detail(label = "明细", fields = ["value"]),
        ],
    ),
)
frame()
frame.add_panel(
    id = "launch",
    area = "auto",
    props = {
        "popup": generic_drilldown_link(
            scene = scene_ref(scene_file = "board.mei", scene_id = "generic_drilldown_board"),
            metric = metric_ref("sales_total"),
            default_slot = 1,
            rowset_dataset_id = "sales_metrics",
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
    id = "generic_drilldown_board",
    profile = "cockpit",
    params = {
        "metric": param(type = "metric", required = True),
        "rowset_dataset_id": param(type = "string"),
    },
    local_nav = {
        "include_hero": True,
    },
)
world(resources = [])
frame(
    layout = grid(
        columns = ["1fr"],
        rows = ["auto", "minmax(0, 1fr)"],
        areas = [["tabs"], ["content"]],
    ),
)
frame.add_panel(
    id = "tabs",
    area = "tabs",
    slot = panel_slot(kind = "tab_bar"),
    blocks = [],
)
frame.add_panel(
    id = "content",
    area = "content",
    slot = panel_slot(kind = "tab_content"),
    blocks = [],
)
"#,
    );
    let compiled =
        compile_app_from_root(&root, &app_root).expect("compile generic drilldown rowset");
    let contract = compiled.scene_contract.expect("contract");
    let popup = contract.panels[0].props.get("popup").expect("popup");
    assert_eq!(
        popup
            .get("params")
            .and_then(|value| value.get("rowset_dataset_id"))
            .and_then(|value| value.as_str()),
        Some("sales_metrics")
    );
    assert_eq!(
        popup
            .get("filter_schema")
            .and_then(|value| value.get("rowset_dataset_id"))
            .and_then(|value| value.as_str()),
        Some("sales_metrics")
    );
    assert!(
        popup
            .get("projection_slots")
            .and_then(|value| value.as_array())
            .is_some_and(|slots| !slots.is_empty()),
        "expected lowered projection_slots, got {:?}",
        popup
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_generic_scene_link_without_tabs_uses_scene_params() {
    let root = temp_root("generic-scene-link-without-tabs");
    let app_root = root.join("generic-scene-link-without-tabs");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(id = "generic-scene-link-without-tabs", default_stage = "home")
scene(id = "home", profile = "page")
world(resources = [])
world.add_metric(
    ds.scalar_map(
        id = "sales_total",
        label = "销售总额",
        values = {"value": 1},
        schema = [ds.column("value", "number")],
        explain = [
            ds.detail(label = "明细", fields = ["value"]),
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
            scene = scene_ref(scene_file = "board.mei", scene_id = "generic_drilldown_board"),
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
    id = "generic_drilldown_board",
    profile = "cockpit",
    params = {
        "metric": param(type = "metric", required = True),
        "rowset_dataset_id": param(type = "string"),
    },
    local_nav = {
        "include_hero": True,
    },
)
world(resources = [])
frame(
    layout = grid(
        columns = ["1fr"],
        rows = ["auto", "minmax(0, 1fr)"],
        areas = [["tabs"], ["content"]],
    ),
)
frame.add_panel(
    id = "tabs",
    area = "tabs",
    slot = panel_slot(kind = "tab_bar"),
    blocks = [],
)
frame.add_panel(
    id = "content",
    area = "content",
    slot = panel_slot(kind = "tab_content"),
    blocks = [],
)
"#,
    );
    let compiled =
        compile_app_from_root(&root, &app_root).expect("compile generic scene link without tabs");
    let contract = compiled.scene_contract.expect("contract");
    let popup = contract.panels[0].props.get("popup").expect("popup");
    assert_eq!(
        popup.get("layout_mode").and_then(|value| value.as_str()),
        Some("generic_tabs")
    );
    assert_eq!(
        popup
            .get("filter_schema")
            .and_then(|value| value.get("rowset_dataset_id"))
            .and_then(|value| value.as_str()),
        Some("sales_metrics")
    );
    assert!(
        popup
            .get("projection_slots")
            .and_then(|value| value.as_array())
            .is_some_and(|slots| !slots.is_empty()),
        "expected projection slots from scene+params generic path, got {:?}",
        popup
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_scene_link_reports_missing_required_param() {
    let root = temp_root("scene-link-missing-required-param");
    let app_root = root.join("scene-link-missing-required-param");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(id = "scene-link-missing-required-param", default_stage = "home")
scene(id = "home", profile = "page")
world(resources = [])
frame()
frame.add_panel(
    id = "launch",
    area = "auto",
    props = {
        "popup": link(
            type = "popup",
            projection = "overlay",
            scene = scene_ref(scene_file = "board.mei", scene_id = "generic_drilldown_board"),
            params = {
                "entry": "detail",
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
    id = "generic_drilldown_board",
    profile = "cockpit",
    params = {
        "metric": param(type = "metric", required = True),
    },
)
world(resources = [])
frame()
"#,
    );
    let compiled =
        compile_app_from_root(&root, &app_root).expect("compile missing required scene link param");
    assert!(
        compiled
            .diagnostics
            .iter()
            .any(|diag| diag.code == "scene_link_param_missing"),
        "expected scene_link_param_missing diagnostic, got {:?}",
        compiled.diagnostics
    );
    let _ = fs::remove_dir_all(&root);
}

