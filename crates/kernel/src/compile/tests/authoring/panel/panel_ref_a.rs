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

