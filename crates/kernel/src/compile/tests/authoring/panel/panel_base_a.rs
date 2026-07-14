use std::fs;

use super::{compile_app_from_root, temp_root, write_file};

use std::fs;

use super::{compile_app_from_root, temp_root, write_file};

#[test]
fn compile_rejects_panel_ref_block_embed_with_area() {
    let root = temp_root("panel-ref-block-embed-removed");
    let app_root = root.join("embed-app");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(id = "embed-app", default_stage = "home")

scene(id = "home", profile = "page")

world()

frame(layout = flex(direction = "column"))

frame.add_panel(
    id = "stack",
    area = "auto",
    blocks = [
        {"component": {
            "block_kind": "panel_ref",
            "scene_file": "child.mei",
            "area": "auto",
            "id": "child_slot",
        }},
    ],
            ],
        ),
    ],
)
"#,
    );
    write_file(
        &app_root.join("templates/shell.mei"),
        r#"
scene(id = "shell_tpl", profile = "cockpit", theme = "cockpit")
world(resources = [])
frame()
frame.add_panel(
    id = "shell",
    area = "auto",
    show_heading = False,
    chrome = "bare",
    variant = "container",
    props = {
        "width": "120px",
        "height": "100px",
        "box_sizing": "border-box",
        "__mei_metric_card": True,
        "__mei_metric_template": "stack",
    },
    layout = layout_metric_stack(),
    blocks = [
        label("模板", area = "label"),
        value("--", area = "value"),
        unit("", area = "unit"),
    ],
)
"#,
    );
    let compiled = compile_app_from_root(&root, &app_root).expect("compile metric_card base");
    assert!(
        compiled
            .diagnostics
            .iter()
            .all(|diag| !matches!(diag.severity, crate::Severity::Error)),
        "metric_card(base=...) should compile: {:?}",
        compiled.diagnostics
    );
    let contract = compiled.scene_contract.expect("contract");
    fn find_panel_by_id<'a>(
        panels: &'a [crate::UiNodeDecl],
        target: &str,
    ) -> Option<&'a crate::UiNodeDecl> {
        for panel in panels {
            if panel.id == target {
                return Some(panel);
            }
            for node in &panel.blocks {
                if let crate::UiTreeNode::Panel(nested) = node {
                    if let Some(found) = find_panel_by_id(std::slice::from_ref(nested), target) {
                        return Some(found);
                    }
                }
            }
        }
        None
    }
    let live = find_panel_by_id(&contract.panels, "live").expect("live metric panel");
    assert!(
        !live.blocks.is_empty(),
        "metric_card(base=...) should inherit or rebuild blocks"
    );
    let width = live
        .props
        .get("width")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert!(
        width.contains("120"),
        "metric_card(base=...) should inherit shell width from template, got {width}"
    );
    let areas = live
        .layout
        .as_ref()
        .and_then(|layout| layout.areas.as_ref())
        .expect("metric_card(base=...) should inherit template layout");
    assert!(
        areas.iter().flatten().any(|cell| cell == "label"),
        "template stack layout areas should survive base clone merge, got {areas:?}"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_metric_card_metric_ref_uses_mei_text_slots_instead_of_legacy_tile() {
    let root = temp_root("metric-card-runtime-ref");
    let app_root = root.join("metric-card-runtime-ref");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(id = "metric-card-runtime-ref", default_stage = "home")
scene(id = "home", profile = "cockpit", theme = "cockpit")
world(resources = [])
world.add_metric(
    ds.scalar_map(
        id = "sales_total",
        label = "案件总数",
        unit = "件",
        values = {"value": 42},
        schema = [ds.column("value", "number", unit = "件")],
    ),
)
frame(
    panels = [
        panel(
            id = "row",
            area = "auto",
            blocks = [
                metric_card(
                    id = "live",
                    template = "stack",
                    source = metric_ref("sales_total"),
                ),
            ],
        ),
    ],
)
"#,
    );
    let compiled =
        compile_app_from_root(&root, &app_root).expect("compile metric_card metric_ref main line");
    assert!(
        compiled
            .diagnostics
            .iter()
            .all(|diag| !matches!(diag.severity, crate::Severity::Error)),
        "metric_card(source=metric_ref(...)) should compile without errors: {:?}",
        compiled.diagnostics
    );
    let contract = compiled.scene_contract.expect("contract");
    fn find_panel_by_id<'a>(
        panels: &'a [crate::UiNodeDecl],
        target: &str,
    ) -> Option<&'a crate::UiNodeDecl> {
        for panel in panels {
            if panel.id == target {
                return Some(panel);
            }
            for node in &panel.blocks {
                if let crate::UiTreeNode::Panel(nested) = node {
                    if let Some(found) = find_panel_by_id(std::slice::from_ref(nested), target) {
                        return Some(found);
                    }
                }
            }
        }
        None
    }
    let live = find_panel_by_id(&contract.panels, "live").expect("live metric panel");
    let live_blocks = live
        .blocks
        .iter()
        .filter_map(|node| match node {
            crate::UiTreeNode::Block(block) => Some(block),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(
        live_blocks.iter().all(|block| block.use_key == "mei.text"),
        "metric_card(source=metric_ref(...)) should emit mei.text slots, got {:?}",
        live_blocks
            .iter()
            .map(|block| &block.use_key)
            .collect::<Vec<_>>()
    );
    let value_block = live_blocks
        .iter()
        .find(|block| block.area.as_deref() == Some("value"))
        .expect("value slot block");
    let content = value_block
        .props
        .get("content")
        .expect("value slot metric content");
    assert!(
        content.is_object(),
        "value slot content should preserve metric object, got {content:?}"
    );
    let _ = fs::remove_dir_all(&root);
}

