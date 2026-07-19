use std::path::PathBuf;
use std::sync::Once;

use mei_host_core::HostContext;
use mei_host_graph::{
    assemble_scope_from_registry, clear_assemble_cache_for_app, import_bundle, ImportOptions,
};
use mei_lang_app::{load_topbar_menu_context, page_body_theme_style, render_page, UiRouteMode};
use mei_lang_kernel::WorkspaceAppMeta;

static INIT: Once = Once::new();

fn ws_demo_v2() -> Option<PathBuf> {
    mei_test_support::optional_external_workspace()
}

fn zhifa_bundle(workspace: &std::path::Path) -> PathBuf {
    workspace.join("apps/zhifa/env/current/build/exchange/zhifa.meibundle")
}

/// Local monorepo optional. Returns `None` when `ws-demo-v2` is not beside mei-lang.
fn ensure_zhifa_imported() -> Option<PathBuf> {
    let workspace = ws_demo_v2()?;
    INIT.call_once(|| {
        let bundle = zhifa_bundle(&workspace);
        assert!(
            bundle.is_file(),
            "run `mei-compiler compile --workspace <ws-demo-v2> --app zhifa` first"
        );
        let ctx = HostContext::new(workspace.clone(), "zhifa");
        import_bundle(
            &ctx,
            &ImportOptions {
                bundle_path: Some(bundle),
            },
        )
        .expect("import zhifa bundle");
        clear_assemble_cache_for_app("zhifa");
    });
    Some(workspace)
}

fn enforcement_body_cell_style(html: &str) -> String {
    let marker = html
        .find("data-mei-panel-id=\"t1/left_rail/enforcement\"")
        .or_else(|| html.find("data-mei-panel-id=\"left_rail/enforcement\""))
        .unwrap_or_else(|| {
            panic!(
                "missing enforcement section panel id in SSR HTML (expected t1/left_rail/enforcement)"
            )
        });
    let chunk = &html[marker..marker.saturating_add(6000)];
    chunk
        .split("data-mei-panel-body=\"true\" class=\"panel-body-cell\" style=\"")
        .nth(1)
        .and_then(|tail| tail.split('"').next())
        .unwrap_or("")
        .to_string()
}

#[test]
fn zhifa_home_ssr_applies_titled_shell_body_padding() {
    let Some(workspace) = ensure_zhifa_imported() else {
        eprintln!("skip: ws-demo-v2 not present (local monorepo optional)");
        return;
    };
    let outcome = assemble_scope_from_registry(workspace.as_path(), "zhifa", "home")
        .expect("assemble")
        .expect("home outcome");
    let topbar_menu = load_topbar_menu_context(workspace.as_path());
    let apps = vec![WorkspaceAppMeta {
        id: "zhifa".to_string(),
        title: outcome.compiled.title.clone(),
        root: outcome.compiled.app_root.clone(),
    }];
    let workspace_cfg = mei_lang_kernel::load_workspace_config(workspace.as_path());
    let theme_style = page_body_theme_style(&workspace_cfg, Some(&outcome.compiled), None);
    let html = render_page(
        &apps,
        &outcome.compiled,
        "zhifa",
        Some(&topbar_menu),
        UiRouteMode::App,
        Some(outcome.compiled.active_target_file.as_str()),
        None,
        None,
        Some("home"),
        None,
        Some("preview"),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        false,
        false,
        None,
        &[],
        false,
        None,
        None,
        theme_style.as_str(),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        &[],
    );
    let body_cell = enforcement_body_cell_style(html.as_str());
    assert!(
        body_cell.contains("padding:8px 4px 2px 4px"),
        "theme.layout dense_strip_100 should apply enforcement section body padding, got `{body_cell}`"
    );
    let layout_errors: Vec<_> = outcome
        .compiled
        .diagnostics
        .iter()
        .filter(|d| {
            d.code.starts_with("layout_policy_")
                && (d.message.contains("enforcement")
                    || d.message.contains("issue_body")
                    || d.message.contains("left_rail/enforcement")
                    || d.message.contains("right_rail/issue"))
        })
        .collect();
    assert!(
        layout_errors.is_empty(),
        "unexpected layout_policy diagnostics: {layout_errors:?}"
    );
    assert!(
        html.contains("执法单位") || html.contains("执法对象"),
        "enforcement-stats metric labels should appear in SSR HTML"
    );
    assert!(
        html.contains("行政检查") || html.contains("AI执法识别"),
        "inspection section should appear in SSR HTML"
    );
    assert!(
        html.contains("metric-bg-target"),
        "enforcement compound slot should render metric-bg-target frame in SSR"
    );
    assert!(
        !html.contains("padding:5px 0"),
        "enforcement slots should not use vertical shell padding that clips frame"
    );
    assert!(
        html.contains("data-mei-slot-frame-bg=\"true\"")
            && (html.contains("metric-bg-long") || html.contains("metric-bg-target")),
        "enforcement compound card should carry slot frame bg"
    );
}

