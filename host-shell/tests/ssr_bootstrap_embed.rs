//! SSR client-bootstrap head fragment injection.

use std::collections::BTreeMap;

use mei_host_core::EvalSlotDescriptor;
use mei_host_graph::{
    build_client_bootstrap_head_fragment, record_slots_from_descriptors, write_client_bootstrap,
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

#[test]
fn ssr_bootstrap_head_fragment_contains_json_script_and_meta() {
    let temp = tempfile::tempdir().expect("tempdir");
    let workspace = temp.path();
    let app_root = workspace.join("apps").join("demo");
    std::fs::create_dir_all(app_root.join("var/active")).expect("var");

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
        64,
    )
    .expect("write manifest");
    record_slots_from_descriptors(workspace, "demo", std::slice::from_ref(&descriptor))
        .expect("record mrg slot");

    let fragment =
        build_client_bootstrap_head_fragment(workspace, "demo", "home").expect("fragment");
    assert!(fragment.contains(r#"id="mei-client-bootstrap""#));
    assert!(fragment.contains("mei-bootstrap-inlined"));
    assert!(fragment.contains("mei-bootstrap-metric-count"));
    assert!(fragment.contains("metric_a"));
    assert!(fragment.contains("bootstrap_metrics"));
    assert!(fragment.contains("bootstrapScopes"));
}
