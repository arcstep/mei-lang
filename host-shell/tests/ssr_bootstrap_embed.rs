//! SSR client-bootstrap head fragment injection.

use std::collections::BTreeMap;
use std::path::Path;

use mei_host_core::EvalSlotDescriptor;
use mei_host_graph::{
    bootstrap_embed_status, build_client_bootstrap_head_fragment,
    record_slots_from_descriptors, write_client_bootstrap,
};
use mei_lang_kernel::{MetricContract, MetricShape};

fn sample_descriptor(slot_key: &str, content_hash: &str) -> EvalSlotDescriptor {
    EvalSlotDescriptor {
        slot_key: slot_key.to_string(),
        scope_key: "home".to_string(),
        owner_resource_id: "__world_metrics__::metrics/demo.bundle.mei".to_string(),
        metric_def_bundle_revision: "bundle".to_string(),
        data_source_revision: "ds".to_string(),
        payload_kind: "metric_response".to_string(),
        content_hash: content_hash.to_string(),
        schema_version: "mei-metric-response-result-artifact-v1".to_string(),
        wall_ms: 1,
        artifact_hit: true,
        workset_id: "workset:home:0".to_string(),
        cache_layer: "client".to_string(),
        cache_layers_ready: mei_host_core::CacheLayersReady {
            disk: true,
            memory: true,
            client: true,
        },
        client_revision: None,
        resident_tier: "memory_resident".to_string(),
        client_eligible: true,
        payload_bytes: None,
    }
}

fn seed_test_app_env(app_root: &Path) {
    std::fs::create_dir_all(app_root.join("var/active")).expect("var/active");
    let env_id = "WS-20260228.0";
    let env_dir = app_root.join("env").join(env_id);
    std::fs::create_dir_all(env_dir.join("build")).expect("build");
    std::fs::create_dir_all(env_dir.join("var")).expect("var");
    let current = app_root.join("env/current");
    if current.exists() {
        std::fs::remove_file(&current).ok();
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink(env_id, &current).expect("symlink env/current");
    #[cfg(windows)]
    std::os::windows::fs::symlink_dir(env_id, &current).expect("symlink env/current");
}

#[test]
fn ssr_bootstrap_head_fragment_contains_json_script_and_meta() {
    let temp = tempfile::tempdir().expect("tempdir");
    let workspace = temp.path();
    let app_root = workspace.join("apps").join("demo");
    seed_test_app_env(app_root.as_path());

    let mut metrics = BTreeMap::new();
    metrics.insert(
        "metric_a".to_string(),
        MetricContract {
            id: "metric_a".to_string(),
            label: None,
            unit: None,
            value_format: None,
            purpose: None,
            shape: MetricShape::Scalar,
            schema: vec![],
            dataset: None,
            transforms: vec![],
            value: serde_json::json!(99),
        },
    );
    let descriptor = sample_descriptor("workset:home:0::metric_a", "hash-a");
    write_client_bootstrap(
        app_root.as_path(),
        "demo",
        "home",
        "workset:home:0",
        std::slice::from_ref(&descriptor),
        &metrics,
        &BTreeMap::new(),
        64,
    )
    .expect("write manifest");
    record_slots_from_descriptors(workspace, "demo", std::slice::from_ref(&descriptor))
        .expect("record mrg slot");

    let fragment =
        build_client_bootstrap_head_fragment(workspace, "demo", "home").expect("fragment");
    assert!(fragment.contains(r#"mei-bootstrap-inlined" content="0""#));
    assert!(fragment.contains("mei-bootstrap-client-revision"));
    assert!(fragment.contains("mei-bootstrap-artifact-url"));
    assert!(fragment.contains("mei:scene-bootstrap:v1:"));
    assert!(fragment.contains("localStorage.getItem"));
    assert!(!fragment.contains(r#"id="mei-client-bootstrap""#));
}

#[test]
fn ssr_bootstrap_head_fragment_inline_mode_contains_json_script() {
    let temp = tempfile::tempdir().expect("tempdir");
    let workspace = temp.path();
    let app_root = workspace.join("apps").join("demo");
    seed_test_app_env(app_root.as_path());
    std::fs::write(
        app_root.join("app.config.json"),
        r#"{"runtime":{"clientBootstrap":{"enabled":true,"embedMode":"inline"}}}"#,
    )
    .expect("app.config");

    let mut metrics = BTreeMap::new();
    metrics.insert(
        "metric_a".to_string(),
        MetricContract {
            id: "metric_a".to_string(),
            label: None,
            unit: None,
            value_format: None,
            purpose: None,
            shape: MetricShape::Scalar,
            schema: vec![],
            dataset: None,
            transforms: vec![],
            value: serde_json::json!(99),
        },
    );
    let descriptor = sample_descriptor("workset:home:0::metric_a", "hash-a");
    write_client_bootstrap(
        app_root.as_path(),
        "demo",
        "home",
        "workset:home:0",
        std::slice::from_ref(&descriptor),
        &metrics,
        &BTreeMap::new(),
        64,
    )
    .expect("write manifest");
    record_slots_from_descriptors(workspace, "demo", std::slice::from_ref(&descriptor))
        .expect("record mrg slot");

    let fragment =
        build_client_bootstrap_head_fragment(workspace, "demo", "home").expect("fragment");
    assert!(fragment.contains(r#"id="mei-client-bootstrap""#));
    assert!(fragment.contains(r#"mei-bootstrap-inlined" content="1""#));
    assert!(fragment.contains("metric_a"));
}

#[test]
fn bootstrap_embed_status_reports_revision_mismatch() {
    let temp = tempfile::tempdir().expect("tempdir");
    let workspace = temp.path();
    let app_root = workspace.join("apps").join("demo");
    seed_test_app_env(app_root.as_path());
    let mut metrics = BTreeMap::new();
    metrics.insert(
        "metric_a".to_string(),
        MetricContract {
            id: "metric_a".to_string(),
            label: None,
            unit: None,
            value_format: None,
            purpose: None,
            shape: MetricShape::Scalar,
            schema: vec![],
            dataset: None,
            transforms: vec![],
            value: serde_json::json!(1),
        },
    );
    let descriptor = sample_descriptor("workset:home:0::metric_a", "hash-a");
    let manifest = write_client_bootstrap(
        app_root.as_path(),
        "demo",
        "home",
        "workset:home:0",
        std::slice::from_ref(&descriptor),
        &metrics,
        &BTreeMap::new(),
        64,
    )
    .expect("write")
    .expect("manifest");
    record_slots_from_descriptors(workspace, "demo", std::slice::from_ref(&descriptor))
        .expect("record mrg slot");
    let mut stale_manifest = manifest.clone();
    stale_manifest.client_revision = "stale-revision".to_string();
    std::fs::write(
        mei_host_graph::client_bootstrap_path(app_root.as_path(), "home"),
        serde_json::to_string_pretty(&stale_manifest).expect("json"),
    )
    .expect("write stale manifest");
    let status = bootstrap_embed_status(workspace, "demo", "home");
    assert!(!status.allowed);
    assert_eq!(status.reason, "revision_mismatch");
    assert!(status.expected_revision.is_some());
}

#[test]
fn bootstrap_embed_status_reports_manifest_missing() {
    let temp = tempfile::tempdir().expect("tempdir");
    let app_root = temp.path().join("apps").join("demo");
    seed_test_app_env(app_root.as_path());
    let status = bootstrap_embed_status(temp.path(), "demo", "home");
    assert!(!status.allowed);
    assert_eq!(status.reason, "manifest_missing");
}
