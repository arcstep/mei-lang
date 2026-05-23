use std::fs;

use super::{compile_app_from_root, compile_app_from_root_with_options, evaluate_runtime_metric_defs, temp_root, write_file, CompileOptions};

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
