use std::collections::HashMap;
use std::env;
use std::sync::{Arc, Condvar, Mutex as StdMutex, OnceLock};

use mei_lang_kernel::CompiledApp;

pub(super) struct CompileInflight {
    pub(super) result: StdMutex<Option<Result<CompiledApp, String>>>,
    pub(super) ready: Condvar,
}

pub(super) fn compile_inflight_map() -> &'static StdMutex<HashMap<String, Arc<CompileInflight>>> {
    static COMPILE_INFLIGHT: OnceLock<StdMutex<HashMap<String, Arc<CompileInflight>>>> =
        OnceLock::new();
    COMPILE_INFLIGHT.get_or_init(|| StdMutex::new(HashMap::new()))
}

pub(super) fn compile_singleflight_enabled() -> bool {
    if env_flag_enabled("MEI_DISABLE_COMPILE_SINGLEFLIGHT") {
        return false;
    }
    !env_list_contains("MEI_PERF_DISABLE", "compile_singleflight")
}

pub fn env_flag_enabled(name: &str) -> bool {
    env::var(name).ok().is_some_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
}

pub(super) fn env_list_contains(name: &str, needle: &str) -> bool {
    env::var(name).ok().is_some_and(|value| {
        value
            .split(',')
            .map(|item| item.trim().to_ascii_lowercase())
            .any(|item| item == needle)
    })
}

pub(super) fn register_compile_inflight(cache_key: &str) -> Option<(Arc<CompileInflight>, bool)> {
    let map = compile_inflight_map();
    let mut guard = map.lock().ok()?;
    if let Some(entry) = guard.get(cache_key) {
        return Some((entry.clone(), false));
    }
    let entry = Arc::new(CompileInflight {
        result: StdMutex::new(None),
        ready: Condvar::new(),
    });
    guard.insert(cache_key.to_string(), entry.clone());
    Some((entry, true))
}

pub(super) fn finish_compile_inflight(
    cache_key: &str,
    entry: &Arc<CompileInflight>,
    result: Result<CompiledApp, String>,
) {
    if let Ok(mut slot) = entry.result.lock() {
        *slot = Some(result);
        entry.ready.notify_all();
    }
    if let Ok(mut guard) = compile_inflight_map().lock() {
        guard.remove(cache_key);
    }
}

pub(super) fn wait_for_compile_inflight(entry: &Arc<CompileInflight>) -> Result<CompiledApp, String> {
    let mut slot = entry
        .result
        .lock()
        .map_err(|_| "compile inflight lock poisoned".to_string())?;
    while slot.is_none() {
        slot = entry
            .ready
            .wait(slot)
            .map_err(|_| "compile inflight wait poisoned".to_string())?;
    }
    slot.clone()
        .ok_or_else(|| "compile inflight finished without result".to_string())?
}

pub fn is_compile_inflight(
    source_root: &std::path::Path,
    app_id: &str,
    options: &mei_lang_kernel::CompileOptions,
) -> bool {
    let cache_key = super::compile_cache_key(source_root, app_id, options);
    compile_inflight_map()
        .lock()
        .ok()
        .is_some_and(|guard| guard.contains_key(&cache_key))
}
