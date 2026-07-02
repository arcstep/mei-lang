use mei_graph::MacroRegistry;
use mei_syntax::v2::{parse_v2_source, V2Item};
use std::path::PathBuf;

fn ws_demo_templates_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../workspaces/ws-demo-v2/stock/templates")
}

#[test]
fn business_layouts_registers_slot_macros() {
    let templates_root = ws_demo_templates_root();
    let path = templates_root.join("cockpit/business-layouts.mei");
    let source = std::fs::read_to_string(&path).expect("read");
    let parsed = parse_v2_source(&source).expect("parse");
    let template_count = parsed
        .items
        .iter()
        .filter(|item| matches!(item, V2Item::TemplateDecl { .. }))
        .count();
    assert!(
        template_count >= 20,
        "expected full business-layouts template surface, got {template_count}"
    );

    let registry = MacroRegistry::load_dir(&templates_root).expect("load");
    assert!(registry.resolve_name("metric_triptych_compound_body").is_some());
    assert!(registry.resolve_name("narrow_metric_slot").is_some());
    assert!(registry.resolve_name("story_opinion_block").is_some());
}
