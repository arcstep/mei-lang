//! 问题办理：world.add_resource(resource_ref) 须在 capsule 加载时解析，否则指标恒为 0。

use std::collections::BTreeMap;

use serde_json::Value;

use ws_spbjw_integration_tests::{
    compile_app_from_root_with_options, evaluate_runtime_metric_defs, source_root, zhifa_app_root,
    CompileOptions,
};

#[test]
fn compile_spbjw_issue_handling_world_metrics_materialize_from_resource_ref() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let capsule = "scenes/07-问题办理.mei";
    let owner = format!("__world_metrics__::{capsule}::metrics");
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/home.mei".to_string()),
        },
    )
    .unwrap_or_else(|e| panic!("compile home preview failed: {e}"));
    let errors: Vec<_> = compiled
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, mei_lang_kernel::Severity::Error))
        .map(|d| d.message.clone())
        .collect();
    assert!(
        errors.is_empty(),
        "问题办理 preview should compile without errors: {errors:?}"
    );
    let owner_dataset = compiled
        .resources
        .iter()
        .find(|r| r.id == owner)
        .and_then(|r| r.dataset.as_ref())
        .unwrap_or_else(|| panic!("missing `{owner}`"));
    let warning_list = compiled
        .resources
        .iter()
        .find(|r| r.id == "warning_list")
        .and_then(|r| r.dataset.as_ref())
        .unwrap_or_else(|| panic!("warning_list should be materialized for `{capsule}`"));
    let namespaced_warning = format!("{capsule}::warning_list");
    let imported_warning = compiled
        .resources
        .iter()
        .find(|r| r.id == namespaced_warning)
        .and_then(|r| r.dataset.as_ref());
    assert!(
        imported_warning.is_some_and(|d| !d.rows.is_empty()) || !warning_list.rows.is_empty(),
        "imported warning_list should have rows"
    );
    assert!(
        !warning_list.rows.is_empty(),
        "warning_list should have rows when loaded via resource_ref"
    );
    let datasets: BTreeMap<_, _> = compiled
        .resources
        .iter()
        .filter_map(|r| r.dataset.clone().map(|d| (r.id.clone(), d)))
        .collect();
    let pending_key = "scenes/07-问题办理.mei::warnings_pending_count";
    let rate_key = "scenes/07-问题办理.mei::effectiveness_issue_verification_rate";
    let metrics = evaluate_runtime_metric_defs(
        &owner_dataset.runtime_metric_defs,
        &[],
        &datasets,
        Some(&[pending_key.to_string(), rate_key.to_string()]),
    )
    .unwrap_or_else(|e| panic!("evaluate issue_handling metrics failed: {e}"));
    let pending = metrics
        .get(pending_key)
        .unwrap_or_else(|| panic!("missing metric `{pending_key}`"));
    let pending_value = pending
        .value
        .get("value")
        .and_then(|v| v.as_f64())
        .unwrap_or_else(|| {
            pending
                .value
                .as_f64()
                .expect("warnings_pending_count value")
        });
    assert!(
        pending_value > 0.0,
        "warnings_pending_count should be > 0 on home preview, got {pending_value}"
    );

    let capsule_compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some(capsule.to_string()),
        },
    )
    .unwrap_or_else(|e| panic!("compile `{capsule}` preview failed: {e}"));
    let capsule_owner_id = "__world_metrics__";
    let capsule_pending_key = "warnings_pending_count";
    let capsule_owner = capsule_compiled
        .resources
        .iter()
        .find(|r| r.id == capsule_owner_id)
        .and_then(|r| r.dataset.as_ref())
        .expect("capsule preview should materialize world metrics");
    let capsule_metrics = evaluate_runtime_metric_defs(
        &capsule_owner.runtime_metric_defs,
        &[],
        &capsule_compiled
            .resources
            .iter()
            .filter_map(|r| r.dataset.clone().map(|d| (r.id.clone(), d)))
            .collect(),
        Some(&[
            capsule_pending_key.to_string(),
            "effectiveness_issue_verification_rate".to_string(),
        ]),
    )
    .unwrap_or_else(|e| panic!("evaluate capsule metrics failed: {e}"));
    assert!(
        capsule_metrics
            .get(capsule_pending_key)
            .and_then(|m| m
                .value
                .get("value")
                .and_then(|v| v.as_f64())
                .or_else(|| m.value.as_f64()))
            .unwrap_or(0.0)
            > 0.0,
        "capsule preview warnings_pending_count should be > 0"
    );
    let rate_rowset_key =
        "scenes/07-问题办理.mei::effectiveness_issue_verification_rate::__scalar_rowset__";
    assert!(
        owner_dataset
            .runtime_metric_defs
            .contains_key(rate_rowset_key),
        "verification rate should hoist inferred scalar rowset for drilldown, keys: {:?}",
        owner_dataset.runtime_metric_defs.keys().collect::<Vec<_>>()
    );
    let rate = metrics
        .get(rate_key)
        .unwrap_or_else(|| panic!("missing metric `{rate_key}`"));
    rate.value
        .get("value")
        .and_then(|v| v.as_f64())
        .or_else(|| rate.value.as_f64())
        .expect("effectiveness_issue_verification_rate should materialize value");

    let warning_detail = datasets
        .get("warning_detail")
        .or_else(|| datasets.get(&format!("{capsule}::warning_detail")))
        .unwrap_or_else(|| panic!("warning_detail dataset should be materialized"));
    assert_eq!(
        warning_detail.rows.len(),
        11,
        "current alert_tracking sample should have 11 verified rows (是/否), got {}",
        warning_detail.rows.len()
    );
    for row in &warning_detail.rows {
        let verified = row
            .get("是否查实")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .trim();
        assert!(
            verified == "是" || verified == "否",
            "warning_detail should only include 是/否, got {verified:?} row={row:?}"
        );
    }
    assert!(
        warning_detail
            .schema
            .iter()
            .any(|column| column.name == "核查情况"),
        "warning_detail schema should include full warning_list columns"
    );
    let rate_metric = owner_dataset
        .runtime_metric_defs
        .get(rate_key)
        .and_then(Value::as_object)
        .unwrap_or_else(|| panic!("missing runtime metric def `{rate_key}`"));
    let detail_fields = rate_metric
        .get("explain")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|item| {
            item.as_object().is_some_and(|map| {
                map.get("id").and_then(Value::as_str) == Some("detail")
                    || map.get("support_role").and_then(Value::as_str) == Some("detail")
            })
        })
        .and_then(|item| item.get("fields"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    assert!(
        detail_fields
            .iter()
            .any(|field| field.as_str() == Some("序号"))
            && detail_fields
                .iter()
                .any(|field| field.as_str() == Some("核查情况")),
        "verification rate detail explain should expose warning_list columns, got {detail_fields:?}"
    );
}

#[test]
fn eval_spbjw_realtime_warnings_cockpit_table_rows_and_status() {
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
    .unwrap_or_else(|e| panic!("compile home preview failed: {e}"));
    let owner = format!("__world_metrics__::scenes/06-实时预警.mei::metrics");
    let owner_dataset = compiled
        .resources
        .iter()
        .find(|r| r.id == owner)
        .and_then(|r| r.dataset.as_ref())
        .unwrap_or_else(|| panic!("missing `{owner}`"));
    let datasets: BTreeMap<_, _> = compiled
        .resources
        .iter()
        .filter_map(|r| r.dataset.clone().map(|d| (r.id.clone(), d)))
        .collect();
    let table_key = "scenes/06-实时预警.mei::warnings_realtime_cockpit_table";
    let metrics = evaluate_runtime_metric_defs(
        &owner_dataset.runtime_metric_defs,
        &[],
        &datasets,
        Some(&[table_key.to_string()]),
    )
    .unwrap_or_else(|e| panic!("evaluate realtime warnings table failed: {e}"));
    let table = metrics
        .get(table_key)
        .unwrap_or_else(|| panic!("missing metric `{table_key}`"));
    let rows = table
        .value
        .as_array()
        .unwrap_or_else(|| panic!("dataframe rows expected, got {:?}", table.value));
    assert!(
        rows.len() > 3,
        "realtime warnings should exceed one carousel page (pageSize=3), got {}",
        rows.len()
    );
    for row in rows {
        let status = row
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        assert!(
            status == "待办" || status == "在办",
            "status should be 待办/在办, got {status:?} row={row:?}"
        );
    }
}

#[test]
fn query_realtime_warning_detail_rowset_with_warning_id_filter() {
    use mei_lang_datasets::{query_metric_dataframe, DatasetQueryOptions};
    use std::collections::BTreeMap;

    let source_root = source_root();
    let app_root = zhifa_app_root();
    let target = "scenes/06-实时预警.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some(target.to_string()),
        },
    )
    .expect("compile realtime warnings preview");
    let table = query_metric_dataframe(
        &compiled,
        app_root.as_path(),
        "__world_metrics__",
        "warnings_realtime_cockpit_table",
        Some("realtime_warnings"),
        Some(target),
        "integration-test",
        DatasetQueryOptions {
            page: 1,
            page_size: 5,
            collect_all: false,
            ..DatasetQueryOptions::default()
        },
        None,
        Vec::new(),
    )
    .expect("cockpit table");
    let warning_id = table
        .rows
        .first()
        .and_then(|row| row.get("warning_id"))
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .expect("warning_id sample");
    let mut filters = BTreeMap::new();
    filters.insert("预警ID".to_string(), warning_id.to_string());
    let detail = query_metric_dataframe(
        &compiled,
        app_root.as_path(),
        "warning_list",
        "realtime_warning_detail::__scalar_rowset__",
        Some("realtime_warnings"),
        Some(target),
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
    );
    eprintln!(
        "detail result: {:?}",
        detail.as_ref().map(|d| d.rows.len()).ok()
    );
    let detail = detail.expect("realtime_warning_detail rowset with warningId filter");
    assert!(
        !detail.rows.is_empty(),
        "filtered detail should return rows"
    );
}

