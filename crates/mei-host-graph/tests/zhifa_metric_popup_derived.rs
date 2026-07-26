//! zhifa home metric cards must derive explain_metric popups from page_instance adjacency.

use std::path::PathBuf;
use std::sync::Once;

use mei_host_core::HostContext;
use mei_host_graph::{
    assemble_scope_from_registry, clear_assemble_cache_for_app, import_bundle, ImportOptions,
};
use mei_lang_kernel::{UiNodeDecl, UiTreeNode};
use serde_json::Value;

static INIT: Once = Once::new();

fn ws_demo_v2() -> Option<PathBuf> {
    mei_test_support::optional_external_workspace()
}

fn ensure_zhifa_imported() -> Option<PathBuf> {
    let workspace = ws_demo_v2()?;
    INIT.call_once(|| {
        let bundle = workspace.join("apps/zhifa/env/current/build/exchange/zhifa.meibundle");
        if !bundle.is_file() {
            return;
        }
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
    if !workspace
        .join("apps/zhifa/env/current/build/exchange/zhifa.meibundle")
        .is_file()
    {
        return None;
    }
    Some(workspace)
}

fn find_panel<'a>(panel: &'a UiNodeDecl, id: &str) -> Option<&'a UiNodeDecl> {
    if panel.id == id {
        return Some(panel);
    }
    for block in &panel.blocks {
        if let UiTreeNode::Panel(nested) = block {
            if let Some(found) = find_panel(nested, id) {
                return Some(found);
            }
        }
    }
    None
}

fn find_panel_in_tree<'a>(panels: &'a [UiNodeDecl], id: &str) -> Option<&'a UiNodeDecl> {
    panels.iter().find_map(|panel| find_panel(panel, id))
}

fn value_slot_popup(panel: &UiNodeDecl) -> Option<Value> {
    for node in &panel.blocks {
        if let UiTreeNode::Block(block) = node {
            if block.props.get("metric_role").and_then(|v| v.as_str()) == Some("value") {
                return block.props.get("popup").cloned();
            }
        }
    }
    None
}

#[test]
fn zhifa_warning_metric_cards_derive_read_only_analytics_popup() {
    let Some(workspace) = ensure_zhifa_imported() else {
        eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
        return;
    };
    let outcome = assemble_scope_from_registry(workspace.as_path(), "zhifa", "home")
        .expect("assemble")
        .expect("home outcome");
    let panels = &outcome
        .compiled
        .scene_contract
        .as_ref()
        .expect("scene contract")
        .panels;

    let expectations = [
        (
            "warnings",
            "warnings_analytics_page",
            "warning_list",
            "warnings_count",
        ),
        (
            "supervision_items",
            "supervision_items_analytics_page",
            "supervision_matters",
            "supervision_items_count",
        ),
        (
            "models",
            "supervision_models_analytics_page",
            "warning_models",
            "supervision_models_count",
        ),
    ];

    for (card_id, scene_id, rowset, metric_key) in expectations {
        let card = find_panel_in_tree(panels, card_id)
            .unwrap_or_else(|| panic!("missing metric card panel `{card_id}`"));
        let popup = value_slot_popup(card).unwrap_or_else(|| {
            panic!("missing value-slot popup on metric card `{card_id}`: {card:?}")
        });
        assert!(
            popup.get("__ref").and_then(|v| v.as_str()) != Some("link_ref"),
            "popup should be resolved for `{card_id}`, got {popup}"
        );
        assert_eq!(
            popup.get("scene_id").and_then(|v| v.as_str()),
            Some(scene_id),
            "unexpected scene for `{card_id}`: {popup}"
        );
        assert_eq!(
            popup
                .get("interaction")
                .and_then(|v| v.get("intent"))
                .and_then(|v| v.as_str()),
            Some("explain_metric"),
            "expected explain_metric intent for `{card_id}`: {popup}"
        );
        assert_eq!(
            popup.get("derived").and_then(|v| v.as_bool()),
            Some(true),
            "expected derived popup for `{card_id}`: {popup}"
        );
        assert_eq!(
            popup
                .get("params")
                .and_then(|v| v.get("rowset_dataset_id"))
                .and_then(|v| v.as_str()),
            Some(rowset),
            "unexpected rowset for `{card_id}`: {popup}"
        );
        let metric = popup
            .get("params")
            .and_then(|v| v.get("metric"))
            .expect("params.metric");
        assert_eq!(
            metric.get("id").and_then(|v| v.as_str()),
            Some(metric_key),
            "unexpected metric id for `{card_id}`: {metric}"
        );
    }
}

