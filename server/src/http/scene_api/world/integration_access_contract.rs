//! Compile → dataset_query / dataset_metric integration for access-side analysis_contract.

use std::collections::BTreeMap;
use std::path::PathBuf;

use mei_lang_kernel::locate_dataset_resource;
use serde_json::Value;

use crate::http::scene_api::types::WorldScope;
use crate::test_support;

use super::bundle::load_world_runtime_bundle;
use super::dataset_llm::{query_world_dataset, query_world_dataset_metrics};
use super::snapshot::build_prompt_catalog_lines;

const DS04_APP: &str = "examples/ds/04-data-table-features";
const DS04_SCENE: &str = "metric_explain_access";

fn workspaces_root() -> PathBuf {
    test_support::test_app_state()
        .expect("workspaces test state")
        .source_root
        .as_ref()
        .clone()
}

fn metric_explain_scope() -> WorldScope {
    WorldScope {
        scene_id: Some(DS04_SCENE.to_string()),
        target_file: None,
    }
}

fn contract_present(payload: &Value, metric_id: &str) -> bool {
    if let Some(present) = payload
        .pointer(&format!("/analysis_contracts/{metric_id}/present"))
        .and_then(Value::as_bool)
    {
        return present;
    }
    payload
        .get("analysis_contracts")
        .and_then(|contracts| contracts.get(metric_id))
        .and_then(|entry| entry.get("present"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

#[test]
fn compile_ds04_metric_explain_scene_emits_runtime_analysis_contracts() {
    let root = workspaces_root();
    let bundle = load_world_runtime_bundle(&root, DS04_APP, Some(&metric_explain_scope()))
        .expect("load bundle");
    let loaded = locate_dataset_resource(&bundle.compiled, "orders").expect("orders dataset");
    let orders = loaded.dataset.as_ref().expect("dataset view");
    let world_metrics = bundle
        .compiled
        .resources
        .iter()
        .find(|resource| resource.id == "__world_metrics__")
        .and_then(|resource| resource.dataset.as_ref())
        .expect("world metrics dataset");
    assert!(
        orders.runtime_analysis_contracts.is_empty(),
        "business dataset should not embed world.add_metric contracts"
    );
    assert!(
        world_metrics
            .runtime_analysis_contracts
            .keys()
            .any(|key| key.contains("orders_total")),
        "expected orders_total on __world_metrics__, keys: {:?}",
        world_metrics
            .runtime_analysis_contracts
            .keys()
            .collect::<Vec<_>>()
    );
}

#[test]
fn query_world_dataset_metrics_returns_analysis_contracts_for_orders_total() {
    let root = workspaces_root();
    let payload = query_world_dataset_metrics(
        &root,
        DS04_APP,
        Some(&metric_explain_scope()),
        "orders",
        &["orders_total".to_string()],
        None,
        &BTreeMap::new(),
    )
    .expect("dataset_metric");
    assert!(
        contract_present(&payload, "orders_total"),
        "analysis_contracts.orders_total.present should be true: {}",
        payload
            .get("analysis_contracts")
            .cloned()
            .unwrap_or(Value::Null)
    );
    assert!(
        payload.get("contract_hint").map_or(true, Value::is_null),
        "contract_hint should be null when contracts exist: {payload}"
    );
}

#[test]
fn query_world_dataset_returns_contracts_preview_for_orders_total() {
    let root = workspaces_root();
    let payload = query_world_dataset(
        &root,
        DS04_APP,
        Some(&metric_explain_scope()),
        "orders",
        None,
        &BTreeMap::new(),
        None,
        None,
    )
    .expect("dataset_query");
    let preview = payload
        .pointer("/dataset/analysis_contracts_preview")
        .expect("analysis_contracts_preview");
    let preview_obj = preview.as_object().expect("preview object");
    assert!(
        preview_obj
            .keys()
            .any(|key: &String| key.contains("orders_total")),
        "preview keys: {:?}",
        preview_obj.keys().collect::<Vec<_>>()
    );
    let entry_key = preview_obj
        .keys()
        .find(|key| key.contains("orders_total"))
        .expect("orders_total preview entry");
    assert_eq!(
        preview_obj
            .get(entry_key)
            .and_then(|v| v.get("present"))
            .and_then(Value::as_bool),
        Some(true)
    );
}

#[test]
fn dataset_metric_without_explain_emits_contract_hint() {
    let root = workspaces_root();
    let payload = query_world_dataset_metrics(
        &root,
        DS04_APP,
        Some(&metric_explain_scope()),
        "orders",
        &["orders_detail_table".to_string()],
        None,
        &BTreeMap::new(),
    )
    .expect("dataset_metric for dataframe metric");
    let contracts = payload
        .get("analysis_contracts")
        .and_then(Value::as_object)
        .expect("analysis_contracts object");
    assert!(
        contracts.is_empty(),
        "dataframe metric should not emit contracts"
    );
    assert_eq!(
        payload.get("contract_hint").and_then(Value::as_str),
        Some("no_runtime_analysis_contracts_for_requested_metrics")
    );
}

#[test]
fn prompt_catalog_includes_analysis_contract_section() {
    let root = workspaces_root();
    let bundle =
        load_world_runtime_bundle(&root, DS04_APP, Some(&metric_explain_scope())).expect("bundle");
    let lines = build_prompt_catalog_lines(&bundle, &[]);
    let joined = lines.join("\n");
    assert!(
        joined.contains("analysis_contract summaries"),
        "catalog should mention analysis_contract summaries:\n{joined}"
    );
}
