use std::path::Path;

use mei_host_graph::{assemble_scope_from_registry, publish_app_data_snapshots};
use mei_lang_datasets::{evaluate_runtime_metrics, RuntimeMetricEvalMode};
use mei_lang_kernel::QueryState;

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
