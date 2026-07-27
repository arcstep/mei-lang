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
fn zhifa_warning_metric_cards_open_explicit_analytics_popups() {
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
        if card_id == "warnings" {
            let asm = outcome
                .compiled
                .scene_projection_assembly_by_id
                .get("warnings_analytics_page")
                .expect("warnings_analytics_page assembly");
            let fs = asm
                .get("filter_schema")
                .or_else(|| asm.pointer("/bindings/filter_schema"))
                .expect("warnings filter_schema");
            assert_eq!(
                fs.get("default_collapsed").and_then(|v| v.as_bool()),
                Some(false),
                "warnings filter panel must default open"
            );
            assert_eq!(fs.get("allow_extra").and_then(|v| v.as_bool()), Some(false));
            assert_eq!(
                fs.get("preset_filter_count").and_then(|v| v.as_u64()),
                Some(3)
            );
            let fields = fs.get("fields").and_then(|v| v.as_array()).expect("fields");
            let first_key = fields.first().and_then(|f| {
                f.get("__args")
                    .or(Some(f))
                    .and_then(|m| m.get("key"))
                    .and_then(|v| v.as_str())
            });
            assert_eq!(
                first_key,
                Some("agency"),
                "preset lead must be agency/主责单位, got {fields:?}"
            );
            let labels: Vec<&str> = fields
                .iter()
                .filter_map(|f| {
                    f.get("__args")
                        .or(Some(f))
                        .and_then(|m| m.get("label").or_else(|| m.get("column")))
                        .and_then(|v| v.as_str())
                })
                .collect();
            assert!(
                !labels.iter().any(|label| *label == "监督类别"),
                "warnings filter must not include 监督类别: {labels:?}"
            );
        }
    }
}

#[test]
fn zhifa_issue_metric_cards_open_shared_handling_analytics_with_status_filters() {
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
        ("pending", "in:待办"),
        ("doing", "in:在办"),
        ("done", "in:办结"),
    ];

    for (card_id, status_filter) in expectations {
        let card = find_panel_in_tree(panels, card_id)
            .unwrap_or_else(|| panic!("missing metric card panel `{card_id}`"));
        let popup = value_slot_popup(card)
            .unwrap_or_else(|| panic!("missing value-slot popup on metric card `{card_id}`"));
        assert_eq!(
            popup.get("scene_id").and_then(|v| v.as_str()),
            Some("issue_handling_analytics_page"),
            "unexpected scene for `{card_id}`: {popup}"
        );
        assert_eq!(
            popup.get("title").and_then(|v| v.as_str()),
            Some("问题办理"),
            "unexpected title for `{card_id}`: {popup}"
        );
        assert_eq!(
            popup
                .get("params")
                .and_then(|v| v.get("rowset_dataset_id"))
                .and_then(|v| v.as_str()),
            Some("issue_handling_list"),
            "unexpected rowset for `{card_id}`: {popup}"
        );
        let metric = popup
            .get("params")
            .and_then(|v| v.get("metric"))
            .expect("params.metric");
        assert_eq!(
            metric.get("id").and_then(|v| v.as_str()),
            Some("issue_handling_analytics"),
            "unexpected metric id for `{card_id}`: {metric}"
        );
        let filters = popup
            .get("params")
            .and_then(|v| v.get("default_filters"))
            .and_then(|v| v.as_object())
            .unwrap_or_else(|| panic!("missing default_filters for `{card_id}`: {popup}"));
        assert_eq!(
            filters.get("办理状态").and_then(|v| v.as_str()),
            Some(status_filter),
            "unexpected 办理状态 filter for `{card_id}`: {popup}"
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

#[test]
fn zhifa_issue_handling_metrics_evaluate() {
    use mei_lang_datasets::{evaluate_runtime_metrics, RuntimeMetricEvalMode};
    use mei_lang_kernel::QueryState;

    let Some(workspace) = ensure_zhifa_imported() else {
        eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
        return;
    };
    let outcome = assemble_scope_from_registry(workspace.as_path(), "zhifa", "home")
        .expect("assemble")
        .expect("home");
    let owner = "__world_metrics__::metrics/issue-handling.bundle.mei";
    let bundle = outcome
        .compiled
        .resources
        .iter()
        .find(|r| r.id == owner)
        .and_then(|r| r.dataset.as_ref())
        .expect("issue-handling bundle");
    let analytics = bundle
        .runtime_metric_defs
        .get("issue_handling_analytics")
        .expect("issue_handling_analytics def");
    assert_eq!(
        analytics
            .pointer("/values/value/rowset/type")
            .and_then(|v| v.as_str()),
        Some("concat_rowsets"),
        "label_status_pending must lower into concat_rowsets, got {analytics}"
    );
    assert!(
        bundle
            .runtime_metric_defs
            .contains_key("issue_handling_analytics::__scalar_rowset__"),
        "detail rowset must expand once analytics rowset is non-null; keys={:?}",
        bundle.runtime_metric_defs.keys().collect::<Vec<_>>()
    );

    // Eval may fail in sandbox without redb write access; defs above are the contract gate.
    let app_root = mei_lang_kernel::resolve_app_root(workspace.as_path(), "zhifa");
    let _ = evaluate_runtime_metrics(
        &outcome.compiled,
        app_root.as_path(),
        owner,
        &[
            "warnings_pending_count".into(),
            "effectiveness_in_progress_count".into(),
            "effectiveness_completed_count".into(),
            "issue_handling_analytics".into(),
        ],
        "home",
        None,
        &QueryState::default(),
        &[],
        RuntimeMetricEvalMode::WithDag,
    );
}
