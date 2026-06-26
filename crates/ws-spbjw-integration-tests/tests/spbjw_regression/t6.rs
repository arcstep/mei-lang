use serde_json::Value;
use ws_spbjw_integration_tests::{
    compile_app_from_root_with_options, evaluate_runtime_metric_defs, source_root, zhifa_app_root,
    CompileOptions,
};

#[test]
fn spbjw_typical_cases_swimlane_metric_dataframe_respects_result_id_filter() {
    use mei_lang_datasets::{query_metric_dataframe, DatasetQueryOptions};
    use std::collections::BTreeMap;

    let source_root = source_root();
    let app_root = zhifa_app_root();
    let board_target = "scenes/09-监督典型案例.board.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/09-监督典型案例.mei".to_string()),
        },
    )
    .expect("compile typical cases preview");
    let highlights = query_metric_dataframe(
        &compiled,
        app_root.as_path(),
        "typical_cases",
        "case_highlights",
        Some("typical_cases"),
        Some("scenes/09-监督典型案例.mei"),
        "integration-test",
        DatasetQueryOptions {
            page: 1,
            page_size: 4,
            collect_all: false,
            ..DatasetQueryOptions::default()
        },
        None,
        Vec::new(),
    )
    .expect("case_highlights dataframe");
    let sample_id = highlights
        .rows
        .first()
        .and_then(|row| row.get("value"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .expect("case_highlights row with 处理结果ID value");
    let mut filters = BTreeMap::new();
    filters.insert("处理结果ID".to_string(), sample_id.to_string());
    let detail = query_metric_dataframe(
        &compiled,
        app_root.as_path(),
        "typical_cases",
        "case_count::__scalar_rowset__",
        Some("typical_cases_detail_board"),
        Some(board_target),
        "integration-test",
        DatasetQueryOptions {
            page: 1,
            page_size: 20,
            filters,
            collect_all: false,
            ..DatasetQueryOptions::default()
        },
        None,
        Vec::new(),
    )
    .expect("typical cases detail metric dataframe");
    assert!(
        !detail.rows.is_empty(),
        "filtered detail should return at least one row"
    );
    assert!(
        detail.rows.iter().all(|row| {
            row.get("处理结果ID").and_then(Value::as_str).map(str::trim) == Some(sample_id)
        }),
        "detail rows should match selected 处理结果ID"
    );
}

#[test]
fn spbjw_home_resolves_imported_typical_cases_dataset_selector() {
    use mei_lang_kernel::locate_dataset_resource;

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
    .expect("compile spbjw home preview");
    let namespaced = "scenes/09-监督典型案例.mei::typical_cases";
    let resource_ids: Vec<_> = compiled
        .resources
        .iter()
        .filter(|resource| {
            resource
                .id
                .contains("typical_cases")
                || resource
                    .dataset
                    .as_ref()
                    .is_some_and(|dataset| dataset.id.contains("typical_cases"))
        })
        .map(|resource| resource.id.as_str())
        .collect();
    assert!(
        !resource_ids.is_empty(),
        "home preview should materialize imported typical_cases resources, got {resource_ids:?}"
    );
    let resource = locate_dataset_resource(&compiled, namespaced)
        .unwrap_or_else(|error| panic!("locate {namespaced}: {error}; resources={resource_ids:?}"));
    assert!(
        resource.dataset.is_some(),
        "typical_cases resource should expose dataset view"
    );
}

#[test]
fn spbjw_typical_cases_board_resolves_namespaced_dataset_selector() {
    use mei_lang_kernel::locate_dataset_resource;

    let source_root = source_root();
    let app_root = zhifa_app_root();
    let board_target = "scenes/09-监督典型案例.board.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some(board_target.to_string()),
        },
    )
    .expect("compile typical cases board preview");
    let namespaced = "scenes/09-监督典型案例.mei::typical_cases";
    let resource = locate_dataset_resource(&compiled, namespaced)
        .unwrap_or_else(|error| {
            let resource_ids: Vec<_> = compiled
                .resources
                .iter()
                .filter(|resource| resource.dataset.is_some())
                .map(|resource| resource.id.as_str())
                .collect();
            panic!("locate {namespaced}: {error}; resources={resource_ids:?}")
        });
    assert_eq!(resource.id, "typical_cases");
}

