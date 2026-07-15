use serde_json::Value;
use ws_spbjw_integration_tests::{
    compile_app_from_root_with_options, evaluate_runtime_metric_defs, source_root, zhifa_app_root,
    CompileOptions,
};

#[test]
fn compile_spbjw_qunfu_home_scene_succeeds() {
    let Some(source_root) = source_root() else {
        eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
        return;
    };
    let app_root = source_root.join("qunfu");
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/home.mei".to_string()),
            ..Default::default()
        },
    )
    .unwrap_or_else(|error| panic!("compile qunfu home failed: {error}"));
    assert!(
        compiled
            .diagnostics
            .iter()
            .all(|diag| !matches!(diag.severity, mei_lang_kernel::Severity::Error)),
        "qunfu home should not produce error diagnostics: {:?}",
        compiled.diagnostics
    );
    assert!(
        compiled.scene_contract.is_some(),
        "qunfu home should produce scene contract"
    );
}

#[test]
fn eval_spbjw_park_relocation_summary_and_charts_nonempty() {
    use std::collections::BTreeMap;

    use ws_spbjw_integration_tests::{coerce_rows_to_schema, load_xlsx_table_snapshot};

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
            preview_target: Some("scenes/01-执法要素.board.mei".to_string()),
            ..Default::default()
        },
    )
    .unwrap_or_else(|error| panic!("compile enforcement board failed: {error}"));
    let resource_id = "__world_metrics__::scenes/01-执法要素.world.mei::metrics";
    let owner = compiled
        .resources
        .iter()
        .find(|r| r.id == resource_id)
        .and_then(|r| r.dataset.as_ref())
        .or_else(|| {
            compiled
                .resources
                .iter()
                .find(|r| r.id.starts_with("__world_metrics__") && r.dataset.is_some())
                .and_then(|r| r.dataset.as_ref())
        })
        .unwrap_or_else(|| {
            let ids = compiled
                .resources
                .iter()
                .filter(|r| r.id.contains("world_metrics"))
                .map(|r| r.id.as_str())
                .collect::<Vec<_>>();
            panic!("world metrics missing; candidates: {ids:?}");
        });
    let mut datasets: BTreeMap<_, _> = compiled
        .resources
        .iter()
        .filter_map(|r| r.dataset.clone().map(|d| (r.id.clone(), d)))
        .collect();
    let relocation_key = if datasets.contains_key("enterprise_relocation") {
        "enterprise_relocation".to_string()
    } else {
        "scenes/01-执法要素.mei::enterprise_relocation".to_string()
    };
    let xlsx_path = app_root.join("upload/迁入迁出企业.xlsx");
    let snapshot = load_xlsx_table_snapshot(
        &xlsx_path,
        "upload/迁入迁出企业.xlsx",
        Some("企业迁入迁出记录"),
        1,
        None,
    )
    .expect("load relocation xlsx");
    {
        let relocation_dataset = datasets
            .get_mut(&relocation_key)
            .expect("enterprise_relocation dataset");
        let schema = relocation_dataset.schema.clone();
        relocation_dataset.rows = coerce_rows_to_schema(snapshot.rows, &schema);
        relocation_dataset.columns = schema.iter().map(|column| column.name.clone()).collect();
    }
    let metrics = evaluate_runtime_metric_defs(
        &owner.runtime_metric_defs,
        &[],
        &datasets,
        Some(&[
            "park_count::relocation_summary".to_string(),
            "park_count::relocation_by_month".to_string(),
            "park_count::relocation_by_park".to_string(),
        ]),
    )
    .expect("evaluate park relocation metrics");
    let summary_rows = metrics
        .get("park_count::relocation_summary")
        .and_then(|metric| metric.value.as_array())
        .map(|rows| rows.len())
        .unwrap_or(0);
    let month_rows = metrics
        .get("park_count::relocation_by_month")
        .and_then(|metric| metric.value.as_array())
        .map(|rows| rows.len())
        .unwrap_or(0);
    assert!(
        summary_rows > 0,
        "relocation_summary should have rows, got {summary_rows}"
    );
    assert!(
        month_rows > 0,
        "relocation_by_month should have rows, got {month_rows}"
    );
    let month_sample = metrics
        .get("park_count::relocation_by_month")
        .and_then(|metric| metric.value.as_array())
        .and_then(|rows| rows.first())
        .and_then(|row| row.get("年月"))
        .and_then(|value| value.as_str())
        .unwrap_or("");
    assert!(
        month_sample.len() == 7 && month_sample.chars().nth(4) == Some('-'),
        "年月 should be yyyy-mm, got {month_sample}"
    );
    assert!(
        month_rows < 84,
        "bucket_date should collapse day-level rows, got {month_rows}"
    );
}

