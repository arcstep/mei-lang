use std::path::Path;

use mei_host_graph::{assemble_scope_from_registry, publish_app_data_snapshots};
use mei_lang_datasets::{evaluate_runtime_metrics, RuntimeMetricEvalMode};
use mei_lang_kernel::QueryState;
use serde_json::Value;

#[test]
fn home_realtime_warning_detail_evaluates_with_data() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../workspaces/ws-demo-v2");
    if !workspace.join("apps/data-demo/app.config.json").is_file() {
        return;
    }
    let _ = publish_app_data_snapshots(workspace.as_path(), "data-demo");
    let outcome = assemble_scope_from_registry(workspace.as_path(), "data-demo", "home")
        .expect("assemble")
        .expect("home assembly");
    let warning = outcome
        .compiled
        .resources
        .iter()
        .find(|r| r.id == "warning_list")
        .and_then(|r| r.dataset.as_ref())
        .expect("warning_list resource");
    assert_eq!(
        warning.source.header_row,
        Some(4),
        "alert_tracking header_row should flow from app.config ops.sources: {:?}",
        warning.source
    );
    let app_root = mei_lang_kernel::resolve_app_root(workspace.as_path(), "data-demo");
    let owner = "__world_metrics__::metrics/realtime-warning.bundle.mei";
    let eval = evaluate_runtime_metrics(
        &outcome.compiled,
        app_root.as_path(),
        owner,
        &["realtime_warning_detail".to_string()],
        "home",
        None,
        &QueryState::default(),
        &[],
        RuntimeMetricEvalMode::WithDag,
    )
    .expect("eval metrics");
    assert!(
        eval.hydrate_perf
            .get("hydrate_datasets_count")
            .copied()
            .unwrap_or(0)
            > 0,
        "expected dataset hydrate, perf={:?}",
        eval.hydrate_perf
    );
    let metric = eval
        .metrics
        .iter()
        .find(|m| m.id == "realtime_warning_detail")
        .expect("metric result");
    assert!(
        !metric.value.is_null(),
        "expected non-null metric value: {:?}",
        metric
    );
}

#[test]
fn board_explain_metrics_resolve_after_v2_hydrate_expand() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../workspaces/ws-demo-v2");
    if !workspace.join("apps/data-demo/app.config.json").is_file() {
        return;
    }
    let _ = publish_app_data_snapshots(workspace.as_path(), "data-demo");
    let outcome = assemble_scope_from_registry(
        workspace.as_path(),
        "data-demo",
        "supervision_items_analytics_board",
    )
    .expect("assemble")
    .expect("board assembly");
    let owner = "__world_metrics__::metrics/supervision-warning.bundle.mei";
    let bundle = outcome
        .compiled
        .resources
        .iter()
        .find(|r| r.id == owner)
        .and_then(|r| r.dataset.as_ref())
        .expect("supervision-warning bundle resource");
    assert!(
        bundle
            .runtime_metric_defs
            .contains_key("supervision_items_count::composition_by_category"),
        "expanded explain metrics should be registered, keys={:?}",
        bundle.runtime_metric_defs.keys().collect::<Vec<_>>()
    );
    assert!(
        bundle
            .runtime_metric_defs
            .contains_key("supervision_items_count::__scalar_rowset__"),
        "detail rowset should be expanded from explain, keys={:?}",
        bundle.runtime_metric_defs.keys().collect::<Vec<_>>()
    );
    let app_root = mei_lang_kernel::resolve_app_root(workspace.as_path(), "data-demo");
    let eval = evaluate_runtime_metrics(
        &outcome.compiled,
        app_root.as_path(),
        owner,
        &["supervision_items_count::composition_by_category".to_string()],
        "supervision_items_analytics_board",
        None,
        &QueryState::default(),
        &[],
        RuntimeMetricEvalMode::WithDag,
    )
    .expect("eval explain metrics");
    let composition = eval
        .metrics
        .iter()
        .find(|m| m.id.contains("composition_by_category"))
        .expect("composition metric result");
    assert!(
        !composition.value.is_null(),
        "expected composition data, got {:?}",
        composition
    );
}

