use serde_json::Value;
use ws_spbjw_integration_tests::{
    compile_app_from_root_with_options, evaluate_runtime_metric_defs, source_root, zhifa_app_root,
    CompileOptions,
};

#[test]
fn spbjw_enforcement_personnel_composition_by_agency_returns_grouped_rows() {
    use mei_lang_datasets::{query_metric_dataframe, DatasetQueryOptions};

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

    let composition = query_metric_dataframe(
        &compiled,
        app_root.as_path(),
        "enforcement_officers",
        "scenes/01-执法要素.mei::enforcement_personnel_count::composition_by_agency",
        Some("enforcement_elements"),
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
    .expect("enforcement_personnel_count composition_by_agency");
    assert!(
        composition.total > 0,
        "composition_by_agency should group officers by 所属部门, got total={} rows={:?}",
        composition.total,
        composition.rows
    );
}

#[test]
fn spbjw_penalty_total_rowset_query_returns_more_than_preview_rows() {
    use mei_lang_datasets::{query_metric_dataframe, DatasetQueryOptions};
    use ws_spbjw_integration_tests::load_xlsx_table_snapshot;

    let source_root = source_root();
    let app_root = zhifa_app_root();
    let board_target = "scenes/04-行政处罚.board.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some(board_target.to_string()),
        },
    )
    .unwrap_or_else(|error| panic!("compile `{board_target}` failed: {error}"));

    let assembly = compiled
        .scene_projection_assembly_by_id
        .get("penalty_total_analytics_board")
        .and_then(Value::as_object)
        .expect("penalty_total_analytics_board assembly");
    let detail_metric_id = assembly
        .get("projection_slots")
        .and_then(Value::as_array)
        .and_then(|slots| {
            slots.iter().find(|slot| {
                slot.as_object()
                    .and_then(|map| map.get("layout_zone"))
                    .and_then(Value::as_str)
                    == Some("detail")
            })
        })
        .and_then(Value::as_object)
        .and_then(|slot| slot.get("metric_id"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .expect("detail slot metric_id");
    assert!(
        detail_metric_id.ends_with("::__scalar_rowset__"),
        "penalty detail slot should bind scalar rowset, got `{detail_metric_id}`"
    );

    let penalty_snapshot = load_xlsx_table_snapshot(
        &app_root.join("upload/8.行政处罚结果清单.xlsx"),
        "upload/8.行政处罚结果清单.xlsx",
        None,
        1,
        None,
    )
    .expect("load full penalty xlsx");
    assert!(
        penalty_snapshot.rows.len() > 1000,
        "fixture should contain more than preview_rows=1000"
    );

    let rowset = query_metric_dataframe(
        &compiled,
        app_root.as_path(),
        "penalty_result_dashboard_ds",
        &detail_metric_id,
        Some("penalty_total_analytics_board"),
        Some(board_target),
        "integration-test",
        DatasetQueryOptions {
            page: 1,
            page_size: 20,
            collect_all: false,
            ..DatasetQueryOptions::default()
        },
        None,
        Vec::new(),
    )
    .expect("penalties_total_count rowset query");
    assert_eq!(
        rowset.total,
        penalty_snapshot.rows.len(),
        "penalty detail query should materialize full xlsx rows, not compile-time preview cap"
    );
}

#[test]
fn spbjw_penalty_filter_prefetch_does_not_cap_rowset_materialization() {
    use mei_lang_datasets::{
        clear_dataset_rows_cache, query_dataset_rows, query_metric_dataframe, DatasetQueryOptions,
    };
    use ws_spbjw_integration_tests::load_xlsx_table_snapshot;

    clear_dataset_rows_cache();

    let source_root = source_root();
    let app_root = zhifa_app_root();
    let scene_target = "scenes/04-行政处罚.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some(scene_target.to_string()),
        },
    )
    .unwrap_or_else(|error| panic!("compile `{scene_target}` failed: {error}"));

    let penalty_snapshot = load_xlsx_table_snapshot(
        &app_root.join("upload/8.行政处罚结果清单.xlsx"),
        "upload/8.行政处罚结果清单.xlsx",
        None,
        1,
        None,
    )
    .expect("load full penalty xlsx");
    assert!(penalty_snapshot.rows.len() > 1000);

    // 模拟 filter-bar 首次拉取 rowset 选项：page_size=1000、非 collect_all。
    let prefetch = query_dataset_rows(
        app_root.as_path(),
        compiled
            .resources
            .iter()
            .find(|resource| resource.id == "penalty_result_dashboard_ds")
            .and_then(|resource| resource.dataset.as_ref())
            .expect("penalty_result_dashboard_ds"),
        DatasetQueryOptions {
            page: 1,
            page_size: 1000,
            collect_all: false,
            ..DatasetQueryOptions::default()
        },
    )
    .expect("penalty filter prefetch");
    assert_eq!(prefetch.rows.len(), 1000);
    assert_eq!(prefetch.total, penalty_snapshot.rows.len());

    let rowset = query_metric_dataframe(
        &compiled,
        app_root.as_path(),
        "penalty_result_dashboard_ds",
        "scenes/04-行政处罚.mei::penalties_total_count::__scalar_rowset__",
        Some("penalty_dashboard"),
        Some(scene_target),
        "integration-test",
        DatasetQueryOptions {
            page: 1,
            page_size: 8,
            collect_all: false,
            ..DatasetQueryOptions::default()
        },
        None,
        Vec::new(),
    )
    .expect("penalty rowset after filter prefetch");
    assert_eq!(
        rowset.total,
        penalty_snapshot.rows.len(),
        "filter prefetch must not poison rowset materialization to preview/page cap"
    );

    clear_dataset_rows_cache();

    let week_rowset = query_metric_dataframe(
        &compiled,
        app_root.as_path(),
        "penalty_result_dashboard_ds",
        "scenes/04-行政处罚.mei::penalties_week_count::__scalar_rowset__",
        Some("penalty_dashboard"),
        Some(scene_target),
        "integration-test",
        DatasetQueryOptions {
            page: 1,
            page_size: 8,
            collect_all: false,
            ..DatasetQueryOptions::default()
        },
        None,
        Vec::new(),
    )
    .expect("penalty week rowset");
    let week_metric = mei_lang_datasets::evaluate_runtime_metrics(
        &compiled,
        app_root.as_path(),
        "penalty_result_dashboard_ds",
        &["penalties_week_count".to_string()],
        "penalty_dashboard",
        Some(scene_target),
        &Default::default(),
        &[],
        mei_lang_datasets::RuntimeMetricEvalMode::WithDag,
    )
    .expect("penalties_week_count metric");
    let week_count = week_metric
        .metrics
        .iter()
        .find(|metric| metric.id == "penalties_week_count")
        .and_then(|metric| metric.value.get("value").and_then(|v| v.as_f64()))
        .unwrap_or(0.0);
    assert_eq!(
        week_rowset.total as f64, week_count,
        "week detail rowset total should match card metric value"
    );
}

