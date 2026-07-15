//! Platform conformance: metric eval on fx-data fixture.

use mei_host_graph::{assemble_scope_from_registry, publish_app_data_snapshots};
use mei_lang_datasets::{evaluate_runtime_metrics, RuntimeMetricEvalMode};
use mei_lang_kernel::QueryState;
use mei_test_support::{ensure_imported, APP_DATA};

#[test]
fn conformance_metric_eval_nonzero() {
    let workspace = ensure_imported(APP_DATA);
    let _ = publish_app_data_snapshots(workspace.as_path(), APP_DATA);
    let outcome = assemble_scope_from_registry(workspace.as_path(), APP_DATA, "home")
        .expect("assemble")
        .expect("home assembly");
    let owner = "__world_metrics__::metrics/fx-rows.bundle.mei";
    let app_root = mei_lang_kernel::resolve_app_root(workspace.as_path(), APP_DATA);
    let eval = evaluate_runtime_metrics(
        &outcome.compiled,
        app_root.as_path(),
        owner,
        &["fx_rows_count".to_string()],
        "home",
        None,
        &QueryState::default(),
        &[],
        RuntimeMetricEvalMode::WithDag,
    )
    .expect("eval metrics");
    let metric = eval
        .metrics
        .iter()
        .find(|m| m.id == "fx_rows_count")
        .expect("metric result");
    assert!(
        !metric.value.is_null(),
        "expected non-null metric value: {:?}",
        metric
    );
}
