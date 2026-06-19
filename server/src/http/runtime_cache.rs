use std::collections::BTreeMap;
use std::fs;
use std::time::Instant;

use mei_lang_kernel::{clear_runtime_compile_caches, data_snapshot_store_root, resolve_app_root};

use crate::AppState;

use super::compile_cache::{clear_compile_cache_for_app, clear_compiled_app_artifacts_for_app};
use super::datasets::{
    clear_dataset_rows_cache, clear_eval_artifact_store, clear_external_file_cache_for_app,
    clear_metric_dataframe_result_cache, clear_metric_response_cache,
};
use super::pages::clear_page_render_cache;

fn elapsed_ms(started: Instant) -> u64 {
    started.elapsed().as_millis() as u64
}

#[derive(Debug, Default)]
pub(crate) struct RuntimeCacheInvalidateReport {
    pub compile_cache_cleared: usize,
    pub compiled_app_artifacts_cleared: usize,
    pub page_render_cache_cleared: usize,
    pub file_cache_cleared: usize,
    pub import_artifacts_cleared: usize,
    pub metric_response_cache_cleared: usize,
    pub metric_dataframe_cache_cleared: usize,
    pub dataset_rows_cache_cleared: usize,
    pub eval_artifacts_cleared: usize,
    pub clear_ms: u64,
}

fn count_files_recursively(path: &std::path::Path) -> usize {
    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| entry.path())
        .map(|child| {
            if child.is_file() {
                1
            } else if child.is_dir() {
                count_files_recursively(&child)
            } else {
                0
            }
        })
        .sum()
}

pub(crate) fn invalidate_app_runtime_caches(
    state: &AppState,
    app_id: &str,
) -> RuntimeCacheInvalidateReport {
    let started = Instant::now();
    let app_root = resolve_app_root(state.source_root.as_path(), app_id);
    let compile_cache_cleared = clear_compile_cache_for_app(&state, app_id);
    let compiled_app_artifacts_cleared = clear_compiled_app_artifacts_for_app(&state, app_id);
    let page_render_cache_cleared = clear_page_render_cache();
    let file_cache_cleared = clear_external_file_cache_for_app(app_root.as_path());
    let import_root = data_snapshot_store_root(app_root.as_path());
    let import_artifacts_cleared = count_files_recursively(import_root.as_path());
    let _ = fs::remove_dir_all(import_root);
    let metric_response_cache_cleared = clear_metric_response_cache();
    let metric_dataframe_cache_cleared = clear_metric_dataframe_result_cache();
    let dataset_rows_cache_cleared = clear_dataset_rows_cache();
    let eval_artifacts_cleared = clear_eval_artifact_store(app_root.as_path());
    clear_runtime_compile_caches();
    RuntimeCacheInvalidateReport {
        compile_cache_cleared,
        compiled_app_artifacts_cleared,
        page_render_cache_cleared,
        file_cache_cleared,
        import_artifacts_cleared,
        metric_response_cache_cleared,
        metric_dataframe_cache_cleared,
        dataset_rows_cache_cleared,
        eval_artifacts_cleared,
        clear_ms: elapsed_ms(started),
    }
}

pub(crate) fn invalidate_report_perf(report: &RuntimeCacheInvalidateReport) -> BTreeMap<String, u64> {
    let mut perf = BTreeMap::new();
    perf.insert("clear_ms".to_string(), report.clear_ms);
    perf.insert(
        "compile_cache_cleared".to_string(),
        report.compile_cache_cleared as u64,
    );
    perf.insert(
        "compiled_app_artifacts_cleared".to_string(),
        report.compiled_app_artifacts_cleared as u64,
    );
    perf.insert(
        "page_render_cache_cleared".to_string(),
        report.page_render_cache_cleared as u64,
    );
    perf.insert(
        "file_cache_cleared".to_string(),
        report.file_cache_cleared as u64,
    );
    perf.insert(
        "import_artifacts_cleared".to_string(),
        report.import_artifacts_cleared as u64,
    );
    perf.insert(
        "metric_response_cache_cleared".to_string(),
        report.metric_response_cache_cleared as u64,
    );
    perf.insert(
        "metric_dataframe_cache_cleared".to_string(),
        report.metric_dataframe_cache_cleared as u64,
    );
    perf.insert(
        "dataset_rows_cache_cleared".to_string(),
        report.dataset_rows_cache_cleared as u64,
    );
    perf.insert(
        "eval_artifacts_cleared".to_string(),
        report.eval_artifacts_cleared as u64,
    );
    perf
}
