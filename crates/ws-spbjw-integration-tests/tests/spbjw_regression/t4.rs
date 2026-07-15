use ws_spbjw_integration_tests::{
    compile_app_from_root_with_options, source_root, zhifa_app_root,
    CompileOptions,
};

#[test]
fn spbjw_supervision_models_count_is_eighteen() {
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
            preview_target: Some("scenes/05-监督预警.mei".to_string()),
            ..Default::default()
        },
    )
    .expect("compile supervision warning preview for models count");
    let metric = compiled
        .world_metrics
        .get("supervision_models_count")
        .map(|entry| &entry.metric)
        .expect("supervision_models_count in world_metrics");
    let value = metric
        .value
        .get("value")
        .and_then(|v| v.as_f64())
        .or_else(|| metric.value.as_f64())
        .unwrap_or(f64::NAN);
    assert_eq!(
        value, 18.0,
        "《10》按序号前缀去重应得 18 个预警模型，got {value}"
    );
}

#[test]
fn spbjw_warning_list_materializes_leading_columns_from_empty_xlsx_headers() {
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
            preview_target: Some("scenes/05-监督预警.mei".to_string()),
            ..Default::default()
        },
    )
    .expect("compile supervision warning for warning_list columns");
    let dataset = compiled
        .resources
        .iter()
        .find(|r| r.id == "warning_list")
        .and_then(|r| r.dataset.as_ref())
        .expect("warning_list dataset");
    let row = dataset
        .rows
        .iter()
        .find(|row| {
            row.get("预警ID")
                .and_then(|v| v.as_str())
                .map(|s| s.contains("YJ2025001"))
                .unwrap_or(false)
        })
        .expect("sample warning row");
    let serial = row.get("序号").and_then(|value| {
        value
            .as_i64()
            .or_else(|| value.as_f64().map(|number| number as i64))
    });
    assert_eq!(
        serial,
        Some(1),
        "序号列应来自 Excel A 列（表头行为空单元格）"
    );
    assert_eq!(
        row.get("监督领域").and_then(|v| v.as_str()),
        Some("行政执法"),
        "监督领域应来自 Excel B 列"
    );
}

#[test]
#[ignore = "历史数据口径：预警条数求和断言待与 Excel 源数据对齐后恢复"]
fn spbjw_warnings_count_sums_warning_entry_column() {
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
            preview_target: Some("scenes/05-监督预警.mei".to_string()),
            ..Default::default()
        },
    )
    .expect("compile supervision warning preview for warnings count");
    let metric = compiled
        .world_metrics
        .get("warnings_count")
        .map(|entry| &entry.metric)
        .expect("warnings_count in world_metrics");
    let value = metric
        .value
        .get("value")
        .and_then(|v| v.as_f64())
        .or_else(|| metric.value.as_f64())
        .unwrap_or(f64::NAN);
    assert_eq!(
        value, 25.0,
        "《11》预警ID去重后对「预警条数」求和应为 25 条，got {value}"
    );
}
