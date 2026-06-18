//! 端到端 metric dataframe 路径探测。
//! `cargo test -p ws-spbjw-integration-tests metric_dataframe_perf_probe -- --ignored --nocapture`

use std::time::Instant;

use mei_lang_datasets::{query_metric_dataframe, DatasetQueryOptions};
use mei_lang_kernel::{
    clear_runtime_compile_caches, compile_app_from_root_with_options, CompileOptions,
};
use ws_spbjw_integration_tests::zhifa_app_root;

fn elapsed_ms(start: Instant) -> u64 {
    start.elapsed().as_millis() as u64
}

#[test]
#[ignore = "manual perf probe; run with --ignored --nocapture"]
fn metric_dataframe_perf_probe_administrative_inspection() {
    let source_root = ws_spbjw_integration_tests::source_root();
    let app_root = zhifa_app_root();
    clear_runtime_compile_caches();

    let compile_started = Instant::now();
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: Some("administrative_inspection".to_string()),
            preview_target: None,
            ..CompileOptions::default()
        },
    )
    .expect("compile administrative_inspection");
    let compile_ms = elapsed_ms(compile_started);
    eprintln!("compile_app (scene-bound)              {compile_ms:6} ms");

    let dataset_id = "scenes/02-行政检查.mei::administrative_inspection_dashboard_ds";
    let metrics = [
        "inspections_6m_count_trend",
        "park_inspection_total_by_park",
        "inspections_no_violation_by_park",
    ];

    for metric_id in metrics {
        clear_runtime_compile_caches();
        let options = DatasetQueryOptions {
            page: 1,
            page_size: 64,
            collect_all: false,
            ..DatasetQueryOptions::default()
        };
        let cold = Instant::now();
        let result = query_metric_dataframe(
            &compiled,
            &app_root,
            dataset_id,
            metric_id,
            Some("home"),
            Some("main.mei"),
            "perf-probe-revision",
            options.clone(),
            None,
            Vec::new(),
        )
        .expect("cold metric dataframe");
        let cold_ms = elapsed_ms(cold);
        eprintln!("\n--- metric `{metric_id}` cold total {cold_ms} ms ---");
        for (k, v) in &result.perf {
            eprintln!("  {k}: {v}");
        }
        eprintln!("  rows returned: {}", result.rows.len());

        let warm = Instant::now();
        let result2 = query_metric_dataframe(
            &compiled,
            &app_root,
            dataset_id,
            metric_id,
            Some("home"),
            Some("main.mei"),
            "perf-probe-revision",
            options,
            None,
            Vec::new(),
        )
        .expect("warm metric dataframe");
        let warm_ms = elapsed_ms(warm);
        eprintln!("--- metric `{metric_id}` warm total {warm_ms} ms ---");
        for (k, v) in &result2.perf {
            if k.contains("cache") || k.ends_with("_ms") {
                eprintln!("  {k}: {v}");
            }
        }
    }
}
