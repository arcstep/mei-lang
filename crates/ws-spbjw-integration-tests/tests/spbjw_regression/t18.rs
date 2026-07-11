use serde_json::Value;
use ws_spbjw_integration_tests::{
    compile_app_from_root_with_options, evaluate_runtime_metric_defs, source_root, zhifa_app_root,
    CompileOptions,
};

#[test]
fn spbjw_map_scene_world_metrics_can_evaluate() {
    use mei_lang_datasets::evaluate_runtime_metrics;

    let source_root = source_root();
    let app_root = zhifa_app_root();
    let target = "scenes/10-地图.mei";
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
        .unwrap_or_else(|| panic!("10-地图 preview should expose __world_metrics__"));
    let metric_id = dataset
        .runtime_metric_defs
        .keys()
        .find(|metric_id| metric_id == &&"map_park_penalty_count_2025".to_string())
        .cloned()
        .unwrap_or_else(|| {
            panic!(
                "10-地图 runtime_metric_defs should include map_park_penalty_count_2025, got: {:?}",
                dataset.runtime_metric_defs.keys().collect::<Vec<_>>()
            )
        });
    let scene_id = compiled
        .active_scene
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("default");
    let metric = evaluate_runtime_metrics(
        &compiled,
        app_root.as_path(),
        "__world_metrics__",
        std::slice::from_ref(&metric_id),
        scene_id,
        Some(target),
        &Default::default(),
        &[],
        mei_lang_datasets::RuntimeMetricEvalMode::WithDag,
    )
    .expect("imported map world metric");
    let resolved = metric
        .metrics
        .iter()
        .find(|entry| entry.id == metric_id)
        .unwrap_or_else(|| panic!("expected metric `{metric_id}` in response"));
    assert!(
        resolved.value.get("value").is_some()
            || resolved.value.is_number()
            || resolved
                .value
                .as_array()
                .map(|rows| !rows.is_empty())
                .unwrap_or(false),
        "map world metric should resolve to scalar or non-empty grouped rows, got {:?}",
        resolved.value
    );
}

#[test]
fn compile_spbjw_enforcement_elements_direct_preview_inferred_rowset_materializes_enforcement_units(
) {
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
        .expect("direct preview world metrics");
    let rowset_key = "enforcement_units_count::__scalar_rowset__";
    let rowset_def = dataset
        .runtime_metric_defs
        .get(rowset_key)
        .and_then(Value::as_object)
        .unwrap_or_else(|| {
            panic!(
                "missing rowset def `{rowset_key}`, keys: {:?}",
                dataset.runtime_metric_defs.keys().collect::<Vec<_>>()
            )
        });
    eprintln!(
        "rowset def: {}",
        serde_json::to_string_pretty(rowset_def).unwrap()
    );
    let metric = dataset
        .metrics
        .get(rowset_key)
        .or_else(|| dataset.metrics.get("enforcement_units_count"));
    if let Some(m) = metric {
        eprintln!("metric value shape: {:?}", m.shape);
        eprintln!(
            "metric value: {}",
            serde_json::to_string(&m.value)
                .unwrap_or_default()
                .chars()
                .take(200)
                .collect::<String>()
        );
    }
}

#[test]
fn compile_spbjw_home_preview_imported_enforcement_personnel_composition_tab_uses_real_rowset() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/home.mei".to_string()),
        },
    )
    .expect("compile home preview");
    let resource_id = "__world_metrics__::scenes/01-执法要素.mei::metrics";
    let metric_key = "scenes/01-执法要素.mei::enforcement_personnel_count";
    let dataset = compiled
        .resources
        .iter()
        .find(|resource| resource.id == resource_id)
        .and_then(|resource| resource.dataset.as_ref())
        .unwrap_or_else(|| panic!("missing `{resource_id}`"));
    let contract = dataset
        .runtime_analysis_contracts
        .get(metric_key)
        .and_then(Value::as_object)
        .unwrap_or_else(|| panic!("missing contract `{metric_key}`"));
    let composition_metric_id = contract
        .get("tab_metrics")
        .and_then(|value| value.get("composition"))
        .and_then(|value| value.get("metric_id"))
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        composition_metric_id.ends_with("::composition_by_agency")
            || composition_metric_id.ends_with("::composition_by_rank"),
        "composition tab should bind to hoisted composition metric, got `{composition_metric_id}`"
    );
    assert!(
        !composition_metric_id.ends_with("::__scalar_rowset__"),
        "composition tab should not bind to raw scalar rowset"
    );
}
