use serde_json::Value;
use ws_spbjw_integration_tests::{
    compile_app_from_root_with_options, evaluate_runtime_metric_defs, source_root, zhifa_app_root,
    CompileOptions,
};

#[test]
fn spbjw_indicator_system_calendar_year_metrics_use_inspection_xlsx_check_date() {
    use std::collections::BTreeMap;

    use ws_spbjw_integration_tests::MetricShape;
    use ws_spbjw_integration_tests::{coerce_rows_to_schema, load_xlsx_table_snapshot};

    let source_root = source_root();
    let app_root = zhifa_app_root();
    let target = "scenes/03-指标体系.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some(target.to_string()),
        },
    )
    .unwrap_or_else(|error| panic!("compile `{target}` failed: {error}"));

    let xlsx_path = app_root.join("upload/5.行政检查结果清单.xlsx");
    assert!(
        xlsx_path.is_file(),
        "expected inspection workbook at {}",
        xlsx_path.display()
    );
    let snapshot = load_xlsx_table_snapshot(
        &xlsx_path,
        "upload/5.行政检查结果清单.xlsx",
        Some("总表"),
        1,
        None,
    )
    .expect("load full inspection xlsx");
    assert!(
        !snapshot.rows.is_empty(),
        "inspection xlsx should contain rows"
    );

    let inspection_resource = compiled
        .resources
        .iter()
        .find(|resource| resource.id == "administrative_inspection")
        .and_then(|resource| resource.dataset.as_ref())
        .unwrap_or_else(|| {
            panic!(
                "compiled preview should expose administrative_inspection dataset; ids: {:?}",
                compiled
                    .resources
                    .iter()
                    .filter(|r| r.dataset.is_some())
                    .map(|r| r.id.as_str())
                    .collect::<Vec<_>>()
            )
        });
    assert!(
        inspection_resource
            .schema
            .iter()
            .any(|column| column.name == "检查日期" && column.type_name == "date"),
        "administrative_inspection schema should declare 检查日期 as date: {:?}",
        inspection_resource.schema
    );

    let coerced_rows = coerce_rows_to_schema(snapshot.rows.clone(), &inspection_resource.schema);
    let count_2024 = coerced_rows
        .iter()
        .filter(|row| {
            row.get("检查日期")
                .and_then(|value| value.as_str())
                .map(|text| text.starts_with("2024"))
                .unwrap_or(false)
        })
        .count();
    let count_2025 = coerced_rows
        .iter()
        .filter(|row| {
            row.get("检查日期")
                .and_then(|value| value.as_str())
                .map(|text| text.starts_with("2025"))
                .unwrap_or(false)
        })
        .count();
    assert!(
        count_2024 > 0 && count_2025 > 0,
        "检查日期 should span 2024 and 2025 after schema coerce (2024={count_2024}, 2025={count_2025})"
    );

    let owner_dataset = compiled
        .resources
        .iter()
        .find(|resource| {
            resource.dataset.as_ref().is_some_and(|dataset| {
                dataset
                    .runtime_metric_defs
                    .contains_key("inspection_frequency_reduction_rate")
            })
        })
        .and_then(|resource| resource.dataset.as_ref())
        .unwrap_or_else(|| panic!("indicator metrics should be on a runtime metric owner"));

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

    let preview_only = evaluate_runtime_metric_defs(
        &owner_dataset.runtime_metric_defs,
        &[],
        &datasets,
        Some(&["inspection_frequency_reduction_rate".to_string()]),
    )
    .expect("evaluate on compile-preview rows");
    let preview_value = preview_only
        .get("inspection_frequency_reduction_rate")
        .and_then(|metric| {
            metric
                .value
                .get("value")
                .and_then(|v| v.as_f64())
                .or_else(|| metric.value.as_f64())
        })
        .unwrap_or(0.0);
    assert!(
        preview_value.is_finite() && preview_value.abs() > f64::EPSILON,
        "preview-materialized rows should already yield non-zero inspection_frequency_reduction_rate, got {preview_value}"
    );

    if let Some(dataset) = datasets.get_mut("administrative_inspection") {
        dataset.rows = coerced_rows;
    }

    let metrics = evaluate_runtime_metric_defs(
        &owner_dataset.runtime_metric_defs,
        &[],
        &datasets,
        Some(&[
            "inspection_frequency_reduction_rate".to_string(),
            "penalty_revenue_growth_rate".to_string(),
        ]),
    )
    .expect("evaluate indicator system calendar year metrics");

    let inspection_rate = metrics
        .get("inspection_frequency_reduction_rate")
        .expect("inspection_frequency_reduction_rate metric");
    assert_eq!(inspection_rate.shape, MetricShape::Scalar);
    let inspection_value = inspection_rate
        .value
        .get("value")
        .and_then(|v| v.as_f64())
        .or_else(|| inspection_rate.value.as_f64())
        .unwrap_or(0.0);
    assert!(
        inspection_value.is_finite() && inspection_value.abs() > f64::EPSILON,
        "inspection_frequency_reduction_rate should be non-zero with full xlsx rows, got {inspection_value}"
    );

    let penalty_schema = datasets
        .get("penalty_result_list")
        .expect("penalty_result_list dataset")
        .schema
        .clone();
    let penalty_path = app_root.join("upload/8.行政处罚结果清单.xlsx");
    let penalty_snapshot = load_xlsx_table_snapshot(
        &penalty_path,
        "upload/8.行政处罚结果清单.xlsx",
        None,
        1,
        None,
    )
    .expect("load full penalty xlsx");
    if let Some(dataset) = datasets.get_mut("penalty_result_list") {
        dataset.rows = coerce_rows_to_schema(penalty_snapshot.rows, &penalty_schema);
    }
    let metrics = evaluate_runtime_metric_defs(
        &owner_dataset.runtime_metric_defs,
        &[],
        &datasets,
        Some(&["penalty_revenue_growth_rate".to_string()]),
    )
    .expect("evaluate penalty revenue growth");
    let penalty_rate = metrics
        .get("penalty_revenue_growth_rate")
        .expect("penalty_revenue_growth_rate metric");
    let penalty_value = penalty_rate
        .value
        .get("value")
        .and_then(|v| v.as_f64())
        .or_else(|| penalty_rate.value.as_f64())
        .unwrap_or(0.0);
    assert!(
        penalty_value.is_finite() && penalty_value.abs() > f64::EPSILON,
        "penalty_revenue_growth_rate should be non-zero with full penalty rows, got {penalty_value}"
    );
}

