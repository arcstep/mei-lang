use serde_json::Value;
use ws_spbjw_integration_tests::{
    compile_app_from_root_with_options, evaluate_runtime_metric_defs, source_root, zhifa_app_root,
    CompileOptions,
};

#[test]
fn compile_spbjw_enforcement_elements_direct_preview_composition_tab_uses_rowset_not_dataset() {
    let Some(source_root) = source_root() else {
        eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
        return;
    };
    let Some(app_root) = zhifa_app_root() else {
        eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
        return;
    };
    let target = "scenes/01-执法要素.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some(target.to_string()),
            ..Default::default()
        },
    )
    .unwrap_or_else(|error| panic!("compile `{target}` failed: {error}"));
    let dataset = compiled
        .resources
        .iter()
        .find(|resource| resource.id == "__world_metrics__")
        .and_then(|resource| resource.dataset.as_ref())
        .expect("direct preview world metrics");
    let contract = dataset
        .runtime_analysis_contracts
        .get("enforcement_personnel_count")
        .and_then(Value::as_object)
        .expect("enforcement_personnel_count contract");
    let composition_metric_id = contract
        .get("tab_metrics")
        .and_then(|value| value.get("composition"))
        .and_then(|value| value.get("metric_id"))
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        composition_metric_id.contains("__scalar_rowset__"),
        "composition tab should bind to inferred scalar rowset, got `{composition_metric_id}`"
    );
    assert_ne!(
        composition_metric_id, "enforcement_units",
        "composition tab should not bind to raw dataset id"
    );
}

#[test]
fn compile_spbjw_runtime_metric_defs_expand_explain_scope_metric_nodes() {
    let Some(source_root) = source_root() else {
        eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
        return;
    };
    let Some(app_root) = zhifa_app_root() else {
        eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
        return;
    };
    let target = "scenes/08-监督成效.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some(target.to_string()),
            ..Default::default()
        },
    )
    .unwrap_or_else(|_| panic!("compile {target} preview"));
    let dataset = compiled
        .resources
        .iter()
        .find_map(|resource| {
            let dataset = resource.dataset.as_ref()?;
            dataset
                .runtime_metric_defs
                .contains_key("effectiveness_handled_person_times")
                .then_some(dataset)
        })
        .expect("supervision effectiveness runtime metric defs");
    assert!(
        dataset
            .runtime_metric_defs
            .contains_key("effectiveness_handled_person_times::__scalar_rowset__"),
        "effectiveness_handled_person_times should hoist inferred scalar rowset child metric"
    );
    let contract = dataset
        .runtime_analysis_contracts
        .get("effectiveness_handled_person_times")
        .and_then(Value::as_object)
        .expect("handled analysis contract");
    let detail = contract
        .get("tab_metrics")
        .and_then(Value::as_object)
        .and_then(|tabs| tabs.get("detail"))
        .and_then(Value::as_object)
        .expect("detail tab metric");
    assert_eq!(
        detail.get("metric_id").and_then(Value::as_str),
        Some("effectiveness_handled_person_times::__scalar_rowset__")
    );
}

#[test]
fn spbjw_effectiveness_transfer_clue_and_filing_count_from_alert_tracking() {
    use std::collections::BTreeMap;

    let Some(source_root) = source_root() else {
        eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
        return;
    };
    let Some(app_root) = zhifa_app_root() else {
        eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
        return;
    };
    let target = "scenes/08-监督成效.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some(target.to_string()),
            ..Default::default()
        },
    )
    .unwrap_or_else(|error| panic!("compile `{target}` failed: {error}"));
    let owner_dataset = compiled
        .resources
        .iter()
        .find_map(|resource| {
            let dataset = resource.dataset.as_ref()?;
            dataset
                .runtime_metric_defs
                .contains_key("effectiveness_transfer_clue_count")
                .then_some(dataset)
        })
        .expect("08 capsule should materialize effectiveness metrics");
    let datasets: BTreeMap<_, _> = compiled
        .resources
        .iter()
        .filter_map(|r| r.dataset.clone().map(|d| (r.id.clone(), d)))
        .collect();
    let metrics = evaluate_runtime_metric_defs(
        &owner_dataset.runtime_metric_defs,
        &[],
        &datasets,
        Some(&[
            "effectiveness_transfer_clue_count".to_string(),
            "effectiveness_filing_count".to_string(),
            "effectiveness_mechanism_item_count".to_string(),
        ]),
    )
    .expect("evaluate supervision effectiveness clue/filing/mechanism metrics");
    let transfer = metrics
        .get("effectiveness_transfer_clue_count")
        .unwrap_or_else(|| panic!("missing metric `effectiveness_transfer_clue_count`"));
    let transfer_value = transfer
        .value
        .get("value")
        .and_then(|v| v.as_f64())
        .or_else(|| transfer.value.as_f64())
        .unwrap_or(f64::NAN);
    assert_eq!(
        transfer_value, 4.0,
        "effectiveness_transfer_clue_count should count four 《11》 rows with 是否转问题线索=是, got {transfer_value}"
    );
    let filing = metrics
        .get("effectiveness_filing_count")
        .unwrap_or_else(|| panic!("missing metric `effectiveness_filing_count`"));
    let filing_value = filing
        .value
        .get("value")
        .and_then(|v| v.as_f64())
        .or_else(|| filing.value.as_f64())
        .unwrap_or(f64::NAN);
    assert_eq!(
        filing_value, 4.0,
        "effectiveness_filing_count should match transfer clue count from 《11》, got {filing_value}"
    );
    let mechanism = metrics
        .get("effectiveness_mechanism_item_count")
        .unwrap_or_else(|| panic!("missing metric `effectiveness_mechanism_item_count`"));
    let mechanism_value = mechanism
        .value
        .get("value")
        .and_then(|v| v.as_f64())
        .or_else(|| mechanism.value.as_f64())
        .unwrap_or(f64::NAN);
    assert_eq!(
        mechanism_value, 10.0,
        "effectiveness_mechanism_item_count should dedupe 10 mechanism titles after splitting on 、 and 》《, got {mechanism_value}"
    );
}
