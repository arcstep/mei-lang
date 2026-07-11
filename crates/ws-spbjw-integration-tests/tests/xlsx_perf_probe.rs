//! 手动性能探测：`cargo test -p ws-spbjw-integration-tests xlsx_perf_probe -- --ignored --nocapture`
//!
//! 量化 xlsx 冷/热读、JSON 物化、并发锁等待，对照宿主日志里的 query_api_ms / total_ms 裂口。

use std::collections::BTreeMap;
use std::sync::Arc;
use std::thread;
use std::time::Instant;

use mei_lang_kernel::{
    cached_load_xlsx_table_snapshot, clear_runtime_compile_caches, coerce_rows_to_schema,
    load_xlsx_table_snapshot, ColumnSchema,
};
use ws_spbjw_integration_tests::zhifa_app_root;

fn elapsed_ms(start: Instant) -> u64 {
    start.elapsed().as_millis() as u64
}

fn print_phase(label: &str, ms: u64, extra: &str) {
    eprintln!("{label:40} {ms:6} ms  {extra}");
}

#[test]
#[ignore = "manual perf probe; run with --ignored --nocapture"]
fn xlsx_perf_probe_zhifa_hot_files() {
    let app_root = zhifa_app_root();
    let cases = [
        (
            "upload/5.行政检查结果清单.xlsx",
            "行政检查主表（~42k 行热点）",
        ),
        ("upload/8.行政处罚结果清单.xlsx", "行政处罚主表"),
    ];

    for (rel_path, label) in cases {
        eprintln!("\n======== {label} ({rel_path}) ========");
        let path = app_root.join(rel_path);
        assert!(path.is_file(), "missing {}", path.display());

        clear_runtime_compile_caches();

        let cold = Instant::now();
        let snapshot = load_xlsx_table_snapshot(&path, rel_path, None, 1, None)
            .expect("cold load_xlsx_table_snapshot");
        let cold_ms = elapsed_ms(cold);
        let row_count = snapshot.rows.len();
        let col_count = snapshot.columns.len();
        print_phase(
            "cold calamine+serde_json",
            cold_ms,
            &format!("rows={row_count} cols={col_count}"),
        );

        let warm1 = Instant::now();
        let (cached1, hit1) = cached_load_xlsx_table_snapshot(&app_root, rel_path, None, 1)
            .expect("warm1 cached load");
        let warm1_ms = elapsed_ms(warm1);
        print_phase(
            "cached_load miss (1st)",
            warm1_ms,
            &format!("hit={hit1} rows={}", cached1.rows.len()),
        );

        let warm2 = Instant::now();
        let (cached2, hit2) = cached_load_xlsx_table_snapshot(&app_root, rel_path, None, 1)
            .expect("warm2 cached load");
        let warm2_ms = elapsed_ms(warm2);
        print_phase(
            "cached_load hit (2nd)",
            warm2_ms,
            &format!("hit={hit2} rows={}", cached2.rows.len()),
        );

        let schema: Vec<ColumnSchema> = snapshot
            .columns
            .iter()
            .take(8)
            .map(|name| ColumnSchema {
                name: name.clone(),
                type_name: "string".to_string(),
                source: None,
                optional: false,
                unit: None,
            })
            .collect();

        let coerce = Instant::now();
        let coerced = coerce_rows_to_schema(snapshot.rows.clone(), &schema);
        let coerce_ms = elapsed_ms(coerce);
        print_phase(
            "coerce_rows_to_schema (8 cols)",
            coerce_ms,
            &format!("rows={}", coerced.len()),
        );

        let clone_rows = Instant::now();
        let _dup = snapshot.rows.clone();
        let clone_ms = elapsed_ms(clone_rows);
        print_phase("clone all JSON rows", clone_ms, "");

        // 模拟 metric_dataframe 路径：collect_all query + hydrate 各 clone 一次
        let metric_path = Instant::now();
        let _a = snapshot.rows.clone();
        let _b = coerced.clone();
        let metric_path_ms = elapsed_ms(metric_path);
        print_phase("sim metric path double-clone", metric_path_ms, "");

        // 并发冷启动：6 个线程同时抢同一张表（对照 home 首开多 metric 并发）
        clear_runtime_compile_caches();
        let app = Arc::new(app_root.clone());
        let rel = Arc::new(rel_path.to_string());
        let parallel_started = Instant::now();
        let handles: Vec<_> = (0..6)
            .map(|tid| {
                let app = Arc::clone(&app);
                let rel = Arc::clone(&rel);
                thread::spawn(move || {
                    let t0 = Instant::now();
                    let (snap, hit) = cached_load_xlsx_table_snapshot(&app, rel.as_str(), None, 1)
                        .expect("parallel cached load");
                    (tid, elapsed_ms(t0), hit, snap.rows.len())
                })
            })
            .collect();
        let mut per_thread = Vec::new();
        for h in handles {
            per_thread.push(h.join().expect("thread join"));
        }
        let parallel_total_ms = elapsed_ms(parallel_started);
        print_phase("parallel x6 cold L3 load (wall)", parallel_total_ms, "");
        for (tid, ms, hit, rows) in &per_thread {
            eprintln!("  thread {tid}: {ms} ms hit={hit} rows={rows}");
        }
        let max_thread_ms = per_thread
            .iter()
            .map(|(_, ms, _, _)| *ms)
            .max()
            .unwrap_or(0);
        let min_thread_ms = per_thread
            .iter()
            .map(|(_, ms, _, _)| *ms)
            .min()
            .unwrap_or(0);
        eprintln!(
            "  thread spread: min={min_thread_ms}ms max={max_thread_ms}ms (lock convoy indicator)"
        );

        assert!(hit2, "second cached_load should hit L3");
        let follower_hits = per_thread.iter().filter(|(_, _, hit, _)| *hit).count();
        assert!(
            follower_hits >= 5,
            "singleflight should let followers reuse leader snapshot, got {follower_hits}/6 hits"
        );
        assert!(
            parallel_total_ms <= warm1_ms.saturating_mul(3).max(1),
            "parallel wall {parallel_total_ms}ms should stay near single load {}ms",
            warm1_ms
        );

        if mei_lang_kernel::resolve_data_snapshot_import_entry(&app_root, rel_path, None, 1)
            .is_some()
        {
            use mei_lang_datasets::{query_dataset_rows, DatasetQueryOptions};
            use mei_lang_kernel::DatasetView;
            let dataset = DatasetView {
                id: rel_path.replace('/', "_"),
                title: Some(rel_path.to_string()),
                purpose: None,
                schema: Vec::new(),
                stage_schema: Vec::new(),
                columns: Vec::new(),
                rows: Vec::new(),
                source: mei_lang_kernel::SourceDecl {
                    kind: "file".to_string(),
                    path: rel_path.to_string(),
                    sheet: None,
                    header_row: Some(1),
                    preview_rows: None,
                    page_size: None,
                    max_page_size: None,
                    table: None,
                    query: None,
                    connection: None,
                    content: None,
                },
                sources: Vec::new(),
                metrics: BTreeMap::new(),
                runtime_metric_defs: BTreeMap::new(),
                runtime_analysis_graph: Default::default(),
                runtime_analysis_contracts: BTreeMap::new(),
            };
            let options = DatasetQueryOptions {
                collect_all: true,
                ..Default::default()
            };
            let handle_started = Instant::now();
            let result =
                query_dataset_rows(&app_root, &dataset, options).expect("table handle query");
            let handle_ms = elapsed_ms(handle_started);
            let table_handle_hit = result.perf.get("table_handle_hit").copied().unwrap_or(0);
            print_phase(
                "table_handle query (import path)",
                handle_ms,
                &format!(
                    "table_handle_hit={table_handle_hit} import_hit={}",
                    result
                        .perf
                        .get("dataset_import_artifact_hit")
                        .copied()
                        .unwrap_or(0)
                ),
            );
        }
    }
}
