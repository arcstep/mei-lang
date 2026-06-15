use std::collections::BTreeMap;
use std::time::Instant;

use mei_lang_kernel::{clear_runtime_compile_caches, resolve_app_root};

use crate::AppState;

use super::compile_cache::clear_compile_cache_for_app;
use super::datasets::{
    clear_external_file_cache_for_app, clear_metric_dataframe_result_cache,
    clear_metric_response_cache,
};
use super::pages::clear_page_render_cache;

fn elapsed_ms(started: Instant) -> u64 {
    started.elapsed().as_millis() as u64
}

#[derive(Debug, Default)]
pub(crate) struct RuntimeCacheInvalidateReport {
    pub compile_cache_cleared: usize,
    pub page_render_cache_cleared: usize,
    pub file_cache_cleared: usize,
    pub metric_response_cache_cleared: usize,
    pub metric_dataframe_cache_cleared: usize,
    pub clear_ms: u64,
}

pub(crate) fn invalidate_app_runtime_caches(
    state: &AppState,
    app_id: &str,
) -> RuntimeCacheInvalidateReport {
    let started = Instant::now();
    let app_root = resolve_app_root(state.source_root.as_path(), app_id);
    let compile_cache_cleared = clear_compile_cache_for_app(&state, app_id);
    let page_render_cache_cleared = clear_page_render_cache();
    let file_cache_cleared = clear_external_file_cache_for_app(app_root.as_path());
    let metric_response_cache_cleared = clear_metric_response_cache();
    let metric_dataframe_cache_cleared = clear_metric_dataframe_result_cache();
    clear_runtime_compile_caches();
    RuntimeCacheInvalidateReport {
        compile_cache_cleared,
        page_render_cache_cleared,
        file_cache_cleared,
        metric_response_cache_cleared,
        metric_dataframe_cache_cleared,
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
        "page_render_cache_cleared".to_string(),
        report.page_render_cache_cleared as u64,
    );
    perf.insert(
        "file_cache_cleared".to_string(),
        report.file_cache_cleared as u64,
    );
    perf.insert(
        "metric_response_cache_cleared".to_string(),
        report.metric_response_cache_cleared as u64,
    );
    perf.insert(
        "metric_dataframe_cache_cleared".to_string(),
        report.metric_dataframe_cache_cleared as u64,
    );
    perf
}