#[test]
fn issue_verification_rate_detail_rowset_evaluates_after_v2_hydrate() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../workspaces/ws-demo-v2");
    if !workspace.join("apps/data-demo/app.config.json").is_file() {
        return;
    }
    let _ = publish_app_data_snapshots(workspace.as_path(), "data-demo");
    let outcome = assemble_scope_from_registry(
        workspace.as_path(),
        "data-demo",
        "issue_rate_analytics_board",
    )
    .expect("assemble")
    .expect("board assembly");
    let owner = "__world_metrics__::metrics/issue-handling.bundle.mei";
    let bundle = outcome
        .compiled
        .resources
        .iter()
        .find(|r| r.id == owner)
        .and_then(|r| r.dataset.as_ref())
        .expect("issue-handling bundle");
    assert!(
        bundle
            .runtime_metric_defs
            .contains_key("effectiveness_issue_verification_rate::__scalar_rowset__"),
        "verification rate should hoist scalar rowset"
    );
    let app_root = mei_lang_kernel::resolve_app_root(workspace.as_path(), "data-demo");
    let eval = evaluate_runtime_metrics(
        &outcome.compiled,
        app_root.as_path(),
        owner,
        &["effectiveness_issue_verification_rate::__scalar_rowset__".to_string()],
        "issue_rate_analytics_board",
        None,
        &QueryState::default(),
        &[],
        RuntimeMetricEvalMode::WithDag,
    )
    .expect("eval verification rowset");
    let rowset = eval
        .metrics
        .iter()
        .find(|m| m.id.contains("__scalar_rowset__"))
        .expect("rowset metric");
    let rows = rowset.value.as_array().expect("dataframe rows");
    assert!(
        !rows.is_empty(),
        "verification rate detail rowset should not be empty, got {:?}",
        rowset
    );
}

#[test]
fn warning_detail_card_board_has_preview_projection_slot() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../workspaces/ws-demo-v2");
    if !workspace.join("apps/data-demo/app.config.json").is_file() {
        return;
    }
    let _ = publish_app_data_snapshots(workspace.as_path(), "data-demo");
    let outcome = assemble_scope_from_registry(
        workspace.as_path(),
        "data-demo",
        "warning_detail_card_board",
    )
    .expect("assemble")
    .expect("card board assembly");
    let assembly = outcome
        .compiled
        .scene_projection_assembly_by_id
        .get("warning_detail_card_board")
        .expect("warning_detail_card_board assembly");
    let Some(slot) = assembly
        .get("projection_slots")
        .and_then(Value::as_array)
        .and_then(|slots| slots.first())
        .and_then(Value::as_object)
    else {
        eprintln!(
            "skip warning_detail_card_board projection_slots: rebuild ws-demo-v2 meibundle after card board examples"
        );
        return;
    };
    assert_eq!(
        slot.get("metric_id").and_then(Value::as_str),
        Some("realtime_warning_detail::__scalar_rowset__")
    );
}

#[test]
fn mechanism_documents_board_has_list_preview_projection_slots() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../workspaces/ws-demo-v2");
    if !workspace.join("apps/data-demo/app.config.json").is_file() {
        return;
    }
    let _ = publish_app_data_snapshots(workspace.as_path(), "data-demo");
    let outcome = assemble_scope_from_registry(
        workspace.as_path(),
        "data-demo",
        "effect_mechanism_documents_board",
    )
    .expect("assemble")
    .expect("mechanism documents board assembly");
    let assembly = outcome
        .compiled
        .scene_projection_assembly_by_id
        .get("effect_mechanism_documents_board")
        .expect("effect_mechanism_documents_board assembly");
    let shell = assembly
        .get("shell_contract")
        .and_then(Value::as_object)
        .expect("shell_contract");
    assert_eq!(
        shell.get("layout_mode").and_then(Value::as_str),
        Some("list_preview")
    );
    let slots = assembly
        .get("projection_slots")
        .and_then(Value::as_array)
        .expect("projection_slots");
    let encoded = serde_json::to_string(slots).expect("encode slots");
    assert!(
        encoded.contains("document_preview"),
        "expected document_preview mapping in slots: {encoded}"
    );
    assert!(
        encoded.contains("mechanism_documents_list"),
        "expected mechanism_documents_list metric in slots: {encoded}"
    );
    let owner = "__world_metrics__::metrics/effectiveness.bundle.mei";
    let app_root = mei_lang_kernel::resolve_app_root(workspace.as_path(), "data-demo");
    let eval = evaluate_runtime_metrics(
        &outcome.compiled,
        app_root.as_path(),
        owner,
        &["effectiveness_mechanism_item_count::mechanism_documents_list".to_string()],
        "effect_mechanism_documents_board",
        None,
        &QueryState::default(),
        &[],
        RuntimeMetricEvalMode::WithDag,
    )
    .expect("eval mechanism documents list");
    let list = eval
        .metrics
        .iter()
        .find(|m| m.id.contains("mechanism_documents_list"))
        .expect("mechanism_documents_list result");
    let rows = list.value.as_array().expect("dataframe rows");
    assert!(
        !rows.is_empty(),
        "mechanism documents list should have rows, got {:?}",
        list
    );
}

