use std::fs;

use super::{compile_app_from_root, temp_root, write_file};

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
            .slot
            .as_ref()
            .and_then(|slot| slot.kind.as_deref()),
        Some("filter")
    );
    assert_eq!(
        contract.panels[0]
            .slot
            .as_ref()
            .and_then(|slot| slot.source.as_deref()),
        Some("filter_schema")
    );
    assert_eq!(
        contract.panels[1]
            .slot
            .as_ref()
            .and_then(|slot| slot.kind.as_deref()),
        Some("row_preview")
    );
    assert_eq!(
        contract.panels[1]
            .slot
            .as_ref()
            .and_then(|slot| slot.selection_from.as_deref()),
        Some("list")
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