#[test]
fn zhifa_home_layer_plan_includes_t1_viewport_chrome() {
    let Some(workspace) = ensure_zhifa_imported() else {
        eprintln!("skip: ws-demo-v2 not present (local monorepo optional)");
        return;
    };
    let outcome = assemble_scope_from_registry(workspace.as_path(), "zhifa", "home")
        .expect("assemble")
        .expect("home outcome");
    let contract = outcome
        .compiled
        .scene_contract
        .as_ref()
        .expect("scene contract");
    let map_stage = find_panel_by_id(&contract.panels, "map_stage").expect("map_stage panel");
    assert_eq!(
        map_stage.props.get("__mei_tier").and_then(|v| v.as_str()),
        Some("t0")
    );
    let center_rail =
        find_panel_by_id(&contract.panels, "center_rail").expect("center_rail region panel");
    assert_eq!(
        center_rail.props.get("__mei_tier").and_then(|v| v.as_str()),
        Some("t1")
    );
    for panel_id in [
        "map-viewport",
        "map-interaction-surface",
        "stage-aperture-frame",
        "map-tools-slot",
        "stage-aperture-hint",
    ] {
        assert!(
            find_panel_by_id(&contract.panels, panel_id).is_some(),
            "missing {panel_id} nested under center_rail map_viewport section"
        );
    }
    let chrome = outcome
        .layer_plan
        .get("tiers")
        .and_then(|v| v.get("t1"))
        .and_then(|v| v.as_array())
        .expect("t1 tier entries");
    let chrome_ids: Vec<&str> = chrome
        .iter()
        .filter_map(|entry| entry.get("panelId").and_then(|v| v.as_str()))
        .collect();
    for expected in ["home_header", "left_rail", "center_rail", "right_rail"] {
        assert!(
            chrome_ids.contains(&expected),
            "layer_plan t1 should include {expected}: {chrome_ids:?}"
        );
    }
}

fn find_panel_by_id<'a>(
    panels: &'a [mei_lang_kernel::UiNodeDecl],
    id: &str,
) -> Option<&'a mei_lang_kernel::UiNodeDecl> {
    for panel in panels {
        if panel.id == id || panel.id.ends_with(&format!("/{id}")) {
            return Some(panel);
        }
        for node in &panel.blocks {
            if let mei_lang_kernel::UiTreeNode::Panel(child) = node {
                if let Some(found) = find_panel_by_id(std::slice::from_ref(child), id) {
                    return Some(found);
                }
            }
        }
    }
    None
}

fn panel_has_layout_fill(contract: &mei_lang_kernel::SceneContract, panel_id: &str) -> bool {
    find_panel_by_id(&contract.panels, panel_id)
        .and_then(|panel| panel.props.get("__mei_layout_fill"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

#[test]
fn zhifa_right_rail_sections_have_no_layout_policy_overflow() {
    let Some(workspace) = ensure_zhifa_imported() else {
        eprintln!("skip: ws-demo-v2 not present (local monorepo optional)");
        return;
    };
    let outcome = assemble_scope_from_registry(workspace.as_path(), "zhifa", "home")
        .expect("assemble")
        .expect("home outcome");
    let right_rail_errors: Vec<_> = outcome
        .compiled
        .diagnostics
        .iter()
        .filter(|d| d.code.starts_with("layout_policy_") && d.message.contains("right_rail"))
        .collect();
    assert!(
        right_rail_errors.is_empty(),
        "right_rail layout_policy errors: {right_rail_errors:?}"
    );
    let contract = outcome
        .compiled
        .scene_contract
        .as_ref()
        .expect("scene contract");
    assert!(
        panel_has_layout_fill(contract, "effectiveness-stats"),
        "effectiveness-stats should use fill-down body props"
    );
    assert!(
        panel_has_layout_fill(contract, "typical-cases"),
        "typical-cases should use fill-down body props"
    );
}

#[test]
fn zhifa_theme_layout_merges_via_index() {
    let Some(workspace) = ensure_zhifa_imported() else {
        eprintln!("skip: ws-demo-v2 not present (local monorepo optional)");
        return;
    };
    let outcome = assemble_scope_from_registry(workspace.as_path(), "zhifa", "home")
        .expect("assemble")
        .expect("home outcome");
    assert!(
        !outcome.compiled.ui_layout_index.nodes.is_empty(),
        "assemble should populate ui_layout_index"
    );
    let contract = outcome
        .compiled
        .scene_contract
        .as_ref()
        .expect("scene contract");
    assert!(
        panel_has_layout_fill(contract, "enforcement-stats"),
        "enforcement-stats should use fill-down body props (0327)"
    );
    let enforcement_section =
        find_panel_by_id(&contract.panels, "enforcement").expect("enforcement section");
    assert!(
        enforcement_section
            .props
            .get("paddingProfile")
            .or_else(|| enforcement_section.props.get("padding_profile"))
            .is_some()
            || outcome
                .compiled
                .ui_layout_index
                .nodes
                .keys()
                .any(|key| key.contains("left_rail/enforcement")),
        "theme layout / ui_layout_index should cover enforcement section"
    );
}
