//! 问题办理：world.add_resource(resource_ref) 须在 capsule 加载时解析，否则指标恒为 0。

use std::collections::BTreeMap;

use super::{
    compile_app_from_root_with_options, evaluate_runtime_metric_defs, workspace_root,
    CompileOptions,
};

#[test]
fn compile_spbjw_issue_handling_world_metrics_materialize_from_resource_ref() {
    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
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
        .filter(|d| matches!(d.severity, crate::Severity::Error))
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
    let rate = metrics
        .get(rate_key)
        .unwrap_or_else(|| panic!("missing metric `{rate_key}`"));
    rate.value
        .get("value")
        .and_then(|v| v.as_f64())
        .or_else(|| rate.value.as_f64())
        .expect("effectiveness_issue_verification_rate should materialize value");

    let detail_key = "scenes/07-问题办理.mei::effectiveness_issue_verification_rate::warning_detail_rows";
    let detail_metrics = evaluate_runtime_metric_defs(
        &owner_dataset.runtime_metric_defs,
        &[],
        &datasets,
        Some(&[detail_key.to_string()]),
    )
    .unwrap_or_else(|e| panic!("evaluate warning_detail_rows failed: {e}"));
    let detail_rows = detail_metrics
        .get(detail_key)
        .and_then(|metric| metric.value.as_array())
        .unwrap_or_else(|| panic!("missing dataframe rows for `{detail_key}`"));
    let warning_detail = datasets
        .get("warning_detail")
        .or_else(|| datasets.get(&format!("{capsule}::warning_detail")))
        .unwrap_or_else(|| panic!("warning_detail dataset should be materialized"));
    assert_eq!(
        detail_rows.len(),
        warning_detail.rows.len(),
        "warning_detail_rows should match warning_detail dataset row count"
    );
    assert_eq!(
        detail_rows.len(),
        11,
        "current alert_tracking sample should have 11 verified rows (是/否), got {}",
        detail_rows.len()
    );
    for row in detail_rows {
        let verified = row
            .get("是否查实")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .trim();
        assert!(
            verified == "是" || verified == "否",
            "warning_detail_rows should only include 是/否, got {verified:?} row={row:?}"
        );
    }
}

#[test]
fn eval_spbjw_realtime_warnings_cockpit_table_rows_and_status() {
    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
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
