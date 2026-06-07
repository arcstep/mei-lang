use std::collections::BTreeMap;
use std::path::PathBuf;

use mei_lang_kernel::CompileOptions;
use mei_lang_toolchain::{
    clear_compile_cache_for_app, compile_app_with_cache, compile_report, query_world_dataset,
    query_world_dataset_metrics, resolve_components_root, runtime_sim_step, RESOURCE_QUERY_SCHEMA_VERSION,
};
use mei_lang_kernel::RuntimeIntent;

fn workspaces_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../workspaces")
        .canonicalize()
        .expect("workspaces root")
}

const DATASET_APP: &str = "examples/ds/01-dataset-baseline";
const METRIC_APP: &str = "examples/ds/04-data-table-features";
const RUNTIME_APP: &str = "examples/sim/01-fire-baseline";

#[test]
fn compile_service_reports_cache_hit_on_second_request() {
    let root = workspaces_root();
    clear_compile_cache_for_app(&root, DATASET_APP);
    let components = resolve_components_root(&root);
    let options = CompileOptions::default();
    let first = compile_app_with_cache(&root, DATASET_APP, options.clone(), components.as_path())
        .map_err(|failure| failure.error)
        .expect("first");
    let second = compile_app_with_cache(&root, DATASET_APP, options, components.as_path())
        .map_err(|failure| failure.error)
        .expect("second");
    assert!(second.cache_hit, "second compile should hit cache");
    assert_eq!(first.compile_revision, second.compile_revision);
}

#[test]
fn clear_compile_cache_for_app_invalidates_cache_hit() {
    let root = workspaces_root();
    let components = resolve_components_root(&root);
    let options = CompileOptions::default();
    clear_compile_cache_for_app(&root, DATASET_APP);
    let _ = compile_app_with_cache(&root, DATASET_APP, options.clone(), components.as_path())
        .map_err(|failure| failure.error)
        .expect("warm");
    let cleared = clear_compile_cache_for_app(&root, DATASET_APP);
    assert!(cleared >= 1, "expected at least one cache entry cleared");
    let after_clear = compile_app_with_cache(&root, DATASET_APP, options, components.as_path())
        .map_err(|failure| failure.error)
        .expect("after clear");
    assert!(!after_clear.cache_hit, "compile after clear should miss cache");
}

#[test]
fn compile_report_revision_matches_cached_outcome() {
    let root = workspaces_root();
    clear_compile_cache_for_app(&root, DATASET_APP);
    let report = compile_report(&root, DATASET_APP, CompileOptions::default()).expect("report");
    assert!(!report.revision_token.is_empty());
    let cached = compile_app_with_cache(
        &root,
        DATASET_APP,
        CompileOptions::default(),
        resolve_components_root(&root).as_path(),
    )
    .map_err(|failure| failure.error)
    .expect("cached");
    assert_eq!(report.revision_token, cached.compile_revision);
    let second = compile_report(&root, DATASET_APP, CompileOptions::default()).expect("second report");
    assert!(second.cache_hit);
    assert_eq!(report.revision_token, second.revision_token);
}

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
    )
    .expect("metric query");
    assert_eq!(payload["dataset_id"], "orders");
    assert!(payload["metrics"].is_array());
    assert!(!payload["metrics"].as_array().unwrap().is_empty());
    assert!(payload["analysis_contracts"].is_object() || payload["analysis_contracts"].is_array());
    assert!(payload["observation"]["compile"]["compile_ms"].as_u64().is_some());
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
