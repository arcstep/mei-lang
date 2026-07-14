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
app(id = "embed-app", default_stage = "home")

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
app(id = "bad-embed", default_stage = "home")

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
app(id = "legacy-embed", default_stage = "home")

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
app(id = "refs-04", default_stage = "home")
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
app(id = "refs-05", default_stage = "home")
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