fn assert_calendar_field_is_date_only(row: &Value, field: &str) {
    let Some(value) = row.get(field) else {
        return;
    };
    if value.is_null() {
        return;
    }
    let text = value
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| value.to_string());
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed == "？" || trimmed == "?" || trimmed == "——" || trimmed == "--"
    {
        return;
    }
    assert!(
        !trimmed.contains(':'),
        "field `{field}` should be calendar date without time, got `{trimmed}`"
    );
    assert!(
        trimmed.len() >= 10
            && trimmed.as_bytes().get(4) == Some(&b'-')
            && trimmed.as_bytes().get(7) == Some(&b'-'),
        "field `{field}` should look like yyyy-mm-dd, got `{trimmed}`"
    );
}

#[test]
fn spbjw_warning_and_issue_result_metric_dataframe_dates_are_calendar_only() {
    use mei_lang_datasets::{query_dataset_rows, query_metric_dataframe, DatasetQueryOptions};

    let Some(source_root) = source_root() else {
        eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
        return;
    };
    let Some(app_root) = zhifa_app_root() else {
        eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
        return;
    };
    let board_target = "scenes/05-监督预警.board.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some(board_target.to_string()),
            ..Default::default()
        },
    )
    .unwrap_or_else(|error| panic!("compile `{board_target}` failed: {error}"));

    let warning_metric = query_metric_dataframe(
        &compiled,
        app_root.as_path(),
        "warning_list",
        "warnings_count::__scalar_rowset__",
        Some("warnings_analytics_board"),
        Some(board_target),
        "integration-test",
        DatasetQueryOptions {
            page: 1,
            page_size: 50,
            collect_all: false,
            ..DatasetQueryOptions::default()
        },
        None,
        Vec::new(),
    )
    .expect("warning_list metric dataframe query");
    assert!(
        !warning_metric.rows.is_empty(),
        "warnings_count detail should return rows"
    );
    for row in &warning_metric.rows {
        for field in ["预警时间", "分办时间", "办结时间"] {
            assert_calendar_field_is_date_only(row, field);
        }
    }

    let issue_board = "scenes/08-监督成效.board.mei";
    let issue_compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some(issue_board.to_string()),
            ..Default::default()
        },
    )
    .unwrap_or_else(|error| panic!("compile `{issue_board}` failed: {error}"));
    let issue_metric = query_metric_dataframe(
        &issue_compiled,
        app_root.as_path(),
        "mechanism_documents",
        "effectiveness_mechanism_item_count::mechanism_documents_list",
        Some("effect_mechanism_documents_board"),
        Some("scenes/_shared/mechanism-documents.board.mei"),
        "integration-test",
        DatasetQueryOptions {
            page: 1,
            page_size: 50,
            collect_all: false,
            ..DatasetQueryOptions::default()
        },
        None,
        Vec::new(),
    )
    .expect("mechanism_documents metric dataframe query");
    assert_eq!(
        issue_metric.rows.len(),
        10,
        "mechanism documents list should expose 10 mapped mechanism rows"
    );
    for row in &issue_metric.rows {
        let name = row.get("机制名称").and_then(Value::as_str).unwrap_or("");
        assert!(
            !name.trim().is_empty(),
            "mechanism document row should include 机制名称, got: {row:?}"
        );
    }

    let warning_list = issue_compiled
        .resources
        .iter()
        .find_map(|resource| {
            resource
                .dataset
                .as_ref()
                .filter(|dataset| dataset.id == "warning_list")
                .cloned()
        })
        .expect("warning_list dataset view");
    let warning_rows = query_dataset_rows(
        app_root.as_path(),
        &warning_list,
        DatasetQueryOptions {
            page: 1,
            page_size: 20,
            collect_all: false,
            ..DatasetQueryOptions::default()
        },
    )
    .expect("warning_list direct query");
    assert!(!warning_rows.rows.is_empty(), "warning_list rows query");
    for row in warning_rows.rows.iter().take(20) {
        for field in ["预警时间", "分办时间", "办结时间"] {
            assert_calendar_field_is_date_only(row, field);
        }
    }

    let realtime_target = "scenes/06-实时预警.mei";
    let realtime_compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some(realtime_target.to_string()),
            ..Default::default()
        },
    )
    .unwrap_or_else(|error| panic!("compile `{realtime_target}` failed: {error}"));
    let realtime_metric = query_metric_dataframe(
        &realtime_compiled,
        app_root.as_path(),
        "__world_metrics__",
        "warnings_realtime_cockpit_table",
        Some("realtime_warnings"),
        Some(realtime_target),
        "integration-test",
        DatasetQueryOptions {
            page: 1,
            page_size: 10,
            collect_all: false,
            ..DatasetQueryOptions::default()
        },
        None,
        Vec::new(),
    )
    .expect("warnings_realtime_cockpit_table metric dataframe query");
    assert!(
        !realtime_metric.rows.is_empty(),
        "realtime cockpit table should return rows"
    );
    for row in &realtime_metric.rows {
        assert_calendar_field_is_date_only(row, "预警时间");
    }
}
