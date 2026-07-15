use ws_spbjw_integration_tests::{
    compile_app_from_root_with_options, source_root, zhifa_app_root,
    CompileOptions,
};

#[test]
fn compile_spbjw_runtime_metric_defs_support_explain_list_shape() {
    let Some(source_root) = source_root() else {
        eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
        return;
    };
    let Some(app_root) = zhifa_app_root() else {
        eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
        return;
    };
    let preview_targets = [
        (
            "scenes/08-监督成效.mei",
            "effectiveness_handled_person_times",
        ),
        (
            "scenes/07-问题办理.mei",
            "effectiveness_issue_verification_rate",
        ),
        (
            "scenes/03-指标体系.mei",
            "inspection_frequency_reduction_rate",
        ),
    ];
    for (target, metric_id) in preview_targets {
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
        let explain = compiled.resources.iter().find_map(|resource| {
            let dataset = resource.dataset.as_ref()?;
            dataset
                .runtime_metric_defs
                .get(metric_id)
                .and_then(|metric| metric.get("explain"))
        });
        let explain =
            explain.unwrap_or_else(|| panic!("{metric_id} explain should exist in {target}"));
        let items = explain.as_array().unwrap_or_else(|| {
            panic!("{metric_id} explain should normalize to list in {target}: {explain:?}")
        });
        assert!(
            !items.is_empty(),
            "{metric_id} explain list should not be empty in {target}"
        );
    }
}

#[test]
fn compile_spbjw_home_preview_imported_world_metrics_align_analysis_contract_keys() {
    let Some(source_root) = source_root() else {
        eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
        return;
    };
    let Some(app_root) = zhifa_app_root() else {
        eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
        return;
    };
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/home.mei".to_string()),
            ..Default::default()
        },
    )
    .expect("compile home preview");
    let cases = [
        ("scenes/01-执法要素.mei", "enforcement_units_count"),
        (
            "scenes/08-监督成效.mei",
            "effectiveness_handled_person_times",
        ),
        (
            "scenes/03-指标体系.mei",
            "inspection_frequency_reduction_rate",
        ),
    ];
    for (capsule, local_metric_id) in cases {
        let resource_id = format!("__world_metrics__::{capsule}::metrics");
        let metric_key = format!("{capsule}::{local_metric_id}");
        let dataset = compiled
            .resources
            .iter()
            .find(|resource| resource.id == resource_id)
            .and_then(|resource| resource.dataset.as_ref())
            .unwrap_or_else(|| {
                panic!(
                    "home preview should include imported world metrics resource `{resource_id}`"
                )
            });
        assert!(
            dataset.runtime_metric_defs.contains_key(&metric_key),
            "expected runtime_metric_defs key `{metric_key}` on `{resource_id}`"
        );
        assert!(
            dataset
                .runtime_analysis_contracts
                .contains_key(&metric_key),
            "expected runtime_analysis_contracts key `{metric_key}` on `{resource_id}`, got keys: {:?}",
            dataset.runtime_analysis_contracts.keys().collect::<Vec<_>>()
        );
    }
}

#[test]
fn compile_spbjw_home_embedded_map_world_metrics_materialized() {
    let Some(source_root) = source_root() else {
        eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
        return;
    };
    let Some(app_root) = zhifa_app_root() else {
        eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
        return;
    };
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/home.mei".to_string()),
            ..Default::default()
        },
    )
    .expect("compile home preview");
    let resource_id = "__world_metrics__::scenes/10-地图.mei::metrics";
    let dataset = compiled
        .resources
        .iter()
        .find(|resource| resource.id == resource_id)
        .and_then(|resource| resource.dataset.as_ref())
        .unwrap_or_else(|| {
            panic!(
                "home preview should include imported map world metrics `{resource_id}`, got: {:?}",
                compiled
                    .resources
                    .iter()
                    .filter(|r| r.id.contains("10-地图") || r.id.contains("__world_metrics__"))
                    .map(|r| r.id.as_str())
                    .collect::<Vec<_>>()
            )
        });
    for metric_id in [
        "scenes/10-地图.mei::map_street_inspection_count_2025",
        "scenes/10-地图.mei::map_enterprise_poi_in_park_2025",
    ] {
        assert!(
            dataset.runtime_metric_defs.contains_key(metric_id),
            "expected `{metric_id}` on home map world metrics, keys: {:?}",
            dataset
                .runtime_metric_defs
                .keys()
                .filter(|k| k.contains("map_"))
                .collect::<Vec<_>>()
        );
    }
}
