use mei_graph::{expand_v2_file, MacroRegistry, TemplateRoots};
use mei_syntax::v2::{parse_v2_source, V2Item};
use std::path::PathBuf;

fn mei_lang_root() -> PathBuf {
    let mut cur = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for _ in 0..10 {
        if cur.join("stock/templates").is_dir() {
            return cur;
        }
        if !cur.pop() {
            break;
        }
    }
    panic!(
        "mei-lang root with stock/templates not found from {}",
        env!("CARGO_MANIFEST_DIR")
    );
}

fn stock_templates_root() -> PathBuf {
    mei_lang_root().join("stock/templates")
}

fn stock_legacy_templates_root() -> PathBuf {
    mei_lang_root().join("stock/legacy/templates")
}

fn optional_external_workspace() -> Option<PathBuf> {
    let raw = std::env::var("MEI_TEST_WORKSPACE").ok()?;
    let path = PathBuf::from(raw.trim());
    if path.as_os_str().is_empty() || !path.is_dir() {
        return None;
    }
    Some(path.canonicalize().unwrap_or(path))
}

#[test]
fn business_layouts_registers_slot_macros() {
    let templates_root = stock_legacy_templates_root();
    let path = templates_root.join("cockpit/business-layouts.mei");
    let source = std::fs::read_to_string(&path).expect("read business-layouts from stock/legacy");
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
    let templates_root = stock_templates_root();
    let roots = TemplateRoots::stock_only(templates_root.clone());
    let registry = MacroRegistry::load_dir(&templates_root).expect("load");
    let parsed = parse_v2_source(
        r#"
use template "cockpit/panel/shell-macros" as ui

content_panel(
    id = "wrap",
    blocks = [ui.screen_header(title = "Demo")],
)
"#,
    )
    .expect("parse screen_header usage");
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
    let templates_root = stock_templates_root();
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
        !dumped.contains("chrome_metric"),
        "must not use chrome_* helpers: {dumped}"
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
    let Some(workspace) = optional_external_workspace() else {
        eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
        return;
    };
    let app_root = workspace.join("apps/mini-data");
    if !app_root.is_dir() {
        eprintln!("skip: apps/mini-data missing under MEI_TEST_WORKSPACE");
        return;
    }
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

#[test]
fn qualified_call_prefers_import_alias_over_local_same_name() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let app_root = tmp.path().join("apps/demo");
    let stock = tmp.path().join("stock/templates");
    let app_templates = app_root.join("src/templates");
    std::fs::create_dir_all(app_templates.join("shared")).unwrap();
    std::fs::create_dir_all(stock.join("cockpit/t2")).unwrap();

    std::fs::write(
        stock.join("cockpit/t2/t2-nav.mei"),
        r#"
template analytics_drilldown_nav(scene_id):
    {"layer": "stock", "scene_id": scene_id}
"#,
    )
    .unwrap();
    std::fs::write(
        app_templates.join("shared/drilldown-local-nav.mei"),
        r#"
use template "cockpit/t2/t2-nav" as t2_nav

template analytics_drilldown_nav(scene_id):
    t2_nav.analytics_drilldown_nav(scene_id = scene_id)
"#,
    )
    .unwrap();

    let roots = TemplateRoots::from_app_and_stock(&app_root, stock);
    let registry = MacroRegistry::load_layered(&roots).expect("load layered");
    assert_eq!(
        registry
            .resolve_name("analytics_drilldown_nav")
            .map(|d| d.file_path.as_str()),
        Some("shared/drilldown-local-nav"),
        "unqualified global name stays app-first"
    );

    let parsed = parse_v2_source(
        r#"
use template "shared/drilldown-local-nav" as nav

content_panel(
    id = "demo",
    blocks = [nav.analytics_drilldown_nav(scene_id = "home")],
)
"#,
    )
    .expect("parse");
    let expanded = expand_v2_file(&parsed, &registry, &roots).expect("expand without recursion");
    let dumped = serde_json::to_string(&expanded).unwrap();
    assert!(dumped.contains("stock"), "{dumped}");
    assert!(dumped.contains("home"), "{dumped}");
}

#[test]
fn macro_expansion_cycle_is_reported_without_stack_overflow() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let stock = tmp.path().join("stock/templates");
    std::fs::create_dir_all(stock.join("cycle")).unwrap();
    std::fs::write(
        stock.join("cycle/a.mei"),
        r#"
use template "cycle/b" as b

template ping():
    b.pong()
"#,
    )
    .unwrap();
    std::fs::write(
        stock.join("cycle/b.mei"),
        r#"
use template "cycle/a" as a

template pong():
    a.ping()
"#,
    )
    .unwrap();

    let roots = TemplateRoots::stock_only(stock.clone());
    let registry = MacroRegistry::load_dir(&stock).expect("load");
    let parsed = parse_v2_source(
        r#"
use template "cycle/a" as a

content_panel(id = "demo", blocks = [a.ping()])
"#,
    )
    .expect("parse");
    let err = expand_v2_file(&parsed, &registry, &roots).expect_err("cycle");
    let message = err.to_string();
    assert!(
        message.contains("macro_expansion_cycle"),
        "unexpected error: {message}"
    );
}

#[test]
fn multi_template_module_non_first_export_resolves_via_alias_method() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let stock = tmp.path().join("stock/templates");
    std::fs::create_dir_all(stock.join("cockpit/t2")).unwrap();
    std::fs::write(
        stock.join("cockpit/t2/t2-nav.mei"),
        r#"
template analytics_drilldown_nav(scene_id):
    {"kind": "nav", "scene_id": scene_id}

template analytics_drilldown_nav_with_row(scene_id, row_spec):
    {"kind": "nav_row", "scene_id": scene_id, "row": row_spec}
"#,
    )
    .unwrap();

    let roots = TemplateRoots::stock_only(stock.clone());
    let registry = MacroRegistry::load_dir(&stock).expect("load");
    assert!(registry
        .resolve_name("analytics_drilldown_nav_with_row")
        .is_some());

    let parsed = parse_v2_source(
        r#"
use template "cockpit/t2/t2-nav" as t2_nav

content_panel(
    id = "demo",
    blocks = [
        t2_nav.analytics_drilldown_nav_with_row(
            scene_id = "home",
            row_spec = {"filter_key": "k"},
        ),
    ],
)
"#,
    )
    .expect("parse");
    let expanded = expand_v2_file(&parsed, &registry, &roots).expect("expand");
    let dumped = serde_json::to_string(&expanded).unwrap();
    assert!(dumped.contains("nav_row"), "{dumped}");
    assert!(dumped.contains("filter_key"), "{dumped}");
}