#[test]
fn spbjw_home_scene_compile_includes_administrative_inspection_dataset() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: Some("home".to_string()),
            preview_target: None,
        },
    )
    .expect("compile home scene (access-style)");
    let inspection = compiled
        .resources
        .iter()
        .find(|r| {
            r.id == "administrative_inspection"
                || r.dataset
                    .as_ref()
                    .is_some_and(|d| d.id == "administrative_inspection")
        })
        .and_then(|r| r.dataset.as_ref());
    assert!(
        inspection.is_some(),
        "home scene compile must include administrative_inspection for indicator metrics; dataset ids: {:?}",
        compiled
            .resources
            .iter()
            .filter(|r| r.dataset.is_some())
            .map(|r| r.id.as_str())
            .collect::<Vec<_>>()
    );
    let inspection = inspection.unwrap();
    assert!(
        !inspection.rows.is_empty(),
        "administrative_inspection should have preview rows on home compile"
    );
    assert!(
        inspection.schema.iter().any(|c| c.name == "检查日期"),
        "schema must include 检查日期"
    );
}

#[test]
fn spbjw_home_preview_imported_indicator_metrics_nonzero() {
    use std::collections::BTreeMap;

    use ws_spbjw_integration_tests::resolve_runtime_metric_def_key;
    use ws_spbjw_integration_tests::MetricShape;
    use ws_spbjw_integration_tests::{coerce_rows_to_schema, load_xlsx_table_snapshot};

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

    let resource_id = "__world_metrics__::scenes/03-指标体系.mei::metrics";
    let metric_key = "scenes/03-指标体系.mei::inspection_frequency_reduction_rate".to_string();
    let owner_dataset = compiled
        .resources
        .iter()
        .find(|resource| resource.id == resource_id)
        .and_then(|resource| resource.dataset.as_ref())
        .unwrap_or_else(|| {
            panic!(
                "home preview should include `{resource_id}`, resources: {:?}",
                compiled
                    .resources
                    .iter()
                    .map(|r| r.id.as_str())
                    .collect::<Vec<_>>()
            )
        });
    let resolved = resolve_runtime_metric_def_key(
        resource_id,
        "inspection_frequency_reduction_rate",
        &owner_dataset.runtime_metric_defs,
    )
    .unwrap_or_else(|| panic!("resolve imported metric key"));
    assert_eq!(resolved, metric_key.as_str());

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
    datasets
        .entry("administrative_inspection".to_string())
        .or_insert_with(|| {
            panic!("home compile should include administrative_inspection in datasets map")
        });

    let preview_metrics = evaluate_runtime_metric_defs(
        &owner_dataset.runtime_metric_defs,
        &[],
        &datasets,
        Some(&[metric_key.clone()]),
    )
    .expect("evaluate on home preview rows without hydrate");
    let preview_value = preview_metrics
        .get(&metric_key)
        .and_then(|metric| {
            metric
                .value
                .get("value")
                .and_then(|v| v.as_f64())
                .or_else(|| metric.value.as_f64())
        })
        .unwrap_or(0.0);
    assert!(
        preview_value.abs() > f64::EPSILON,
        "home preview rows alone should yield non-zero imported metric, got {preview_value}"
    );

    let xlsx_path = app_root.join("upload/5.行政检查结果清单.xlsx");
    let snapshot = load_xlsx_table_snapshot(
        &xlsx_path,
        "upload/5.行政检查结果清单.xlsx",
        Some("总表"),
        1,
        None,
    )
    .expect("load inspection xlsx");
    let schema = datasets
        .get("administrative_inspection")
        .expect("administrative_inspection")
        .schema
        .clone();
    if let Some(dataset) = datasets.get_mut("administrative_inspection") {
        dataset.rows = coerce_rows_to_schema(snapshot.rows, &schema);
    }

    let metric_ids = vec![metric_key.clone()];
    let metrics = evaluate_runtime_metric_defs(
        &owner_dataset.runtime_metric_defs,
        &[],
        &datasets,
        Some(metric_ids.as_slice()),
    )
    .expect("evaluate imported home metric");
    let metric = metrics
        .get(&metric_key)
        .or_else(|| metrics.get("inspection_frequency_reduction_rate"))
        .expect("imported metric result");
    assert_eq!(metric.shape, MetricShape::Scalar);
    let value = metric
        .value
        .get("value")
        .and_then(|v| v.as_f64())
        .or_else(|| metric.value.as_f64())
        .unwrap_or(0.0);
    assert!(
        value.is_finite() && value.abs() > f64::EPSILON,
        "home imported inspection_frequency_reduction_rate should be non-zero, got {value}"
    );
}