#[test]
fn query_realtime_warning_detail_rowset_via_warning_detail_card_board() {
    use mei_lang_datasets::{query_metric_dataframe, DatasetQueryOptions};
    use std::collections::BTreeMap;

    let source_root = source_root();
    let app_root = zhifa_app_root();
    let host_target = "scenes/06-实时预警.mei";
    let board_target = "scenes/_shared/warning-detail.card.board.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some(board_target.to_string()),
        },
    )
    .expect("compile warning detail card board");
    let table = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some(host_target.to_string()),
        },
    )
    .expect("compile realtime warnings preview");
    let table = query_metric_dataframe(
        &table,
        app_root.as_path(),
        "__world_metrics__",
        "warnings_realtime_cockpit_table",
        Some("realtime_warnings"),
        Some(host_target),
        "integration-test",
        DatasetQueryOptions {
            page: 1,
            page_size: 5,
            collect_all: false,
            ..DatasetQueryOptions::default()
        },
        None,
        Vec::new(),
    )
    .expect("cockpit table");
    let warning_id = table
        .rows
        .first()
    .and_then(|row| row.get("warning_id"))
    .and_then(|v| v.as_str())
    .map(str::trim)
    .filter(|v| !v.is_empty())
    .expect("warning_id sample");
    let mut filters = BTreeMap::new();
    filters.insert("预警ID".to_string(), warning_id.to_string());
    let detail = query_metric_dataframe(
        &compiled,
        app_root.as_path(),
        "warning_list",
        "realtime_warning_detail::__scalar_rowset__",
        Some("warning_detail_card_board"),
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
    .expect("realtime_warning_detail rowset via warning detail card board");
    assert!(
        !detail.rows.is_empty(),
        "board overlay should resolve realtime_warning_detail rowset"
    );
}

#[test]
fn query_issue_handling_detail_rowset_via_issue_handling_detail_card_board() {
    use mei_lang_datasets::{query_metric_dataframe, DatasetQueryOptions};

    let source_root = source_root();
    let app_root = zhifa_app_root();
    let board_target = "scenes/_shared/issue-clue-detail.card.board.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: Some("issue_handling_detail_card_board".to_string()),
            preview_target: Some(board_target.to_string()),
        },
    )
    .expect("compile issue handling detail card board");
    let detail = query_metric_dataframe(
        &compiled,
        app_root.as_path(),
        "scenes/07-问题办理.mei::warning_list",
        "scenes/07-问题办理.mei::effectiveness_completed_count::__scalar_rowset__",
        Some("issue_handling_detail_card_board"),
        Some(board_target),
        "integration-test",
        DatasetQueryOptions {
            page: 1,
            page_size: 5,
            collect_all: false,
            ..DatasetQueryOptions::default()
        },
        None,
        Vec::new(),
    )
    .expect("issue handling completed rowset via detail card board");
    assert!(
        !detail.rows.is_empty(),
        "issue handling detail card should resolve 07 warning_list metric rowset"
    );
}