#[test]
fn zhifa_issue_metric_cards_derive_read_only_analytics_popup() {
    let Some(workspace) = ensure_zhifa_imported() else {
        eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
        return;
    };
    let outcome = assemble_scope_from_registry(workspace.as_path(), "zhifa", "home")
        .expect("assemble")
        .expect("home outcome");
    let panels = &outcome
        .compiled
        .scene_contract
        .as_ref()
        .expect("scene contract")
        .panels;

    let expectations = [
        (
            "pending",
            "issue_pending_analytics_page",
            "warning_list",
            "warnings_pending_count",
        ),
        (
            "doing",
            "issue_doing_analytics_page",
            "warning_list",
            "effectiveness_in_progress_count",
        ),
        (
            "done",
            "issue_done_analytics_page",
            "warning_list",
            "effectiveness_completed_count",
        ),
    ];

    for (card_id, scene_id, rowset, metric_key) in expectations {
        let card = find_panel_in_tree(panels, card_id)
            .unwrap_or_else(|| panic!("missing metric card panel `{card_id}`"));
        let popup = value_slot_popup(card)
            .unwrap_or_else(|| panic!("missing value-slot popup on metric card `{card_id}`"));
        assert_eq!(
            popup.get("scene_id").and_then(|v| v.as_str()),
            Some(scene_id),
            "unexpected scene for `{card_id}`: {popup}"
        );
        assert_eq!(
            popup
                .get("interaction")
                .and_then(|v| v.get("intent"))
                .and_then(|v| v.as_str()),
            Some("explain_metric"),
            "expected explain_metric intent for `{card_id}`: {popup}"
        );
        assert_eq!(
            popup
                .get("params")
                .and_then(|v| v.get("rowset_dataset_id"))
                .and_then(|v| v.as_str()),
            Some(rowset),
            "unexpected rowset for `{card_id}`: {popup}"
        );
        let metric = popup
            .get("params")
            .and_then(|v| v.get("metric"))
            .expect("params.metric");
        assert_eq!(
            metric.get("id").and_then(|v| v.as_str()),
            Some(metric_key),
            "unexpected metric id for `{card_id}`: {metric}"
        );
    }
}

#[test]
fn zhifa_warnings_analytics_local_nav_resolves_row_popup_and_field_links() {
    let Some(workspace) = ensure_zhifa_imported() else {
        eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
        return;
    };
    let outcome = assemble_scope_from_registry(workspace.as_path(), "zhifa", "home")
        .expect("assemble")
        .expect("home outcome");

    let nav = outcome
        .compiled
        .scene_local_nav_by_target
        .values()
        .find(|local_nav| {
            local_nav.get("scene_id").and_then(|v| v.as_str()) == Some("warnings_analytics_page")
        })
        .expect("warnings_analytics_page local_nav");

    let popup = nav.get("row_drilldown_popup").expect("row_drilldown_popup");
    assert_ne!(
        popup.get("__ref").and_then(|v| v.as_str()),
        Some("link_ref"),
        "row_drilldown_popup must be resolved, got {popup}"
    );
    assert_eq!(
        popup.get("scene_id").and_then(|v| v.as_str()),
        Some("warning_detail_page"),
        "expected warning detail scene, got {popup}"
    );
    assert!(
        popup
            .get("scene_file")
            .and_then(|v| v.as_str())
            .is_some_and(|s| !s.is_empty()),
        "scene_file required: {popup}"
    );

    let presentation = &outcome.presentation_map;
    let links = presentation
        .pointer("/objectFieldLinksByObjectType/zhifa.Warning")
        .expect("Warning objectFieldLinks");
    assert!(
        links
            .get("预警ID")
            .and_then(|v: &Value| v.as_array())
            .is_some_and(|a: &Vec<Value>| {
                a.len() == 1
                    && a[0].get("role").and_then(|r| r.as_str()) == Some("self")
                    && a[0].get("objectType").and_then(|r| r.as_str()) == Some("zhifa.Warning")
            }),
        "预警ID must open Warning detail only (no IssueResult chooser): {links}"
    );
    assert!(
        links.get("预警模型").is_some(),
        "预警模型 mapping links required: {links}"
    );
    let category_links = links
        .get("预警模型")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    assert!(
        category_links.iter().any(|link| {
            link.get("objectType").and_then(|v| v.as_str()) == Some("zhifa.AlertModel")
        }),
        "预警模型 → AlertModel mapping required: {links}"
    );
    assert!(
        category_links.iter().all(|link| {
            link.get("objectType").and_then(|v| v.as_str()) != Some("zhifa.SupervisionMatter")
        }),
        "预警模型 must not dual-link SupervisionMatter: {links}"
    );
}
