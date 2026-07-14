use std::path::Path;

use mei_host_graph::assemble_scope_from_registry;
use mei_lang_datasets::{evaluate_runtime_metrics, RuntimeMetricEvalMode};
use mei_lang_kernel::QueryState;

#[test]
fn thunder_devices_online_evaluates_from_app_toml_fixture() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../workspaces/ws-demo-v2");
    let app = workspace.join("apps/thunder");
    assert!(app.join("app.toml").is_file(), "thunder app.toml missing");
    assert!(
        !app.join("app.config.json").is_file(),
        "thunder should be toml-only for this regression"
    );

    let outcome = assemble_scope_from_registry(workspace.as_path(), "thunder", "home")
        .expect("assemble")
        .expect("home assembly");

    let dataset = outcome.compiled.resources.iter().find_map(|r| {
        let d = r.dataset.as_ref()?;
        if d.id == "storm_metrics" || d.source.path.contains("storm-events") {
            Some(d)
        } else {
            None
        }
    });
    let Some(ds) = dataset else {
        panic!("storm_metrics dataset missing from assembled resources");
    };
    assert_eq!(
        ds.source.path, "prototype/storm-events.fixture.json",
        "ops.sources path must resolve from app.toml"
    );
    assert_eq!(ds.source.kind, "json");

    let app_root = mei_lang_kernel::resolve_app_root(workspace.as_path(), "thunder");
    let owner = "__world_metrics__::metrics/storm-events.bundle.mei";
    let eval = evaluate_runtime_metrics(
        &outcome.compiled,
        app_root.as_path(),
        owner,
        &["devices_online".to_string(), "storm_events_total".to_string()],
        "home",
        None,
        &QueryState::default(),
        &[],
        RuntimeMetricEvalMode::WithDag,
    )
    .expect("eval metrics");

    let devices = eval
        .metrics
        .iter()
        .find(|m| m.id == "devices_online")
        .expect("devices_online metric");
    eprintln!("devices_online={:?}", devices.value);
    let number = metrics_value_number(&devices.value);
    assert!(
        number > 0.0,
        "devices_online should be >0 after toml hydrate, got {:?}",
        devices.value
    );
}

fn metrics_value_number(value: &serde_json::Value) -> f64 {
    if let Some(n) = value.as_f64() {
        return n;
    }
    if let Some(n) = value.as_i64() {
        return n as f64;
    }
    if let Some(obj) = value.as_object() {
        if let Some(inner) = obj.get("value") {
            return metrics_value_number(inner);
        }
    }
    0.0
}
