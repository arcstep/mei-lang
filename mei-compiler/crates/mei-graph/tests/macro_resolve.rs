use mei_graph::{expand_v2_file, MacroRegistry, TemplateRoots};
use mei_syntax::v2::{parse_v2_source, parse_v2_source_file, V2Item};
use std::path::PathBuf;

fn ws_demo_templates_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../workspaces/ws-demo-v2/stock/templates")
}

fn ws_demo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../workspaces/ws-demo-v2")
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
    assert!(registry.resolve_name("metric_triptych_fill_body").is_some());
    assert!(registry.resolve_name("narrow_metric_slot").is_some());
    assert!(registry.resolve_name("solid_metric_fill_slot").is_some());
}

#[test]
fn screen_header_default_assets_macro_fully_expanded_at_compile() {
    let templates_root = ws_demo_templates_root();
    let roots = TemplateRoots::stock_only(templates_root.clone());
    let registry = MacroRegistry::load_dir(&templates_root).expect("load");
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../workspaces/ws-demo-v2/apps/mini-park/src/scene/home/t1/r-header/s-header/layout.mei");
    let parsed = parse_v2_source_file(&path).expect("parse mini-park header");
    let expanded = expand_v2_file(&parsed, &registry, &roots).expect("expand");
    let dumped = serde_json::to_string(&expanded).expect("serialize expanded");

    assert!(
        !dumped.contains("screen_header_assets"),
        "nested macro defaults must expand before lower; got: {dumped}"
    );
    assert!(
        dumped.contains("cockpit.header-brand") || dumped.contains("header-brand"),
        "expected header brand component in expanded IR: {dumped}"
    );
}

#[test]
fn layout_defaults_exports_constructor_names_without_recursive_expansion() {
    let templates_root = ws_demo_templates_root();
    let roots = TemplateRoots::stock_only(templates_root.clone());
    let registry = MacroRegistry::load_dir(&templates_root).expect("load");
    let parsed = parse_v2_source(
        r#"
use template "cockpit/layout-defaults" as ui

content_panel(
    id = "demo",
    blocks = [
        ui.narrow_metric_card(id = "n", source = {"label": "A", "value": "1", "unit": "x"}),
        ui.plain_metric_card(id = "p"),
        ui.compound_panel(id = "c", blocks = []),
    ],
)
"#,
    )
    .expect("parse layout defaults usage");
    let expanded = expand_v2_file(&parsed, &registry, &roots).expect("expand");
    let dumped = serde_json::to_string(&expanded).expect("serialize expanded");

    assert!(!dumped.contains("\"ui\""), "qualified calls must expand");
    assert!(
        dumped.contains("metric-bg-normal") && dumped.contains("background"),
        "narrow_metric_card must bake explicit background: {dumped}"
    );
    assert!(
        dumped.contains("metric-bg-target"),
        "compound_panel must bake compound background: {dumped}"
    );
    assert!(
        !dumped.contains("chrome_metric") && !dumped.contains("\"surface\""),
        "must not use chrome_* helpers or surface tokens: {dumped}"
    );
}

#[test]
fn layered_resolve_prefers_app_templates_then_src_then_stock() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let app_root = tmp.path().join("apps/demo");
    let stock = tmp.path().join("stock/templates");
    let app_templates = app_root.join("src/templates");
    let app_src = app_root.join("src");
    std::fs::create_dir_all(app_templates.join("shared")).unwrap();
    std::fs::create_dir_all(app_src.join("scene/home/t1")).unwrap();
    std::fs::create_dir_all(stock.join("cockpit")).unwrap();
    std::fs::create_dir_all(stock.join("shared")).unwrap();

    std::fs::write(
        stock.join("cockpit/layout-defaults.mei"),
        r#"
template panel(id = "stock"):
    panel(id = id, props = {"from": "stock"})
"#,
    )
    .unwrap();
    std::fs::write(
        stock.join("shared/x.mei"),
        r#"
template mark():
    {"layer": "stock"}
"#,
    )
    .unwrap();
    std::fs::write(
        app_templates.join("shared/x.mei"),
        r#"
template mark():
    {"layer": "app_templates"}
"#,
    )
    .unwrap();
    std::fs::write(
        app_src.join("scene/home/t1/geometry.mei"),
        r#"
template focus_inset():
    {"inset": 12}
"#,
    )
    .unwrap();

    let roots = TemplateRoots::from_app_and_stock(&app_root, stock);
    let registry = MacroRegistry::load_layered(&roots).expect("load layered");

    assert_eq!(
        registry
            .resolve_path("shared/x")
            .map(|d| d.file_path.as_str()),
        Some("shared/x")
    );
    let mark = registry.resolve_path("shared/x").expect("shared/x");
    let dumped = serde_json::to_string(&mark.body).unwrap();
    assert!(
        dumped.contains("app_templates"),
        "app templates must override stock: {dumped}"
    );

    assert!(
        registry.resolve_path("scene/home/t1/geometry").is_some(),
        "scene-colocated geometry must resolve from app src"
    );
    assert!(
        registry.resolve_path("cockpit/layout-defaults").is_some(),
        "stock fallback must still work"
    );

    let parsed = parse_v2_source(
        r#"
use template "shared/x" as x
use template "scene/home/t1/geometry" as geo
use template "cockpit/layout-defaults" as ui

content_panel(
    id = "demo",
    blocks = [x.mark(), geo.focus_inset(), ui.panel(id = "p")],
)
"#,
    )
    .expect("parse");
    let expanded = expand_v2_file(&parsed, &registry, &roots).expect("expand");
    let dumped = serde_json::to_string(&expanded).unwrap();
    assert!(dumped.contains("app_templates"), "{dumped}");
    assert!(dumped.contains("\"inset\""), "{dumped}");
    assert!(dumped.contains("from"), "{dumped}");
}

#[test]
fn mini_data_geometry_resolves_via_layered_registry() {
    let workspace = ws_demo_root();
    let app_root = workspace.join("apps/mini-data");
    let stock = workspace.join("stock/templates");
    let roots = TemplateRoots::from_app_and_stock(&app_root, stock);
    let registry = MacroRegistry::load_layered(&roots).expect("load");
    assert!(
        registry.resolve_path("scene/home/t1/geometry").is_some(),
        "mini-data geometry.mei must be visible to use template"
    );
    assert!(registry.resolve_name("focus_inset").is_some());
    assert!(
        registry.resolve_path("shared/filter-fields").is_some(),
        "app src/templates/shared must resolve"
    );
    assert!(registry.resolve_path("cockpit/layout-defaults").is_some());
}
