use std::path::PathBuf;
use std::sync::Once;

use mei_host_core::HostContext;
use mei_host_graph::{assemble_scope_from_registry, import_bundle, ImportOptions};
use mei_lang_app::{
    load_topbar_menu_context, page_body_theme_style, render_page, UiRouteMode,
};
use mei_lang_kernel::WorkspaceAppMeta;

static INIT: Once = Once::new();

fn ws_demo_v2() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../workspaces/ws-demo-v2")
        .canonicalize()
        .expect("ws-demo-v2")
}

fn pretty_panels_bundle() -> PathBuf {
    ws_demo_v2().join("apps/pretty-panels/env/current/build/exchange/pretty-panels.meibundle")
}

fn ensure_pretty_panels_imported() -> PathBuf {
    let workspace = ws_demo_v2();
    INIT.call_once(|| {
        assert!(
            pretty_panels_bundle().is_file(),
            "run `mei-compiler compile --workspace ws-demo-v2 --app pretty-panels` first"
        );
        let ctx = HostContext::new(workspace.clone(), "pretty-panels");
        import_bundle(
            &ctx,
            &ImportOptions {
                bundle_path: Some(pretty_panels_bundle()),
            },
        )
        .expect("import pretty-panels bundle");
    });
    workspace
}

fn enforcement_body_cell_style(html: &str) -> String {
    let marker = "data-mei-panel-id=\"left_rail/enforcement\"";
    let start = html
        .find(marker)
        .unwrap_or_else(|| panic!("missing {marker}"));
    let chunk = &html[start..start.saturating_add(6000)];
    chunk
        .split("data-mei-panel-body=\"true\" class=\"panel-body-cell\" style=\"")
        .nth(1)
        .and_then(|tail| tail.split('"').next())
        .unwrap_or("")
        .to_string()
}

#[test]
fn pretty_panels_home_ssr_applies_titled_shell_body_padding() {
    let workspace = ensure_pretty_panels_imported();
    let outcome = assemble_scope_from_registry(workspace.as_path(), "pretty-panels", "home")
        .expect("assemble")
        .expect("home outcome");
    let topbar_menu = load_topbar_menu_context(workspace.as_path());
    let apps = vec![WorkspaceAppMeta {
        id: "pretty-panels".to_string(),
        title: outcome.compiled.title.clone(),
        root: outcome.compiled.app_root.clone(),
    }];
    let workspace_cfg = mei_lang_kernel::load_workspace_config(workspace.as_path());
    let theme_style =
        page_body_theme_style(&workspace_cfg, Some(&outcome.compiled), None);
    let html = render_page(
        &apps,
        &outcome.compiled,
        "pretty-panels",
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
    );
    let body_cell = enforcement_body_cell_style(html.as_str());
    assert!(
        body_cell.contains("padding:8px 6px 6px 6px"),
        "layoutTuning compact should override enforcement section body padding, got `{body_cell}`"
    );
    let layout_errors: Vec<_> = outcome
        .compiled
        .diagnostics
        .iter()
        .filter(|d| d.code.starts_with("layout_policy_"))
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
        html.contains("data-mei-slot-frame-bg=\"true\"") && html.contains("metric-bg-long"),
        "AI compound card should carry slot frame bg with metric-bg-long"
    );
}

#[test]
fn pretty_panels_home_layer_plan_includes_t1_viewport_chrome() {
    let workspace = ensure_pretty_panels_imported();
    let outcome = assemble_scope_from_registry(workspace.as_path(), "pretty-panels", "home")
        .expect("assemble")
        .expect("home outcome");
    let contract = outcome
        .compiled
        .scene_contract
        .as_ref()
        .expect("scene contract");
    let map_stage = contract
        .panels
        .iter()
        .find(|panel| panel.id == "map_stage")
        .expect("map_stage panel");
    assert_eq!(
        map_stage.props.get("__mei_tier").and_then(|v| v.as_str()),
        Some("t0")
    );
    let viewport_frame = find_panel_by_id(&contract.panels, "viewport_frame")
        .expect("viewport_frame nested under center_rail");
    assert_eq!(
        viewport_frame.props.get("__mei_tier").and_then(|v| v.as_str()),
        Some("t1")
    );
    for panel_id in ["stage_aperture_frame", "stage_aperture"] {
        assert!(
            find_panel_by_id(&contract.panels, panel_id).is_some(),
            "missing nested {panel_id} under center_rail map viewport"
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
    for expected in [
        "home_header",
        "left_rail",
        "center_rail",
        "right_rail",
    ] {
        assert!(
            chrome_ids.contains(&expected),
            "layer_plan t1 should include {expected}: {chrome_ids:?}"
        );
    }
}

fn find_panel_by_id<'a>(panels: &'a [mei_lang_kernel::PanelDecl], id: &str) -> Option<&'a mei_lang_kernel::PanelDecl> {
    for panel in panels {
        if panel.id == id || panel.id.ends_with(&format!("/{id}")) {
            return Some(panel);
        }
        for node in &panel.blocks {
            if let mei_lang_kernel::UiNodeDecl::Panel(child) = node {
                if let Some(found) = find_panel_by_id(std::slice::from_ref(child), id) {
                    return Some(found);
                }
            }
        }
    }
    None
}

fn panel_content_budget_rows(contract: &mei_lang_kernel::SceneContract, panel_id: &str) -> Option<Vec<i64>> {
    let panel = find_panel_by_id(&contract.panels, panel_id)?;
    panel
        .props
        .get("__mei_content_budget")
        .and_then(|b| b.get("rows"))
        .and_then(|rows| {
            rows.as_array().map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_i64())
                    .collect::<Vec<_>>()
            })
        })
}

#[test]
fn pretty_panels_right_rail_sections_have_no_layout_policy_overflow() {
    let workspace = ensure_pretty_panels_imported();
    let outcome = assemble_scope_from_registry(workspace.as_path(), "pretty-panels", "home")
        .expect("assemble")
        .expect("home outcome");
    let right_rail_errors: Vec<_> = outcome
        .compiled
        .diagnostics
        .iter()
        .filter(|d| {
            d.code.starts_with("layout_policy_")
                && d.message.contains("right_rail")
        })
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
    assert_eq!(
        panel_content_budget_rows(contract, "effectiveness-stats").as_deref(),
        Some([70, 70].as_ref())
    );
    assert_eq!(
        panel_content_budget_rows(contract, "typical-cases").as_deref(),
        Some([294].as_ref())
    );
}

#[test]
fn pretty_panels_layout_tuning_merges_content_budget_via_index() {
    let workspace = ensure_pretty_panels_imported();
    let outcome = assemble_scope_from_registry(workspace.as_path(), "pretty-panels", "home")
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
    let rows = panel_content_budget_rows(contract, "enforcement-stats").expect("budget rows");
    assert_eq!(
        rows,
        vec![88],
        "layoutTuning contentBudget should merge onto enforcement-stats"
    );
}
