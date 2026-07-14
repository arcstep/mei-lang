use std::fs;

use super::{compile_app_from_root, temp_root, write_file};

#[test]
fn compile_refs_scenario3_world_ref_imports_external_resources_for_props() {
    let root = temp_root("refs-scenario-3");
    let app_root = root.join("refs-03");
    write_file(
        &app_root.join("shared-world.mei"),
        r#"
world(resources = [resource(id = "shared_doc", kind = "document", content = "from external world")])
"#,
    );
    write_file(
        &app_root.join("shared-frame.mei"),
        r#"
scene(id = "shared", profile = "page")
world()
frame()
"#,
    );
    write_file(
        &app_root.join("main.mei"),
        r#"
app(id = "refs-03", default_stage = "home")
scene(
    id = "home",
    profile = "page",
    world = world_ref(scene_file = "shared-world.mei"),
    frame = frame_ref(scene_file = "shared-frame.mei"),
)
frame.add_panel(
    id = "welcome",
    area = "auto",
    blocks = [doc.markdown(area = "auto", resource = resource_ref("shared_doc"))],
)
"#,
    );
    let compiled = compile_app_from_root(&root, &app_root).expect("compile refs scenario 3");
    assert!(
        compiled
            .diagnostics
            .iter()
            .all(|diag| !matches!(diag.severity, crate::Severity::Error)),
        "scenario 3 should compile with imported world: {:?}",
        compiled.diagnostics
    );
    assert!(
        compiled
            .resources
            .iter()
            .any(|item| item.id == "shared_doc"),
        "world_ref should make shared_doc available to props"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn compile_builtin_text_shorthand() {
    let root = temp_root("mei-text-shorthand");
    let app_root = root.join("text-app");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(id = "text-app", default_stage = "home")

scene(id = "home", profile = "page")

world(id = "home_world", resources = [])

frame(layout = flex(direction = "column"))

frame.add_panel(
    id = "p",
    area = "auto",
    blocks = [
        text("我是文本"),
        text(html = "<b>加粗</b>"),
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
    write_file(
        &root.join(".stock/components/mei/text.js"),
        "// stub for compile asset resolution",
    );

    let compiled = compile_app_from_root(&root, &app_root).expect("compile text shorthand");
    assert!(
        compiled
            .diagnostics
            .iter()
            .all(|diag| !matches!(diag.severity, crate::Severity::Error)),
        "text shorthand should compile: {:?}",
        compiled.diagnostics
    );
    let contract = compiled.scene_contract.expect("scene contract");
    let panel = contract
        .panels
        .iter()
        .find(|p| p.id == "p")
        .expect("panel p");
    assert_eq!(panel.blocks.len(), 2);
    for (idx, expected) in ["mei.text", "mei.text"].iter().enumerate() {
        match &panel.blocks[idx] {
            crate::UiTreeNode::Block(block) => assert_eq!(block.use_key, *expected),
            other => panic!("block {idx} should be Block, got {other:?}"),
        }
    }
    assert!(
        compiled
            .component_assets
            .iter()
            .any(|a| a.key == "mei.text"),
        "mei.text should be in component_assets"
    );
    let _ = fs::remove_dir_all(&root);
}
