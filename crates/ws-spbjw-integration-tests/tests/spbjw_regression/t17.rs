use serde_json::Value;
use ws_spbjw_integration_tests::{
    compile_app_from_root_with_options, evaluate_runtime_metric_defs, source_root, zhifa_app_root,
    CompileOptions,
};

#[test]
fn compile_spbjw_enforcement_elements_direct_preview_world_metrics_have_analysis_contracts() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let target = "scenes/01-执法要素.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some(target.to_string()),
        },
    )
    .unwrap_or_else(|error| panic!("compile `{target}` failed: {error}"));
    let dataset = compiled
        .resources
        .iter()
        .find(|resource| resource.id == "__world_metrics__")
        .and_then(|resource| resource.dataset.as_ref())
        .unwrap_or_else(|| panic!("`{target}` direct preview should include __world_metrics__"));
    for metric_id in [
        "enforcement_units_count",
        "enforcement_personnel_count",
        "enforcement_items_count",
        "key_enterprises_count",
        "park_count",
        "whitelist_enterprises_count",
    ] {
        assert!(
            dataset.runtime_metric_defs.contains_key(metric_id),
            "expected runtime_metric_defs key `{metric_id}`, got: {:?}",
            dataset.runtime_metric_defs.keys().collect::<Vec<_>>()
        );
        assert!(
            dataset.runtime_analysis_contracts.contains_key(metric_id),
            "expected runtime_analysis_contracts key `{metric_id}`, got: {:?}",
            dataset
                .runtime_analysis_contracts
                .keys()
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn compile_spbjw_enforcement_elements_enforcement_units_resource_has_hydratable_source() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let target = "scenes/01-执法要素.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some(target.to_string()),
        },
    )
    .unwrap_or_else(|error| panic!("compile `{target}` failed: {error}"));
    let dataset = compiled
        .resources
        .iter()
        .find_map(|resource| {
            resource
                .dataset
                .as_ref()
                .filter(|dataset| dataset.id == "enforcement_units")
                .cloned()
        })
        .unwrap_or_else(|| {
            panic!(
                "expected enforcement_units dataset, got ids: {:?}",
                compiled
                    .resources
                    .iter()
                    .filter_map(|r| r.dataset.as_ref().map(|d| d.id.as_str()))
                    .collect::<Vec<_>>()
            )
        });
    assert_eq!(dataset.source.kind, "xlsx");
    assert!(
        dataset.source.path.contains("执法单位"),
        "unexpected source path: {}",
        dataset.source.path
    );
    assert!(
        !dataset.rows.is_empty(),
        "compile-time preview rows should not be empty"
    );
    let first = dataset.rows.first().cloned().unwrap_or_default();
    assert!(
        first.get("类别").is_some() || first.get("执法单位").is_some(),
        "expected schema-mapped row keys, got keys: {:?}",
        first.as_object().map(|m| m.keys().collect::<Vec<_>>())
    );
}

#[test]
fn spbjw_enforcement_items_count_rowset_matches_metric_value() {
    use mei_lang_datasets::{evaluate_runtime_metrics, query_metric_dataframe, DatasetQueryOptions};

    let source_root = source_root();
    let app_root = zhifa_app_root();
    let target = "scenes/01-执法要素.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some(target.to_string()),
        },
    )
    .unwrap_or_else(|error| panic!("compile `{target}` failed: {error}"));
    let scene_id = compiled
        .active_scene
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("default");
    let rowset = query_metric_dataframe(
        &compiled,
        app_root.as_path(),
        "__world_metrics__",
        "enforcement_items_count::__scalar_rowset__",
        Some(scene_id),
        Some(target),
        "integration-test",
        DatasetQueryOptions {
            page: 1,
            page_size: 16,
            collect_all: false,
            ..DatasetQueryOptions::default()
        },
        None,
        Vec::new(),
    )
    .expect("enforcement_items_count rowset");
    let metric = evaluate_runtime_metrics(
        &compiled,
        app_root.as_path(),
        "__world_metrics__",
        &["enforcement_items_count".to_string()],
        scene_id,
        Some(target),
        &Default::default(),
        &[],
        mei_lang_datasets::RuntimeMetricEvalMode::WithDag,
    )
    .expect("enforcement_items_count metric");
    let value = metric
        .metrics
        .iter()
        .find(|metric| metric.id == "enforcement_items_count")
        .and_then(|metric| metric.value.get("value").and_then(|value| value.as_f64()))
        .unwrap_or(0.0);
    assert_eq!(
        rowset.total as f64, value,
        "enforcement_items_count rowset total should match metric value"
    );
}

