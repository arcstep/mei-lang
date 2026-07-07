use mei_graph::{expand_v2_file, MacroRegistry};
use mei_syntax::v2::{parse_v2_source, parse_v2_source_file, V2Item};
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

#[test]
fn screen_header_default_assets_macro_fully_expanded_at_compile() {
    let templates_root = ws_demo_templates_root();
    let registry = MacroRegistry::load_dir(&templates_root).expect("load");
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../workspaces/ws-demo-v2/apps/mini-park/src/scene/home/t1/r-header/s-header/layout.mei");
    let parsed = parse_v2_source_file(&path).expect("parse mini-park header");
    let expanded = expand_v2_file(&parsed, &registry, &templates_root).expect("expand");
    let dumped = serde_json::to_string(&expanded).expect("serialize expanded");

    assert!(
        !dumped.contains("screen_header_assets"),
        "nested macro defaults must expand before lower; got: {dumped}"
    );
    assert!(
        dumped.contains("header/screen-title-bg@3x.svg"),
        "expected title_bg asset ref in expanded IR: {dumped}"
    );
    assert!(
        dumped.contains("header/screen-title-center@3x.svg"),
        "expected title_mid asset ref in expanded IR: {dumped}"
    );
}
