//! L3 xlsx 表快照加载 singleflight，避免并发冷读重复解析同一文件。

use std::collections::HashMap;
use std::env;
use std::sync::{Arc, Condvar, Mutex as StdMutex, OnceLock};

use anyhow::{anyhow, Result};

use super::loaders::XlsxTableSnapshot;

pub(super) struct XlsxInflight {
    pub(super) result: StdMutex<Option<Result<Arc<XlsxTableSnapshot>, String>>>,
    pub(super) ready: Condvar,
}

fn xlsx_inflight_map() -> &'static StdMutex<HashMap<String, Arc<XlsxInflight>>> {
    static XLSX_INFLIGHT: OnceLock<StdMutex<HashMap<String, Arc<XlsxInflight>>>> = OnceLock::new();
    XLSX_INFLIGHT.get_or_init(|| StdMutex::new(HashMap::new()))
}

fn env_flag_enabled(name: &str) -> bool {
    env::var(name).ok().is_some_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
}

fn env_list_contains(name: &str, needle: &str) -> bool {
    env::var(name).ok().is_some_and(|value| {
        value
            .split(',')
            .map(|item| item.trim().to_ascii_lowercase())
            .any(|item| item == needle)
    })
}

pub(super) fn xlsx_singleflight_enabled() -> bool {
    if env_flag_enabled("MEI_DISABLE_XLSX_SINGLEFLIGHT") {
        return false;
    }
    !env_list_contains("MEI_PERF_DISABLE", "xlsx_singleflight")
}

pub(super) fn register_xlsx_inflight(cache_key: &str) -> Option<(Arc<XlsxInflight>, bool)> {
    let map = xlsx_inflight_map();
    let mut guard = map.lock().ok()?;
    if let Some(entry) = guard.get(cache_key) {
        return Some((entry.clone(), false));
    }
    let entry = Arc::new(XlsxInflight {
        result: StdMutex::new(None),
        ready: Condvar::new(),
    });
    guard.insert(cache_key.to_string(), entry.clone());
    Some((entry, true))
}

pub(super) fn finish_xlsx_inflight(
    cache_key: &str,
    entry: &Arc<XlsxInflight>,
    result: Result<Arc<XlsxTableSnapshot>, String>,
) {
    if let Ok(mut slot) = entry.result.lock() {
        *slot = Some(result);
        entry.ready.notify_all();
    }
    if let Ok(mut guard) = xlsx_inflight_map().lock() {
        guard.remove(cache_key);
    }
}

pub(super) fn wait_for_xlsx_inflight(entry: &Arc<XlsxInflight>) -> Result<Arc<XlsxTableSnapshot>> {
    let mut slot = entry
        .result
        .lock()
        .map_err(|_| anyhow!("xlsx inflight lock poisoned"))?;
    while slot.is_none() {
        slot = entry
            .ready
            .wait(slot)
            .map_err(|_| anyhow!("xlsx inflight wait poisoned"))?;
    }
    slot.clone()
        .ok_or_else(|| anyhow!("xlsx inflight finished without result"))?
        .map_err(|error| anyhow!(error))
}

pub(super) fn clear_xlsx_inflight_for_tests() {
    if let Ok(mut guard) = xlsx_inflight_map().lock() {
        guard.clear();
    }
}
