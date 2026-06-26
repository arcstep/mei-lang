use serde_json::Value;
use ws_spbjw_integration_tests::{
    compile_app_from_root_with_options, evaluate_runtime_metric_defs, source_root, zhifa_app_root,
    CompileOptions,
};

#[test]
fn compile_spbjw_enforcement_elements_personnel_rowset_evaluates_nonempty() {
    use std::collections::BTreeMap;

    use ws_spbjw_integration_tests::resolve_runtime_metric_def_key;
    use ws_spbjw_integration_tests::MetricShape;

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
    let resource_id = "__world_metrics__";
    let owner = compiled
        .resources
        .iter()
        .find(|resource| resource.id == resource_id)
        .and_then(|resource| resource.dataset.as_ref())
        .unwrap_or_else(|| {
            panic!(
                "native preview should expose `{resource_id}`, got: {:?}",
                compiled
                    .resources
                    .iter()
                    .filter(|r| r.id.contains("world_metrics"))
                    .map(|r| r.id.as_str())
                    .collect::<Vec<_>>()
            )
        });
    let rowset_key = "enforcement_personnel_count::__scalar_rowset__";
    let resolved =
        resolve_runtime_metric_def_key(resource_id, rowset_key, &owner.runtime_metric_defs)
            .unwrap_or_else(|| panic!("resolve `{rowset_key}` on `{resource_id}`"));
    let mut datasets = compiled
        .resources
        .iter()
        .filter_map(|resource| {
            resource
                .dataset
                .clone()
                .map(|dataset| (resource.id.clone(), dataset))
        })
        .collect::<BTreeMap<_, _>>();
    let dataset_aliases: Vec<_> = datasets
        .values()
        .map(|dataset| (dataset.id.clone(), dataset.clone()))
        .collect();
    for (dataset_id, dataset) in dataset_aliases {
        datasets.entry(dataset_id).or_insert(dataset);
    }
    let metrics = evaluate_runtime_metric_defs(
        &owner.runtime_metric_defs,
        &[],
        &datasets,
        Some(&[resolved.clone()]),
    )
    .unwrap_or_else(|error| panic!("evaluate `{resolved}` failed: {error}"));
    let metric = metrics
        .get(&resolved)
        .unwrap_or_else(|| panic!("missing metric `{resolved}`"));
    assert_eq!(metric.shape, MetricShape::Dataframe);
    let row_count = metric.value.as_array().map(|rows| rows.len()).unwrap_or(0);
    assert!(
        row_count > 0,
        "personnel rowset should materialize rows, got {row_count}"
    );
}

#[test]
fn compile_spbjw_home_imported_personnel_rowset_evaluates_nonempty() {
    use std::collections::BTreeMap;

    use ws_spbjw_integration_tests::resolve_runtime_metric_def_key;
    use ws_spbjw_integration_tests::MetricShape;

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
    let metric_key = "scenes/01-执法要素.mei::enforcement_personnel_count::__scalar_rowset__";
    let owner = compiled
        .resources
        .iter()
        .find(|resource| resource.id == resource_id)
        .and_then(|resource| resource.dataset.as_ref())
        .unwrap_or_else(|| panic!("missing imported world metrics `{resource_id}`"));
    let resolved =
        resolve_runtime_metric_def_key(resource_id, metric_key, &owner.runtime_metric_defs)
            .unwrap_or_else(|| panic!("resolve imported rowset `{metric_key}`"));
    let mut datasets = compiled
        .resources
        .iter()
        .filter_map(|resource| {
            resource
                .dataset
                .clone()
                .map(|dataset| (resource.id.clone(), dataset))
        })
        .collect::<BTreeMap<_, _>>();
    let dataset_aliases: Vec<_> = datasets
        .values()
        .map(|dataset| (dataset.id.clone(), dataset.clone()))
        .collect();
    for (dataset_id, dataset) in dataset_aliases {
        datasets.entry(dataset_id).or_insert(dataset);
    }
    let metrics = evaluate_runtime_metric_defs(
        &owner.runtime_metric_defs,
        &[],
        &datasets,
        Some(&[resolved.clone()]),
    )
    .unwrap_or_else(|error| panic!("evaluate imported rowset failed: {error}"));
    let metric = metrics.get(&resolved).expect("imported rowset metric");
    assert_eq!(metric.shape, MetricShape::Dataframe);
    let row_count = metric.value.as_array().map(|rows| rows.len()).unwrap_or(0);
    assert!(
        row_count > 0,
        "imported personnel rowset should materialize rows, got {row_count}"
    );
}

#[test]
#[ignore = "历史数据口径：park_migration_yearly scoped metric 已迁移，待单独恢复断言"]
fn spbjw_park_migration_yearly_table_evaluates_nonempty_rows() {
    use std::collections::BTreeMap;

    use ws_spbjw_integration_tests::MetricShape;

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
    let world_metrics = compiled
        .resources
        .iter()
        .find(|resource| resource.id == "__world_metrics__")
        .and_then(|resource| resource.dataset.as_ref())
        .unwrap_or_else(|| panic!("`{target}` direct preview should include __world_metrics__"));
    let yearly_key = world_metrics
        .runtime_metric_defs
        .keys()
        .find(|key| key.contains("park_migration_yearly"))
        .cloned()
        .unwrap_or_else(|| {
            panic!(
                "missing park_migration_yearly scoped metric, keys: {:?}",
                world_metrics.runtime_metric_defs.keys().collect::<Vec<_>>()
            )
        });
    let datasets = compiled
        .resources
        .iter()
        .filter_map(|resource| {
            resource
                .dataset
                .clone()
                .map(|dataset| (resource.id.clone(), dataset))
        })
        .collect::<BTreeMap<_, _>>();
    let metrics = evaluate_runtime_metric_defs(
        &world_metrics.runtime_metric_defs,
        &[],
        &datasets,
        Some(&[yearly_key.clone()]),
    )
    .unwrap_or_else(|error| panic!("evaluate park migration yearly failed: {error}"));
    let metric = metrics
        .get(&yearly_key)
        .unwrap_or_else(|| panic!("missing evaluated metric `{yearly_key}`"));
    assert_eq!(metric.shape, MetricShape::Dataframe);
    let row_count = metric.value.as_array().map(|rows| rows.len()).unwrap_or(0);
    assert!(
        row_count > 0,
        "park migration yearly wide table should have rows, got {row_count}; value={}",
        serde_json::to_string(&metric.value).unwrap_or_default()
    );
}