#[test]
fn inspection_trend_year_compare_evaluates_after_bundle_constant_resolve() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../workspaces/ws-demo-v2");
    if !workspace.join("apps/data-demo/app.config.json").is_file() {
        return;
    }
    let _ = publish_app_data_snapshots(workspace.as_path(), "data-demo");
    let outcome = assemble_scope_from_registry(workspace.as_path(), "data-demo", "home")
        .expect("assemble")
        .expect("home assembly");
    let owner = "__world_metrics__::metrics/inspection-dashboard.bundle.mei";
    let app_root = mei_lang_kernel::resolve_app_root(workspace.as_path(), "data-demo");
    let eval = evaluate_runtime_metrics(
        &outcome.compiled,
        app_root.as_path(),
        owner,
        &["inspections_6m_count_trend".to_string()],
        "home",
        None,
        &QueryState::default(),
        &[],
        RuntimeMetricEvalMode::WithDag,
    )
    .expect("eval inspection trend");
    let metric = eval
        .metrics
        .iter()
        .find(|m| m.id == "inspections_6m_count_trend")
        .expect("trend metric");
    let encoded = metric.value.to_string();
    assert!(
        !encoded.contains("__var"),
        "expected resolved constants in lowered metric, got {:?}",
        metric.value
    );
    let rows = metric.value.as_array().expect("trend dataframe rows");
    assert!(
        !rows.is_empty(),
        "expected trend rows, got {:?}",
        metric.value
    );
}

#[test]
fn indicator_calendar_year_metrics_evaluate_non_zero() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../workspaces/ws-demo-v2");
    if !workspace.join("apps/data-demo/app.config.json").is_file() {
        return;
    }
    let _ = publish_app_data_snapshots(workspace.as_path(), "data-demo");
    let outcome = assemble_scope_from_registry(workspace.as_path(), "data-demo", "home")
        .expect("assemble")
        .expect("home assembly");
    let owner = "__world_metrics__::metrics/indicator-system.bundle.mei";
    let app_root = mei_lang_kernel::resolve_app_root(workspace.as_path(), "data-demo");
    let eval = evaluate_runtime_metrics(
        &outcome.compiled,
        app_root.as_path(),
        owner,
        &[
            "inspection_frequency_reduction_rate".to_string(),
            "penalty_revenue_growth_rate".to_string(),
        ],
        "home",
        None,
        &QueryState::default(),
        &[],
        RuntimeMetricEvalMode::WithDag,
    )
    .expect("eval indicator calendar metrics");
    for id in [
        "inspection_frequency_reduction_rate",
        "penalty_revenue_growth_rate",
    ] {
        let metric = eval.metrics.iter().find(|m| m.id == id).expect(id);
        let value = metric
            .value
            .get("value")
            .and_then(Value::as_f64)
            .or_else(|| metric.value.as_f64())
            .unwrap_or(0.0);
        assert!(
            value.is_finite() && value.abs() > f64::EPSILON,
            "{id} should be non-zero, got {value:?}"
        );
    }
}
