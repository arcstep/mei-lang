use std::fs;

use super::{compile_app_from_root, temp_root, write_file};

#[test]
fn compile_supports_nested_panels() {
    let root = temp_root("nested-panels");
    let app_root = root.join("nested");
    write_file(
        &root.join(".stock/components/manifest.json"),
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
        &root.join(".stock/components/manifest.json"),
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
fn compile_merges_theme_and_scene_shared_context() {
    let root = temp_root("shared-context");
    let app_root = root.join("shared-app");
    write_file(
        &app_root.join("main.mei"),
        r##"
app(id = "shared-app", default_scene = "home")

scene(
    id = "home",
    profile = "cockpit",
    theme = "cockpit",
    shared = {
        "layout": {"rail_width": 520},
        "table": {"preview_chars": 18},
    },
)

world(resources = [])

theme(
    id = "cockpit",
    shared = {
        "layout": {"rail_width": 480, "header_height": 72},
        "table": {"preview_chars": 30},
    },
    components = {
        "dataset_table": {
            "cell_preview_max_chars": shared_ref("table.preview_chars"),
        },
    },
)

frame(layout = flex(direction = "column"))

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
        &root.join(".stock/components/manifest.json"),
        r#"
{
  "components": {
    "markdown": { "tag": "mei-doc-markdown", "script": "doc-markdown.js" }
  }
}
"#,
    );

    let compiled = compile_app_from_root(&root, &app_root).expect("compile shared app");
    let contract = compiled.scene_contract.expect("scene contract");
    assert_eq!(
        contract.themes[0]
            .shared
            .get("layout")
            .and_then(|value| value.get("header_height"))
            .and_then(|value| value.as_i64()),
        Some(72)
    );
    assert_eq!(
        contract
            .scene
            .shared
            .get("layout")
            .and_then(|value| value.get("rail_width"))
            .and_then(|value| value.as_i64()),
        Some(520)
    );
    assert_eq!(
        contract
            .shared
            .get("layout")
            .and_then(|value| value.get("rail_width"))
            .and_then(|value| value.as_i64()),
        Some(520)
    );
    assert_eq!(
        contract
            .shared
            .get("layout")
            .and_then(|value| value.get("header_height"))
            .and_then(|value| value.as_i64()),
        Some(72)
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_rejects_non_object_or_ref_shared_context_values() {
    let root = temp_root("invalid-shared-context");
    let app_root = root.join("invalid-shared-app");
    write_file(
        &app_root.join("main.mei"),
        r##"
app(id = "invalid-shared-app", default_scene = "home")

scene(
    id = "home",
    profile = "page",
    shared = [],
)

world(resources = [])

theme(
    id = "page",
    shared = {
        "bad": dataset_ref("orders"),
    },
)

frame(layout = flex(direction = "column"))
"##,
    );

    let compiled = compile_app_from_root(&root, &app_root).expect("compile invalid shared app");
    assert!(compiled
        .diagnostics
        .iter()
        .any(|diag| diag.code == "invalid_shared_context_value"));
    let contract = compiled.scene_contract.expect("scene contract");
    assert_eq!(contract.scene.shared, serde_json::json!({}));
    assert_eq!(
        contract.themes[0].shared.get("bad"),
        Some(&serde_json::Value::Null)
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
        &root.join(".stock/components/mei/manifest.json"),
        r#"
{
  "components": {
    "mei.text": { "tag": "mei-text", "script": "text.js" }
  }
}
"#,
    );
    write_file(&root.join(".stock/components/mei/text.js"), "// stub");

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
        crate::UiTreeNode::Block(block)
            if block.area.as_deref() == Some("head")
                && block.props.get("content").and_then(|v| v.as_str()) == Some("标题")
    )));
    assert!(panel.blocks.iter().any(|node| matches!(
        node,
        crate::UiTreeNode::Block(block) if block.area.as_deref() == Some("body")
    )));
    let _ = fs::remove_dir_all(&root);
}
