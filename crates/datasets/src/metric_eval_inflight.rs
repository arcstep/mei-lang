use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, OnceLock};

type JsonInflightMap = BTreeMap<String, Arc<Mutex<Option<Result<serde_json::Value, String>>>>>;

fn metric_eval_json_inflight_map() -> &'static Mutex<JsonInflightMap> {
    static MAP: OnceLock<Mutex<JsonInflightMap>> = OnceLock::new();
    MAP.get_or_init(|| Mutex::new(BTreeMap::new()))
}

type ArtifactInflightValue = Option<(crate::result_artifact::LoadedMetricResponseArtifact, u64)>;
type ArtifactInflightMap =
    BTreeMap<String, Arc<Mutex<Option<Result<ArtifactInflightValue, String>>>>>;

fn metric_artifact_inflight_map() -> &'static Mutex<ArtifactInflightMap> {
    static MAP: OnceLock<Mutex<ArtifactInflightMap>> = OnceLock::new();
    MAP.get_or_init(|| Mutex::new(BTreeMap::new()))
}

type InflightSlot<T> = Arc<Mutex<Option<Result<T, String>>>>;
type InflightMap<T> = BTreeMap<String, InflightSlot<T>>;

fn run_singleflight<T, F>(
    map: &'static Mutex<InflightMap<T>>,
    key: String,
    execute: F,
) -> Result<T, String>
where
    T: Clone + Send + 'static,
    F: FnOnce() -> Result<T, String>,
{
    let slot = {
        let mut guard = map
            .lock()
            .map_err(|_| "metric eval inflight lock poisoned".to_string())?;
        if let Some(existing) = guard.get(&key) {
            existing.clone()
        } else {
            let entry = Arc::new(Mutex::new(None));
            guard.insert(key.clone(), entry.clone());
            entry
        }
    };
    {
        let cached = slot
            .lock()
            .map_err(|_| "metric eval inflight slot poisoned".to_string())?;
        if let Some(result) = cached.as_ref() {
            return result.clone();
        }
    }
    let produced = execute();
    {
        let mut cached = slot
            .lock()
            .map_err(|_| "metric eval inflight slot poisoned".to_string())?;
        if cached.is_none() {
            *cached = Some(produced.clone());
        }
    }
    if let Ok(mut guard) = map.lock() {
        guard.remove(&key);
    }
    produced
}

/// Coalesce concurrent serializable metric eval work keyed by idempotency key.
pub fn run_metric_eval_singleflight<T, F>(key: String, execute: F) -> Result<T, String>
where
    T: serde::Serialize + serde::de::DeserializeOwned + Clone + Send + 'static,
    F: FnOnce() -> Result<T, String>,
{
    let value = run_singleflight(metric_eval_json_inflight_map(), key, || {
        execute().and_then(|value| {
            serde_json::to_value(value)
                .map_err(|error| format!("metric eval inflight encode failed: {error}"))
        })
    })?;
    serde_json::from_value(value)
        .map_err(|error| format!("metric eval inflight decode failed: {error}"))
}

/// Coalesce concurrent metric response artifact disk loads for the same cache key.
pub fn run_metric_response_artifact_load_singleflight<F>(
    key: String,
    execute: F,
) -> Result<ArtifactInflightValue, String>
where
    F: FnOnce() -> Result<ArtifactInflightValue, String>,
{
    run_singleflight(metric_artifact_inflight_map(), key, execute)
}
