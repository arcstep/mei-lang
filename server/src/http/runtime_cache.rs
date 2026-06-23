use std::collections::BTreeMap;
use std::time::Instant;

use mei_lang_kernel::{clear_runtime_compile_caches, resolve_app_root};

use crate::AppState;

use super::compile_cache::clear_compile_cache_for_app;
use super::datasets::{
    clear_dataset_rows_cache, clear_external_file_cache_for_app,
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

/// Drop in-process runtime caches so the next request reloads from prebuild artifacts.
///
/// Prebuild disk artifacts (`compiled_app`, import snapshots, eval/result stores) are
/// intentionally preserved here. They are owned by `mei-toolchain prebuild` and must
/// survive upload/recompute cache refresh on access-only hosts.
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
    let dataset_rows_cache_cleared = clear_dataset_rows_cache();
    clear_runtime_compile_caches();
    RuntimeCacheInvalidateReport {
        compile_cache_cleared,
        compiled_app_artifacts_cleared: 0,
        page_render_cache_cleared,
        file_cache_cleared,
        import_artifacts_cleared: 0,
        metric_response_cache_cleared,
        metric_dataframe_cache_cleared,
        dataset_rows_cache_cleared,
        eval_artifacts_cleared: 0,
        clear_ms: elapsed_ms(started),
    }
}

pub(crate) fn invalidate_report_perf(
    report: &RuntimeCacheInvalidateReport,
) -> BTreeMap<String, u64> {
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

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    use mei_lang_kernel::resolve_app_root;

    use super::*;
    use crate::agent_runtime::ManagedOpencodeRuntime;
    use crate::auth::AuthEnforcement;
    use crate::mei_agent::NativeAgent;
    use crate::resource_tool_bridge::SceneResourceToolExecutor;
    use crate::AppState;

    fn test_state(source_root: PathBuf) -> AppState {
        let native_agent = NativeAgent::open_with_resource_tools(
            source_root.clone(),
            Arc::new(SceneResourceToolExecutor::default()),
        )
        .expect("native agent");
        AppState {
            package_root: Arc::new(PathBuf::from(env!("CARGO_MANIFEST_DIR"))),
            source_root: Arc::new(source_root),
            agent_preferred_mode: Arc::new("native".into()),
            agent_preferred_server_url: Arc::new(String::new()),
            agent_auto_start: false,
            auth_enforcement: AuthEnforcement::Disabled,
            agent_runtime: Arc::new(Mutex::new(ManagedOpencodeRuntime::default())),
            agent_session_context: Arc::new(Mutex::new(std::collections::HashMap::new())),
            native_agent: Arc::new(native_agent),
        }
    }

    #[test]
    fn invalidate_app_runtime_caches_preserves_prebuild_disk_artifacts() {
        let source_root = std::env::temp_dir().join(format!(
            "mei-runtime-cache-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&source_root);
        fs::create_dir_all(&source_root).expect("temp source root");
        let app_id = "zhifa";
        let app_root = resolve_app_root(source_root.as_path(), app_id);
        fs::create_dir_all(&app_root).expect("app root");
        let marker = app_root
            .join(".mei")
            .join("artifacts")
            .join("compiled_app")
            .join("marker.bin");
        fs::create_dir_all(marker.parent().expect("parent")).expect("artifact dir");
        fs::write(&marker, b"prebuild").expect("marker");

        let report = invalidate_app_runtime_caches(&test_state(source_root.clone()), app_id);

        assert_eq!(report.compiled_app_artifacts_cleared, 0);
        assert_eq!(report.import_artifacts_cleared, 0);
        assert_eq!(report.eval_artifacts_cleared, 0);
        assert!(
            marker.is_file(),
            "prebuild compiled_app artifacts must survive runtime cache invalidation"
        );
        let _ = fs::remove_dir_all(source_root);
    }
}
