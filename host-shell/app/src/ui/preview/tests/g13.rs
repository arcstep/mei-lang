use super::helpers::preview_metric_with_runtime_def;
use super::nodes::component_html;
use serde_json::{json, Value};

#[test]
fn resolve_value_rejects_legacy_explain_object_contract_projection() {
    let resolved = preview_metric_with_runtime_def(json!({
        "explain": {
            "note": "旧 explain object 仍可兼容。",
            "detail_table_metric_id": "sales_total_table",
            "metrics": [
                {"id": "definition", "kind": "definition", "label": "口径"},
                {"id": "detail", "kind": "detail", "label": "销售明细", "fields": ["销售单ID", "客户", "金额"]}
            ]
        }
    }));
    assert!(
        resolved
            .get("__mei_runtime_ref")
            .and_then(|value| value.get("analysis_contract"))
            .is_none(),
        "legacy explain object should not project analysis_contract"
    );
}

#[test]
fn resolve_value_keeps_legacy_drilldown_internal_to_preview() {
    let resolved = preview_metric_with_runtime_def(json!({
        "explain": [
            {"__kind": "explain_item", "id": "definition", "kind": "definition", "label": "口径"},
            {"__kind": "explain_item", "id": "detail", "kind": "detail", "label": "销售明细", "fields": ["销售单ID"], "source": {"__ref": "metric", "id": "sales_total_table"}}
        ],
        "drilldown": {
            "scene": "legacy_board",
            "title": "旧明细板",
            "note": "这只是兼容字段。",
            "table_metric_id": "sales_total_table",
            "dataset_id": "sales_metrics",
            "tabs": ["detail"]
        }
    }));
    let runtime_ref = resolved
        .get("__mei_runtime_ref")
        .and_then(Value::as_object)
        .expect("runtime ref for metric");
    assert!(
        runtime_ref.get("analysis_contract").is_some(),
        "runtime ref should still expose analysis_contract"
    );
    for legacy_key in [
        "drilldown_scene",
        "drilldown_target_scene_id",
        "drilldown_enabled",
        "drilldown_tabs",
        "drilldown_title",
        "drilldown_note",
        "drilldown_table_metric_id",
        "drilldown_dataset_id",
        "drilldown_layout_preset",
        "drilldown_columns",
        "drilldown_headers",
        "drilldown_basis_refs",
        "drilldown_detail_fields",
        "drilldown_recommended_dimensions",
        "drilldown_ratio_numerator",
        "drilldown_ratio_denominator",
        "drilldown_ratio_formula",
        "drilldown_tab_metrics",
    ] {
        assert!(
            !runtime_ref.contains_key(legacy_key),
            "runtime ref should not re-expose legacy preview key: {legacy_key}"
        );
    }
}

#[test]
fn component_html_escapes_quotes_in_data_props_attribute() {
    let props = json!({
        "label": "it's a typical case",
        "popup": {"scene_id": "typical_cases_detail_board"},
    });
    let html = component_html("mei-cockpit-data-table", &props);
    assert!(html.contains("data-props=\""));
    assert!(!html.contains("data-props='"));
    let start = html.find("data-props=\"").expect("data-props attr") + "data-props=\"".len();
    let end = html[start..].find('"').expect("closing quote") + start;
    let payload = &html[start..end];
    let decoded = payload.replace("&quot;", "\"");
    let parsed: Value = serde_json::from_str(&decoded).expect("valid json payload");
    assert_eq!(
        parsed.get("label").and_then(Value::as_str),
        Some("it's a typical case")
    );
}

