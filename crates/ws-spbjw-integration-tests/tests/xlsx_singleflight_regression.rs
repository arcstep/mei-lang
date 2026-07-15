//! xlsx L3 singleflight 回归：并发冷读应复用同一份快照。

use std::sync::Arc;
use std::thread;

use mei_lang_kernel::{cached_load_xlsx_table_snapshot, clear_runtime_compile_caches};
use ws_spbjw_integration_tests::zhifa_app_root;

#[test]
fn xlsx_parallel_cold_load_reuses_single_snapshot() {
    let Some(app_root) = zhifa_app_root() else {
        eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
        return;
    };
    let rel_path = "upload/8.行政处罚结果清单.xlsx";
    if !app_root.join(rel_path).is_file() {
        eprintln!("skip: zhifa xlsx fixture missing");
        return;
    }

    clear_runtime_compile_caches();
    let app = Arc::new(app_root);
    let rel = Arc::new(rel_path.to_string());
    let handles: Vec<_> = (0..6)
        .map(|_| {
            let app = Arc::clone(&app);
            let rel = Arc::clone(&rel);
            thread::spawn(move || {
                cached_load_xlsx_table_snapshot(app.as_path(), rel.as_str(), None, 1)
                    .expect("parallel load")
            })
        })
        .collect();

    let mut row_counts = Vec::new();
    let mut hits = 0usize;
    for handle in handles {
        let (snapshot, hit) = handle.join().expect("join");
        if hit {
            hits += 1;
        }
        row_counts.push(snapshot.rows.len());
    }
    assert!(
        row_counts.windows(2).all(|w| w[0] == w[1]),
        "row counts should match across threads"
    );
    assert!(
        hits >= 5,
        "at least 5/6 threads should observe cache hit via singleflight, got {hits}"
    );
}
