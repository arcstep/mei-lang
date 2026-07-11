use super::support::*;

#[test]
fn query_world_dataset_contract_shape_is_stable() {
    let root = workspaces_root();
    clear_compile_cache_for_app(&root, DATASET_APP);
    let payload = query_world_dataset(
        &root,
        DATASET_APP,
        None,
        "sales_data",
        None,
        &BTreeMap::new(),
        None,
        None,
        None,
    )
    .expect("dataset query");
    assert_eq!(payload["id"], "sales_data");
    assert!(payload["sample_rows"].is_array());
    assert!(payload["dataset"]["schema_preview"].is_array());
    assert!(payload["observation"]["exposure"]["query_schema_version"]
        .as_str()
        .is_some_and(|version| version.contains(RESOURCE_QUERY_SCHEMA_VERSION)));
    assert!(payload["perf"]["total_ms"].as_u64().is_some());
}

#[test]
fn query_world_dataset_metrics_contract_shape_is_stable() {
    let root = workspaces_root();
    clear_compile_cache_for_app(&root, METRIC_APP);
    let payload = query_world_dataset_metrics(
        &root,
        METRIC_APP,
        None,
        "orders",
        &["orders_overview".to_string()],
        None,
        &BTreeMap::new(),
        None,
        &[],
    )
    .expect("metric query");
    assert_eq!(payload["dataset_id"], "orders");
    assert!(payload["metrics"].is_array());
    assert!(!payload["metrics"].as_array().unwrap().is_empty());
    assert!(payload["analysis_contracts"].is_object() || payload["analysis_contracts"].is_array());
    assert!(payload["observation"]["compile"]["compile_ms"]
        .as_u64()
        .is_some());
    assert!(payload["perf"]["metric_eval_ms"].as_u64().is_some());
}

#[test]
fn runtime_sim_step_returns_scene_view_and_html() {
    let root = workspaces_root();
    clear_compile_cache_for_app(&root, RUNTIME_APP);
    let result = runtime_sim_step(
        &root,
        RUNTIME_APP,
        None,
        RuntimeIntent {
            kind: "sync".to_string(),
            target: None,
        },
    )
    .expect("runtime sim");
    assert!(!result.html.is_empty());
    assert!(!result.scene_view.scene_id.is_empty());
}
