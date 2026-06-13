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
    let host_shared = compiled
        .resources
        .iter()
        .find(|item| item.id == "shared_doc")
        .expect("host shared_doc");
    assert!(
        host_shared
            .document
            .as_deref()
            .is_some_and(|content| content.contains("host wins")),
        "host world keeps its own shared_doc"
    );
    let imported_shared = compiled
        .resources
        .iter()
        .find(|item| item.id == "panels/base.mei::shared_doc")
        .expect("namespaced imported shared_doc");
    assert!(
        imported_shared
            .document
            .as_deref()
            .is_some_and(|content| content.contains("external loses")),
        "imported panel must bind to private capsule resource, not host shared_doc"
    );
    assert_eq!(
        panel.import_scope.as_deref(),
        Some("panels/base.mei"),
        "panel_ref panel should carry import_scope"
    );
    assert!(
        compiled
            .diagnostics
            .iter()
            .any(|diag| { diag.code == "host_world_shadows_imported_panel_resource" }),
        "expected shadowing warning when host declares same local id as imported panel"
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
fn compile_metric_card_metric_ref_uses_mei_text_slots_instead_of_legacy_tile() {
    let root = temp_root("metric-card-runtime-ref");
    let app_root = root.join("metric-card-runtime-ref");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(id = "metric-card-runtime-ref", default_scene = "home")
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
    let live_blocks = live
        .blocks
        .iter()
        .filter_map(|node| match node {
            crate::UiNodeDecl::Block(block) => Some(block),
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

#[test]
fn compile_panel_base_imports_direct_world_metrics_from_multiple_sources() {
    let root = temp_root("panel-ref-import-world-metrics");
    let app_root = root.join("panel-ref-import-world-metrics");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(id = "panel-ref-import-world-metrics", default_scene = "home")
scene(id = "home", profile = "page")
world()
frame(
    panels = [
        panel(
            id = "summary_a",
            area = "auto",
            base = panel_ref(id = "summary_panel_a", scene_file = "panels/a.mei"),
        ),
        panel(
            id = "summary_b",
            area = "auto",
            base = panel_ref(id = "summary_panel_b", scene_file = "panels/b.mei"),
        ),
    ],
)
"#,
    );
    write_file(
        &app_root.join("panels/a.mei"),
        r#"
scene(id = "panel_a", profile = "page")
world()
world.add_metric(
    ds.scalar_map(
        id = "warning_models",
        label = "预警模型",
        values = {"value": 15},
        unit = "个",
        schema = [ds.column("value", "number")],
    ),
)
frame()
frame.add_panel(
    id = "summary_panel_a",
    area = "auto",
    blocks = [
        metric_card(
            id = "metric_a",
            source = metric_ref("warning_models"),
        ),
    ],
)
"#,
    );
    write_file(
        &app_root.join("panels/b.mei"),
        r#"
scene(id = "panel_b", profile = "page")
world()
world.add_metric(
    ds.scalar_map(
        id = "warning_supervision",
        label = "监督事项",
        values = {"value": 2000},
        unit = "项",
        schema = [ds.column("value", "number")],
    ),
)
frame()
frame.add_panel(
    id = "summary_panel_b",
    area = "auto",
    blocks = [
        metric_card(
            id = "metric_b",
            source = metric_ref("warning_supervision"),
        ),
    ],
)
"#,
    );

    let compiled =
        compile_app_from_root(&root, &app_root).expect("compile panel_ref imported world metrics");
    assert!(
        compiled
            .diagnostics
            .iter()
            .all(|diag| !matches!(diag.severity, crate::Severity::Error)),
        "panel(base=panel_ref(...)) should import external direct world metrics: {:?}",
        compiled.diagnostics
    );
    assert!(
        compiled
            .world_metrics
            .contains_key("panels/a.mei::warning_models"),
        "panel a metrics are namespaced"
    );
    assert!(
        compiled
            .world_metrics
            .contains_key("panels/b.mei::warning_supervision"),
        "panel b metrics are namespaced"
    );
    let resource_ids = compiled
        .resources
        .iter()
        .map(|resource| resource.id.as_str())
        .collect::<Vec<_>>();
    assert!(
        resource_ids
            .iter()
            .any(|id| id.contains("panels/a.mei::metrics")),
        "resources should contain imported world metrics dataset for panels/a.mei, got {resource_ids:?}"
    );
    assert!(
        resource_ids
            .iter()
            .any(|id| id.contains("panels/b.mei::metrics")),
        "resources should contain imported world metrics dataset for panels/b.mei, got {resource_ids:?}"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_nested_panel_refs_import_direct_world_metrics_from_grandchild_source() {
    let root = temp_root("nested-panel-ref-import-world-metrics");
    let app_root = root.join("nested-panel-ref-import-world-metrics");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(id = "nested-panel-ref-import-world-metrics", default_scene = "home")
scene(id = "home", profile = "page")
world()
frame(
    panels = [
        panel(
            id = "host",
            area = "auto",
            base = panel_ref(id = "layout_panel", scene_file = "panels/layout.mei"),
        ),
    ],
)
"#,
    );
    write_file(
        &app_root.join("panels/layout.mei"),
        r#"
scene(id = "layout_scene", profile = "page")
world()
frame()
frame.add_panel(
    id = "layout_panel",
    area = "auto",
    blocks = [
        panel(
            base = panel_ref(id = "summary_panel", scene_file = "panels/detail.mei"),
        ),
    ],
)
"#,
    );
    write_file(
        &app_root.join("panels/detail.mei"),
        r#"
scene(id = "detail_scene", profile = "page")
world()
world.add_metric(
    ds.scalar_map(
        id = "warning_supervision",
        label = "监督事项",
        values = {"value": 2000},
        unit = "项",
        schema = [ds.column("value", "number")],
    ),
)
frame()
frame.add_panel(
    id = "summary_panel",
    area = "auto",
    blocks = [
        metric_card(
            id = "metric_detail",
            source = metric_ref("warning_supervision"),
        ),
    ],
)
"#,
    );

    let compiled = compile_app_from_root(&root, &app_root)
        .expect("compile nested panel_ref imported world metrics");
    assert!(
        compiled
            .diagnostics
            .iter()
            .all(|diag| !matches!(diag.severity, crate::Severity::Error)),
        "nested panel(base=panel_ref(...)) should import grandchild direct world metrics: {:?}",
        compiled.diagnostics
    );
    assert!(
        compiled
            .world_metrics
            .contains_key("panels/detail.mei::warning_supervision"),
        "imported panel metrics are namespaced by capsule path"
    );
    assert!(
        !compiled.world_metrics.contains_key("warning_supervision"),
        "flat metric id must not leak into host world_metrics ledger"
    );
    let resource_ids = compiled
        .resources
        .iter()
        .map(|resource| resource.id.as_str())
        .collect::<Vec<_>>();
    assert!(
        resource_ids
            .iter()
            .any(|id| id.contains("panels/detail.mei::metrics")),
        "resources should contain imported world metrics dataset for panels/detail.mei, got {resource_ids:?}"
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
    let compiled =
        compile_app_from_root(&root, &app_root).expect("compile metric_card base v_align");
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
    let compiled =
        compile_app_from_root(&root, &app_root).expect("compile metric_card base bg source");
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
fn compile_panel_slot_syntax_maps_to_projection_props() {
    let root = temp_root("panel-slot-syntax");
    let app_root = root.join("panel-slot-syntax");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(id = "panel-slot-syntax", default_scene = "home")
scene(id = "home", profile = "page")
world(resources = [])
frame()
frame.add_panel(
    id = "filter",
    area = "auto",
    slot = panel_slot(kind = "filter", source = "filter_schema"),
    blocks = [],
)
frame.add_panel(
    id = "preview",
    area = "auto",
    slot = panel_slot(kind = "row_preview", accepts = ["summary"], selection_from = "list"),
    blocks = [],
)
"#,
    );
    let compiled = compile_app_from_root(&root, &app_root).expect("compile panel slot syntax");
    let contract = compiled.scene_contract.expect("contract");
    assert_eq!(
        contract.panels[0]
            .props
            .get("projection_role")
            .and_then(|value| value.as_str()),
        Some("filter")
    );
    assert_eq!(
        contract.panels[0]
            .props
            .get("projection_source")
            .and_then(|value| value.as_str()),
        Some("filter_schema")
    );
    assert_eq!(
        contract.panels[1]
            .props
            .get("projection_role")
            .and_then(|value| value.as_str()),
        Some("row_preview")
    );
    assert_eq!(
        contract.panels[1]
            .props
            .get("selection_source")
            .and_then(|value| value.as_str()),
        Some("list")
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_link_params_passthrough() {
    let root = temp_root("link-params-passthrough");
    let app_root = root.join("link-params-passthrough");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(id = "link-params-passthrough", default_scene = "home")
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
    let compiled = compile_app_from_root(&root, &app_root).expect("compile link params passthrough");
    let contract = compiled.scene_contract.expect("contract");
    let popup = contract.panels[0].props.get("popup").expect("popup");
    assert_eq!(
        popup.get("params")
            .and_then(|value| value.get("entry"))
            .and_then(|value| value.as_str()),
        Some("overview")
    );
    assert_eq!(
        popup.get("params")
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
app(id = "generic-drilldown-rowset", default_scene = "home")
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
)
world(resources = [])
frame()
"#,
    );
    let compiled = compile_app_from_root(&root, &app_root).expect("compile generic drilldown rowset");
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
        popup.get("projection_slots").and_then(|value| value.as_array()).is_some_and(|slots| !slots.is_empty()),
        "expected lowered projection_slots, got {:?}",
        popup
    );
    let _ = fs::remove_dir_all(&root);
}

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
        popup.get("filter_schema")
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
            slot.get("layout_zone")
                .and_then(|value| value.as_str())
                == Some("chart")
        }),
        "expected chart zone slot, got {:?}",
        slots
    );
    assert!(
        slots.iter().any(|slot| {
            slot.get("layout_zone")
                .and_then(|value| value.as_str())
                == Some("detail")
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
        popup.get("filter_schema")
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
            slot.get("layout_zone")
                .and_then(|value| value.as_str())
                == Some("list")
        }),
        "expected list zone slot, got {:?}",
        slots
    );
    assert!(
        slots.iter().any(|slot| {
            slot.get("layout_zone")
                .and_then(|value| value.as_str())
                == Some("preview")
        }),
        "expected preview zone slot, got {:?}",
        slots
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
