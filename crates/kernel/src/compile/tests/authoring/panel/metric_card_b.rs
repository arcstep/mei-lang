use std::fs;

use super::{compile_app_from_root, temp_root, write_file};

#[test]
fn compile_metric_card_base_inherits_template_slot_vertical_align_defaults() {
    let root = temp_root("metric-card-base-v-align");
    let app_root = root.join("metric-card-base-v-align");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(id = "metric-card-base-v-align", default_stage = "home")
scene(id = "home", profile = "cockpit", theme = "cockpit")
world(resources = [])
frame(
    panels = [
        panel(
            id = "row",
            area = "auto",
            show_heading = False,
            blocks = [
                metric_card(
                    base = metric_card_ref(id = "shell", scene_file = "templates/shell.mei"),
                    id = "live",
                    source = {"label": "现场", "value": "42", "unit": "项"},
                ),
                metric_card(
                    base = metric_card_ref(id = "shell", scene_file = "templates/shell.mei"),
                    id = "override",
                    value_vertical_align = "end",
                    source = {"label": "覆写", "value": "9", "unit": "项"},
                ),
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
        "__mei_metric_card": True,
        "__mei_metric_template": "stack",
        "__mei_metric_value_v_align": "top",
    },
    layout = layout_metric_stack(),
    blocks = [
        label("模板", area = "label", vertical_align = "center"),
        value("--", area = "value"),
        unit("", area = "unit"),
    ],
)
"#,
    );
    let compiled =
        compile_app_from_root(&root, &app_root).expect("compile metric_card base v_align");
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
    fn value_slot_v_align(panel: &crate::UiNodeDecl) -> Option<&str> {
        panel.blocks.iter().find_map(|node| {
            let crate::UiTreeNode::Block(block) = node else {
                return None;
            };
            if block.props.get("metric_role").and_then(|v| v.as_str()) != Some("value") {
                return None;
            }
            block.props.get("metric_v_align").and_then(|v| v.as_str())
        })
    }
    let live = find_panel_by_id(&contract.panels, "live").expect("live");
    assert_eq!(
        value_slot_v_align(live),
        Some("top"),
        "template __mei_metric_value_v_align should apply after base+source clone"
    );
    let override_panel = find_panel_by_id(&contract.panels, "override").expect("override");
    assert_eq!(
        value_slot_v_align(override_panel),
        Some("end"),
        "value_vertical_align= on metric_card should override template default"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_metric_card_base_prefers_template_block_vertical_align_over_props() {
    let root = temp_root("metric-card-base-block-v-align");
    let app_root = root.join("metric-card-base-block-v-align");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(id = "t", default_stage = "home")
scene(id = "home", profile = "cockpit", theme = "cockpit")
world(resources = [])
frame(
    panels = [
        panel(
            id = "row",
            show_heading = False,
            blocks = [
                metric_card(
                    base = metric_card_ref(id = "shell", scene_file = "templates/shell.mei"),
                    id = "live",
                    source = {"label": "L", "value": "V", "unit": "U"},
                ),
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
    show_heading = False,
    chrome = "bare",
    variant = "container",
    props = {
        "__mei_metric_card": True,
        "__mei_metric_template": "stack",
        "__mei_metric_label_v_align": "center",
        "__mei_metric_value_v_align": "center",
    },
    layout = layout_metric_stack(),
    blocks = [
        label("·", area = "label", vertical_align = "end"),
        value("--", area = "value", vertical_align = "top"),
        unit("", area = "unit", vertical_align = "top"),
    ],
)
"#,
    );
    let compiled = compile_app_from_root(&root, &app_root).expect("compile");
    let contract = compiled.scene_contract.expect("contract");
    fn find_panel<'a>(panels: &'a [crate::UiNodeDecl], id: &str) -> Option<&'a crate::UiNodeDecl> {
        for panel in panels {
            if panel.id == id {
                return Some(panel);
            }
            for node in &panel.blocks {
                if let crate::UiTreeNode::Panel(nested) = node {
                    if let Some(found) = find_panel(std::slice::from_ref(nested), id) {
                        return Some(found);
                    }
                }
            }
        }
        None
    }
    fn role_v_align<'a>(panel: &'a crate::UiNodeDecl, role: &str) -> Option<&'a str> {
        panel.blocks.iter().find_map(|node| {
            let crate::UiTreeNode::Block(block) = node else {
                return None;
            };
            if block.props.get("metric_role").and_then(|v| v.as_str()) != Some(role) {
                return None;
            }
            block.props.get("metric_v_align").and_then(|v| v.as_str())
        })
    }
    let live = find_panel(&contract.panels, "live").expect("live");
    assert_eq!(role_v_align(live, "label"), Some("end"));
    assert_eq!(role_v_align(live, "value"), Some("top"));
    assert_eq!(role_v_align(live, "unit"), Some("top"));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_metric_card_base_preserves_background_when_height_overridden() {
    let root = temp_root("metric-card-base-bg");
    let app_root = root.join("metric-card-base-bg");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(id = "metric-card-base-bg", default_stage = "home")
scene(id = "home", profile = "cockpit", theme = "cockpit")
world(resources = [])
frame(
    panels = [
        panel(
            id = "row",
            area = "auto",
            show_heading = False,
            blocks = [
                metric_card(
                    base = metric_card_ref(id = "shell", scene_file = "templates/shell.mei"),
                    id = "live",
                    height_px = 118,
                    source = {"label": "现场", "value": "42", "unit": "项"},
                ),
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
        "width": "114px",
        "height": "128px",
        "box_sizing": "border-box",
        "__mei_metric_card": True,
        "background": {
            "image": "url(/assets/metric-bg.svg)",
            "size": "100% 100%",
            "position": "center",
            "repeat": "no-repeat",
        },
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
    let compiled = compile_app_from_root(&root, &app_root).expect("compile metric_card base bg");
    assert!(
        compiled
            .diagnostics
            .iter()
            .all(|diag| !matches!(diag.severity, crate::Severity::Error)),
        "metric_card(base=..., height_px=...) should compile: {:?}",
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
    let bg_image = live
        .props
        .get("background")
        .and_then(|bg| bg.get("image"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert!(
        bg_image.contains("metric-bg.svg"),
        "height_px overlay must not replace template background, got {bg_image}"
    );
    assert_eq!(
        live.props.get("height").and_then(|v| v.as_str()),
        Some("118px")
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_metric_card_base_preserves_background_with_source_only() {
    let root = temp_root("metric-card-base-bg-source");
    let app_root = root.join("metric-card-base-bg-source");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(id = "metric-card-base-bg-source", default_stage = "home")
scene(id = "home", profile = "cockpit", theme = "cockpit")
world(resources = [])
frame(
    panels = [
        panel(
            id = "row",
            area = "auto",
            show_heading = False,
            blocks = [
                metric_card(
                    base = metric_card_ref(id = "shell", scene_file = "templates/shell.mei"),
                    id = "live",
                    source = {"label": "现场", "value": "42", "unit": "项"},
                ),
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
        "width": "114px",
        "height": "128px",
        "box_sizing": "border-box",
        "__mei_metric_card": True,
        "background": {
            "image": "url(/assets/metric-bg.svg)",
            "size": "100% 100%",
            "position": "center",
            "repeat": "no-repeat",
        },
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
    let compiled =
        compile_app_from_root(&root, &app_root).expect("compile metric_card base bg source");
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
    let bg_image = live
        .props
        .get("background")
        .and_then(|bg| bg.get("image"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert!(
        bg_image.contains("metric-bg.svg"),
        "metric_card(base=..., source=...) must keep template background, got {bg_image}"
    );
    assert_eq!(
        live.props.get("width").and_then(|v| v.as_str()),
        Some("114px"),
        "instance overlay must not replace template width with 100%"
    );
    let _ = fs::remove_dir_all(&root);
}

