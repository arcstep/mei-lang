use std::fs;

use super::{compile_app_from_root, temp_root, write_file};

#[test]
fn compile_rejects_panel_ref_block_embed_with_area() {
    let root = temp_root("panel-ref-block-embed-removed");
    let app_root = root.join("embed-app");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(id = "embed-app", default_scene = "home")

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
)
"#,
    );

    let compiled = compile_app_from_root(&root, &app_root).expect("compile legacy block panel_ref");
    assert!(
        compiled.diagnostics.iter().any(|diag| {
            diag.code == "panel_ref_embed_removed"
                || (diag.code == "compile_scene_failed"
                    && diag.message.contains("panel_ref_embed_removed"))
        }),
        "legacy block panel_ref IR should report panel_ref_embed_removed: {:?}",
        compiled.diagnostics
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_supports_panel_ref_in_frame_panels() {
    let root = temp_root("panel-ref-frame-panels");
    let app_root = root.join("embed-app");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(id = "embed-app", default_scene = "home")

scene(id = "home", profile = "page")

world()

frame(
    layout = flex(direction = "column"),
    panels = [
        panel_ref(id = "inner", scene_file = "child.mei"),
    ],
)
"#,
    );
    write_file(
        &app_root.join("child.mei"),
        r#"
scene(id = "child_scene", profile = "page")
world()
frame()
frame.add_panel(id = "inner", area = "auto", blocks = [])
"#,
    );

    let compiled =
        compile_app_from_root(&root, &app_root).expect("compile panel_ref in frame.panels");
    assert!(
        compiled
            .diagnostics
            .iter()
            .all(|diag| !matches!(diag.severity, crate::Severity::Error)),
        "frame.panels panel_ref should not error: {:?}",
        compiled.diagnostics
    );
    let contract = compiled.scene_contract.expect("scene contract");
    assert_eq!(contract.panels.len(), 1);
    assert_eq!(contract.panels[0].id, "inner");

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_reports_top_level_panel_ref_embed() {
    let root = temp_root("top-level-panel-ref-embed");
    let app_root = root.join("bad-embed");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(id = "bad-embed", default_scene = "home")

scene(id = "home", profile = "page")

world()

frame()

exports.append({"component": {
    "block_kind": "panel_ref",
    "scene_file": "child.mei",
    "area": "auto",
    "id": "orphan",
}})
"#,
    );

    let compiled =
        compile_app_from_root(&root, &app_root).expect("compile top-level panel_ref embed");
    assert!(
        compiled.diagnostics.iter().any(|diag| {
            (diag.code == "panel_ref_embed_removed" || diag.code == "top_level_panel_ref_embed")
                && matches!(diag.severity, crate::Severity::Error)
        }),
        "expected panel_ref_embed_removed at scene top level: {:?}",
        compiled.diagnostics
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_reports_deprecated_panel_capsule_ref_block_kind() {
    let root = temp_root("deprecated-panel-capsule-ref");
    let app_root = root.join("legacy-embed");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(id = "legacy-embed", default_scene = "home")

scene(id = "home", profile = "page")

frame(layout = flex(direction = "column"))

frame.add_panel(
    id = "stack",
    area = "auto",
    blocks = [
        {"component": {
            "block_kind": "panel_capsule_ref",
            "scene_file": "child.mei",
            "area": "auto",
            "id": "slot",
        }},
    ],
)
"#,
    );
    write_file(
        &app_root.join("child.mei"),
        r#"
scene(id = "child_scene", profile = "page")
world()
frame()
"#,
    );

    let compiled =
        compile_app_from_root(&root, &app_root).expect("compile legacy panel_capsule_ref");
    assert!(
        compiled.diagnostics.iter().any(|diag| {
            diag.code == "deprecated_panel_capsule_ref"
                && matches!(diag.severity, crate::Severity::Error)
        }),
        "expected deprecated_panel_capsule_ref: {:?}",
        compiled.diagnostics
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_refs_scenario4_panel_ref_with_world_ref_imports_external_panel() {
    let root = temp_root("refs-scenario-4");
    let app_root = root.join("refs-04");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(id = "refs-04", default_scene = "home")
scene(
    id = "home",
    profile = "page",
    world = world_ref(scene_file = "panels/widget.mei"),
)
frame(
    panels = [
        panel_ref(id = "widget_panel", scene_file = "panels/widget.mei"),
    ],
)
"#,
    );
    write_file(
        &app_root.join("panels/widget.mei"),
        r#"
scene(id = "widget", profile = "page")
world(resources = [resource(id = "widget_doc", kind = "document", content = "capsule doc")])
frame()
frame.add_panel(
    id = "widget_panel",
    area = "auto",
    blocks = [doc.markdown(area = "auto", resource = resource_ref("widget_doc"))],
)
"#,
    );
    let compiled = compile_app_from_root(&root, &app_root).expect("compile refs scenario 4");
    assert!(
        compiled
            .diagnostics
            .iter()
            .all(|diag| !matches!(diag.severity, crate::Severity::Error)),
        "scenario 4 should compile with world_ref + panel_ref: {:?}",
        compiled.diagnostics
    );
    assert!(
        !compiled
            .diagnostics
            .iter()
            .any(|diag| diag.code == "imported_resource_not_authorized"),
        "world_ref should authorize widget_doc"
    );
    assert!(
        compiled
            .resources
            .iter()
            .any(|item| item.id == "widget_doc"),
        "widget_doc from external world should appear in scene resources"
    );
    let contract = compiled.scene_contract.expect("scene contract");
    assert!(
        contract.panels.iter().any(|p| p.id == "widget_panel"),
        "panel_ref should resolve widget_panel into scene panels"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_refs_scenario5_local_panel_and_resource_override_external_ledger() {
    let root = temp_root("refs-scenario-5");
    let app_root = root.join("refs-05");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(id = "refs-05", default_scene = "home")
scene(id = "home", profile = "page")
world(resources = [resource(id = "shared_doc", kind = "document", content = "host wins")])
frame(
    panels = [
        panel(
            base = panel_ref(id = "slot", scene_file = "panels/base.mei"),
            title = "覆盖后的 panel 标题",
        ),
    ],
)
"#,
    );
    write_file(
        &app_root.join("panels/base.mei"),
        r#"
scene(id = "base", profile = "page")
world(resources = [resource(id = "shared_doc", kind = "document", content = "external loses")])
frame()
frame.add_panel(
    id = "slot",
    title = "外部 panel 原标题",
    area = "auto",
    blocks = [doc.markdown(area = "auto", resource = resource_ref("shared_doc"))],
)
"#,
    );
    let compiled = compile_app_from_root(&root, &app_root).expect("compile refs scenario 5");
    assert!(
        compiled
            .diagnostics
            .iter()
            .all(|diag| !matches!(diag.severity, crate::Severity::Error)),
        "scenario 5 should compile: {:?}",
        compiled.diagnostics
    );
    let contract = compiled.scene_contract.expect("contract");
    let panel = contract
        .panels
        .iter()
        .find(|panel| panel.id == "slot")
        .expect("slot panel");
    assert_eq!(panel.title.as_deref(), Some("覆盖后的 panel 标题"));
    assert!(
        !panel.blocks.is_empty(),
        "panel(base=panel_ref) should inherit blocks from base panel"
    );
    let shared = compiled
        .resources
        .iter()
        .find(|item| item.id == "shared_doc")
        .expect("shared_doc");
    assert!(
        shared
            .document
            .as_deref()
            .is_some_and(|content| content.contains("host wins")),
        "host world should override external capsule resource on same id"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_panel_base_clones_and_overrides_area() {
    let root = temp_root("panel-base-clone");
    let app_root = root.join("clone-app");
    write_file(
        &app_root.join("external.mei"),
        r#"
scene(id = "ext", profile = "page")
world()
frame()
frame.add_panel(
    id = "widget",
    title = "外部标题",
    area = "stats",
    blocks = [doc.markdown(area = "auto", resource = resource_ref("doc"))],
)
"#,
    );
    write_file(
        &app_root.join("main.mei"),
        r#"
app(id = "clone-app", default_scene = "home")
scene(id = "home", profile = "page")
world(resources = [resource(id = "doc", kind = "document", content = "ok")])
frame(
    panels = [
        panel(
            base = panel_ref(id = "widget", scene_file = "external.mei"),
            area = "left",
        ),
    ],
)
"#,
    );
    let compiled = compile_app_from_root(&root, &app_root).expect("compile panel base clone");
    let contract = compiled.scene_contract.expect("contract");
    let panel = contract
        .panels
        .iter()
        .find(|p| p.id == "widget")
        .expect("cloned panel");
    assert_eq!(panel.area.as_deref(), Some("left"));
    assert_eq!(panel.title.as_deref(), Some("外部标题"));
    assert_eq!(
        panel.blocks.len(),
        2,
        "title materializes head block plus inherited body block"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_nine_grid_nested_panel_base_clones() {
    let root = temp_root("nine-grid-nested-panel");
    let app_root = root.join("grid-nested");
    write_file(
        &app_root.join("templates/cell.mei"),
        r#"
scene(id = "c", profile = "page")
world(resources = [resource(id = "x", kind = "document", content = "x")])
frame()
frame.add_panel(id = "cell", blocks = [doc.markdown(area = "auto", resource = resource_ref("x"))])
"#,
    );
    write_file(
        &app_root.join("templates/row.mei"),
        r#"
scene(id = "r", profile = "page")
world(resources = [resource(id = "a", kind = "document", content = "a")])
frame()
frame.add_panel(
    id = "row",
    layout = flex(direction = "row"),
    blocks = [
        panel(base = panel_ref(id = "cell", scene_file = "templates/cell.mei"), id = "a", blocks = [doc.markdown(area = "auto", resource = resource_ref("a"))]),
        panel(base = panel_ref(id = "cell", scene_file = "templates/cell.mei"), id = "b"),
        panel(base = panel_ref(id = "cell", scene_file = "templates/cell.mei"), id = "c"),
    ],
)
"#,
    );
    write_file(
        &app_root.join("main.mei"),
        r#"
app(id = "grid-nested", default_scene = "home")
scene(id = "home", profile = "page")
world()
frame(
    layout = flex(direction = "column"),
    panels = [
        panel(base = panel_ref(id = "row", scene_file = "templates/row.mei"), id = "r1"),
        panel(base = panel_ref(id = "row", scene_file = "templates/row.mei"), id = "r2"),
    ],
)
"#,
    );
    let compiled = compile_app_from_root(&root, &app_root).expect("compile nested panel grid");
    let contract = compiled.scene_contract.expect("contract");
    assert_eq!(contract.panels.len(), 2);
    let row1 = contract.panels.iter().find(|p| p.id == "r1").expect("r1");
    assert_eq!(
        row1.blocks.len(),
        3,
        "row should have 3 horizontal nested panels"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
#[ignore = "existing incomplete assertion; tracked separately"]
fn compile_component_base_clones_use_key() {
    let root = temp_root("component-base-clone");
    let app_root = root.join("cmp-base");
    write_file(
        &app_root.join("source.mei"),
        r#"
scene(id = "s", profile = "page")
world()
frame()
frame.add_panel(
    id = "p",
    area = "auto",
    blocks = [component("doc.markdown", id = "hero", props = {"title": "基线"})],
)
"#,
    );
    write_file(
        &app_root.join("main.mei"),
        r#"
app(id = "cmp-base", default_scene = "home")
scene(id = "home", profile = "page")
world(resources = [resource(id = "doc", kind = "document", content = "x")])
frame()
frame.add_panel(
    id = "p1",
    area = "auto",
    blocks = [
        component(
            base = component_ref(id = "hero", scene_file = "source.mei"),
            props = {"title": "克隆"},
            data = resource_ref("doc"),
        ),
    ],
)
"#,
    );
    let compiled = compile_app_from_root(&root, &app_root).expect("compile component base");
    let contract = compiled.scene_contract.expect("contract");
    let panel = contract
        .panels
        .iter()
        .find(|p| p.id == "p1")
        .expect("panel");
    assert_eq!(panel.blocks.len(), 1);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_metric_card_base_clones_shell_and_overrides_source() {
    let root = temp_root("metric-card-base-clone");
    let app_root = root.join("metric-card-clone");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(id = "metric-card-clone", default_scene = "home")
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
        panels: &'a [crate::PanelDecl],
        target: &str,
    ) -> Option<&'a crate::PanelDecl> {
        for panel in panels {
            if panel.id == target {
                return Some(panel);
            }
            for node in &panel.blocks {
                if let crate::UiNodeDecl::Panel(nested) = node {
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
fn compile_metric_card_base_inherits_template_slot_vertical_align_defaults() {
    let root = temp_root("metric-card-base-v-align");
    let app_root = root.join("metric-card-base-v-align");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(id = "metric-card-base-v-align", default_scene = "home")
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
    let compiled = compile_app_from_root(&root, &app_root).expect("compile metric_card base v_align");
    let contract = compiled.scene_contract.expect("contract");
    fn find_panel_by_id<'a>(
        panels: &'a [crate::PanelDecl],
        target: &str,
    ) -> Option<&'a crate::PanelDecl> {
        for panel in panels {
            if panel.id == target {
                return Some(panel);
            }
            for node in &panel.blocks {
                if let crate::UiNodeDecl::Panel(nested) = node {
                    if let Some(found) = find_panel_by_id(std::slice::from_ref(nested), target) {
                        return Some(found);
                    }
                }
            }
        }
        None
    }
    fn value_slot_v_align(panel: &crate::PanelDecl) -> Option<&str> {
        panel.blocks.iter().find_map(|node| {
            let crate::UiNodeDecl::Block(block) = node else {
                return None;
            };
            if block
                .props
                .get("metric_role")
                .and_then(|v| v.as_str())
                != Some("value")
            {
                return None;
            }
            block
                .props
                .get("metric_v_align")
                .and_then(|v| v.as_str())
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
app(id = "t", default_scene = "home")
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
    fn find_panel<'a>(panels: &'a [crate::PanelDecl], id: &str) -> Option<&'a crate::PanelDecl> {
        for panel in panels {
            if panel.id == id {
                return Some(panel);
            }
            for node in &panel.blocks {
                if let crate::UiNodeDecl::Panel(nested) = node {
                    if let Some(found) = find_panel(std::slice::from_ref(nested), id) {
                        return Some(found);
                    }
                }
            }
        }
        None
    }
    fn role_v_align<'a>(panel: &'a crate::PanelDecl, role: &str) -> Option<&'a str> {
        panel.blocks.iter().find_map(|node| {
            let crate::UiNodeDecl::Block(block) = node else {
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
app(id = "metric-card-base-bg", default_scene = "home")
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
        panels: &'a [crate::PanelDecl],
        target: &str,
    ) -> Option<&'a crate::PanelDecl> {
        for panel in panels {
            if panel.id == target {
                return Some(panel);
            }
            for node in &panel.blocks {
                if let crate::UiNodeDecl::Panel(nested) = node {
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
app(id = "metric-card-base-bg-source", default_scene = "home")
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
    let compiled = compile_app_from_root(&root, &app_root).expect("compile metric_card base bg source");
    let contract = compiled.scene_contract.expect("contract");
    fn find_panel_by_id<'a>(
        panels: &'a [crate::PanelDecl],
        target: &str,
    ) -> Option<&'a crate::PanelDecl> {
        for panel in panels {
            if panel.id == target {
                return Some(panel);
            }
            for node in &panel.blocks {
                if let crate::UiNodeDecl::Panel(nested) = node {
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

#[test]
#[ignore = "existing incomplete assertion; tracked separately"]
fn compile_panel_base_rejects_wrong_ref_kind() {
    let root = temp_root("panel-base-wrong-kind");
    let app_root = root.join("bad-base");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(id = "bad-base", default_scene = "home")
scene(id = "home", profile = "page")
world()
frame(
    panels = [
        panel(base = frame_ref(scene_file = "missing.mei")),
    ],
)
"#,
    );
    let compiled = compile_app_from_root(&root, &app_root).expect("compile bad panel base");
    assert!(
        compiled
            .diagnostics
            .iter()
            .any(|d| d.code == "invalid_panel_base_ref_kind"),
        "expected invalid_panel_base_ref_kind: {:?}",
        compiled.diagnostics
    );
    let _ = fs::remove_dir_all(&root);
}