#[test]
#[ignore = "slow: compiles ws-spbjw/zhifa home for SSR payload measurement"]
fn zhifa_home_build_resolved_data_props_under_5mb() {
    use mei_lang_kernel::{
        compile_app_from_root_with_options, BlockDecl, CompileOptions, UiTreeNode,
    };

    use super::{
        build_preview_runtime_context,
        nodes::component_html,
        resolve::{attach_host_meta, resolve_value, HostMetaOptions, RuntimeSceneAnchor},
        theme,
    };
    use crate::ui::route::UiRouteMode;

    fn walk_blocks<'a>(nodes: &'a [UiTreeNode], out: &mut Vec<&'a BlockDecl>) {
        for node in nodes {
            match node {
                UiTreeNode::Block(block) => out.push(block),
                UiTreeNode::Panel(panel) => walk_blocks(&panel.blocks, out),
                UiTreeNode::PanelRefEmbed(_) => {}
            }
        }
    }

    let Some(source_root) = (|| {
        let raw = std::env::var("MEI_TEST_WORKSPACE").ok()?;
        let path = std::path::PathBuf::from(raw.trim());
        if path.as_os_str().is_empty() || !path.is_dir() {
            return None;
        }
        Some(path.canonicalize().unwrap_or(path))
    })() else {
        eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
        return;
    };
    let app_root = source_root.join("zhifa");
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/home.mei".to_string()),
            strict_layout_policy: true,
        },
    )
    .unwrap_or_else(|e| panic!("compile zhifa home failed: {e}"));
    let scene_contract = compiled
        .scene_contract
        .as_ref()
        .expect("home scene contract");
    let runtime_ctx =
        build_preview_runtime_context(&compiled, UiRouteMode::Layout, None, None, None, None, None);
    assert!(
        runtime_ctx.host_ssr_slim_payload,
        "build mode must enable host SSR slim payload"
    );
    let resolved_theme = theme::resolve_theme(scene_contract, None);
    let scene_anchor = RuntimeSceneAnchor {
        scene_id: scene_contract.scene.id.clone(),
        scene_path: Some("scenes/home.mei".to_string()),
    };
    let mut blocks = Vec::new();
    for panel in &scene_contract.panels {
        walk_blocks(&panel.blocks, &mut blocks);
    }
    let mut data_props_count = 0usize;
    let mut data_props_bytes = 0usize;
    let mut data_props_max_bytes = 0usize;
    for block in blocks {
        let resolved = resolve_value(
            &block.props,
            &resolved_theme.shared,
            scene_contract,
            &runtime_ctx.resources,
            &scene_anchor,
            &runtime_ctx.index,
            &compiled,
            runtime_ctx.host_ssr_slim_payload,
            None,
        );
        let props = attach_host_meta(
            resolved,
            &compiled,
            "zhifa",
            &resolved_theme.components,
            Some("scenes/home.mei"),
            HostMetaOptions::default(),
        );
        let tag = compiled
            .component_assets
            .iter()
            .find(|asset| asset.key == block.use_key)
            .map(|asset| asset.tag.as_str())
            .unwrap_or("mei-missing-component");
        let html = component_html(tag, &props);
        let payload_len = html
            .split_once("data-props=\"")
            .and_then(|(_, tail): (&str, &str)| tail.split_once('"'))
            .map(|(payload, _): (&str, &str)| payload.len())
            .unwrap_or(0);
        if payload_len > 0 {
            data_props_count += 1;
            data_props_bytes += payload_len;
            data_props_max_bytes = data_props_max_bytes.max(payload_len);
        }
    }
    eprintln!(
        "zhifa home data_props_count={data_props_count} bytes={} max={}",
        data_props_bytes, data_props_max_bytes
    );
    const FIVE_MB: usize = 5 * 1024 * 1024;
    assert!(
        data_props_bytes < FIVE_MB,
        "resolved data-props total {data_props_bytes} bytes exceeds 5MB (count={data_props_count}, max={data_props_max_bytes})"
    );
}

#[test]
#[ignore = "slow: full render_page HTML payload measurement for zhifa home build"]
fn zhifa_home_full_render_page_data_props_under_5mb() {
    use mei_lang_kernel::{
        compile_app_from_root_with_options, load_workspace_config, CompileOptions,
    };

    use crate::ui::route::UiRouteMode;
    use crate::ui::{page_body_theme_style, render_page};

    let Some(source_root) = (|| {
        let raw = std::env::var("MEI_TEST_WORKSPACE").ok()?;
        let path = std::path::PathBuf::from(raw.trim());
        if path.as_os_str().is_empty() || !path.is_dir() {
            return None;
        }
        Some(path.canonicalize().unwrap_or(path))
    })() else {
        eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
        return;
    };
    let app_root = source_root.join("zhifa");
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/home.mei".to_string()),
            strict_layout_policy: true,
        },
    )
    .unwrap_or_else(|e| panic!("compile zhifa home failed: {e}"));
    let workspace = load_workspace_config(&source_root);
    let shell_theme = page_body_theme_style(&workspace, Some(&compiled), None);
    let html = render_page(
        &[],
        &compiled,
        "zhifa",
        None,
        UiRouteMode::Layout,
        Some("scenes/home.mei"),
        Some(""),
        None,
        Some("home"),
        None,
        None,
        None,
        None,
        None,
        Some("scene:home"),
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
        shell_theme.as_str(),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        &[],
    );
    let mut data_props_count = 0usize;
    let mut data_props_bytes = 0usize;
    let mut data_props_max_bytes = 0usize;
    let mut search_from = 0usize;
    const ATTR: &str = "data-props=\"";
    while search_from < html.len() {
        let tail = &html[search_from..];
        let Some(rel) = tail.find(ATTR) else {
            break;
        };
        let payload_start = search_from + rel + ATTR.len();
        let payload = &html[payload_start..];
        let Some(end_rel) = payload.find('"') else {
            break;
        };
        data_props_count += 1;
        data_props_bytes += end_rel;
        data_props_max_bytes = data_props_max_bytes.max(end_rel);
        search_from = payload_start + end_rel + 1;
    }
    eprintln!(
        "zhifa full render_page html_bytes={} data_props_count={data_props_count} bytes={} max={}",
        html.len(),
        data_props_bytes,
        data_props_max_bytes
    );
    const FIVE_MB: usize = 5 * 1024 * 1024;
    assert!(
        data_props_bytes < FIVE_MB,
        "full render_page data-props {data_props_bytes} bytes exceeds 5MB"
    );
}
