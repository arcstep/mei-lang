use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use serde_json::{json, Value};

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

#[derive(Debug, Default)]
struct SingleflightCounters {
    leader: AtomicU64,
    waiter: AtomicU64,
    penetration: AtomicU64,
    shared_failure: AtomicU64,
}

static SINGLEFLIGHT_COUNTERS: SingleflightCounters = SingleflightCounters {
    leader: AtomicU64::new(0),
    waiter: AtomicU64::new(0),
    penetration: AtomicU64::new(0),
    shared_failure: AtomicU64::new(0),
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SingleflightRole {
    Leader,
    Waiter,
}

#[derive(Debug, Clone)]
pub struct SingleflightOutcome<T> {
    pub value: T,
    pub role: SingleflightRole,
}

#[derive(Debug, Clone, Default)]
pub struct MetricEvalSingleflightStats {
    pub leader: u64,
    pub waiter: u64,
    pub penetration: u64,
    pub shared_failure: u64,
}

impl MetricEvalSingleflightStats {
    pub fn to_json(&self) -> Value {
        json!({
            "leader": self.leader,
            "waiter": self.waiter,
            "penetration": self.penetration,
            "sharedFailure": self.shared_failure,
        })
    }
}

pub fn snapshot_metric_eval_singleflight_stats() -> MetricEvalSingleflightStats {
    MetricEvalSingleflightStats {
        leader: SINGLEFLIGHT_COUNTERS.leader.load(Ordering::Relaxed),
        waiter: SINGLEFLIGHT_COUNTERS.waiter.load(Ordering::Relaxed),
        penetration: SINGLEFLIGHT_COUNTERS.penetration.load(Ordering::Relaxed),
        shared_failure: SINGLEFLIGHT_COUNTERS.shared_failure.load(Ordering::Relaxed),
    }
}

pub fn reset_metric_eval_singleflight_stats_for_tests() {
    SINGLEFLIGHT_COUNTERS.leader.store(0, Ordering::Relaxed);
    SINGLEFLIGHT_COUNTERS.waiter.store(0, Ordering::Relaxed);
    SINGLEFLIGHT_COUNTERS
        .penetration
        .store(0, Ordering::Relaxed);
    SINGLEFLIGHT_COUNTERS
        .shared_failure
        .store(0, Ordering::Relaxed);
}

type InflightSlot<T> = Arc<Mutex<Option<Result<T, String>>>>;
type InflightMap<T> = BTreeMap<String, InflightSlot<T>>;

fn run_singleflight<T, F>(
    map: &'static Mutex<InflightMap<T>>,
    key: String,
    execute: F,
) -> Result<SingleflightOutcome<T>, String>
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
    let outcome = {
        let mut cached = slot
            .lock()
            .map_err(|_| "metric eval inflight slot poisoned".to_string())?;
        if let Some(result) = cached.as_ref() {
            SINGLEFLIGHT_COUNTERS.waiter.fetch_add(1, Ordering::Relaxed);
            if result.is_err() {
                SINGLEFLIGHT_COUNTERS
                    .shared_failure
                    .fetch_add(1, Ordering::Relaxed);
            }
            result.clone().map(|value| SingleflightOutcome {
                value,
                role: SingleflightRole::Waiter,
            })
        } else {
            SINGLEFLIGHT_COUNTERS.leader.fetch_add(1, Ordering::Relaxed);
            SINGLEFLIGHT_COUNTERS
                .penetration
                .fetch_add(1, Ordering::Relaxed);
            let produced = execute();
            *cached = Some(produced.clone());
            produced.map(|value| SingleflightOutcome {
                value,
                role: SingleflightRole::Leader,
            })
        }
    };
    if let Ok(mut guard) = map.lock() {
        if Arc::strong_count(&slot) == 2 {
            guard.remove(&key);
        } else if matches!(
            outcome.as_ref().map(|o| o.role),
            Ok(SingleflightRole::Leader)
        ) && outcome.is_err()
        {
            guard.remove(&key);
        }
    }
    outcome
}

/// Coalesce concurrent serializable metric eval work keyed by idempotency key.
pub fn run_metric_eval_singleflight<T, F>(key: String, execute: F) -> Result<T, String>
where
    T: serde::Serialize + serde::de::DeserializeOwned + Clone + Send + 'static,
    F: FnOnce() -> Result<T, String>,
{
    let outcome = run_singleflight(metric_eval_json_inflight_map(), key, || {
        execute().and_then(|value| {
            serde_json::to_value(value)
                .map_err(|error| format!("metric eval inflight encode failed: {error}"))
        })
    })?;
    serde_json::from_value(outcome.value)
        .map_err(|error| format!("metric eval inflight decode failed: {error}"))
}

/// Whole-eval singleflight for Clone outcomes (thin eval main path).
pub fn run_whole_eval_singleflight<T, F>(
    key: String,
    execute: F,
) -> Result<SingleflightOutcome<T>, String>
where
    T: Clone + Send + Sync + 'static,
    F: FnOnce() -> Result<T, String>,
{
    run_typed_singleflight(key, execute)
}

fn run_typed_singleflight<T, F>(key: String, execute: F) -> Result<SingleflightOutcome<T>, String>
where
    T: Clone + Send + Sync + 'static,
    F: FnOnce() -> Result<T, String>,
{
    // One map per concrete T via TypeId-keyed outer map would be ideal; for the
    // thin-eval path we keep a dedicated Any-erased map with Arc payload.
    type Erased = Arc<dyn std::any::Any + Send + Sync>;
    type ErasedMap = BTreeMap<String, Arc<Mutex<Option<Result<Erased, String>>>>>;
    fn erased_map() -> &'static Mutex<ErasedMap> {
        static MAP: OnceLock<Mutex<ErasedMap>> = OnceLock::new();
        MAP.get_or_init(|| Mutex::new(BTreeMap::new()))
    }

    let slot = {
        let mut guard = erased_map()
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
    let outcome = {
        let mut cached = slot
            .lock()
            .map_err(|_| "metric eval inflight slot poisoned".to_string())?;
        if let Some(result) = cached.as_ref() {
            SINGLEFLIGHT_COUNTERS.waiter.fetch_add(1, Ordering::Relaxed);
            match result {
                Ok(any) => {
                    let typed = any
                        .downcast_ref::<T>()
                        .ok_or_else(|| "metric eval inflight type mismatch".to_string())?;
                    Ok(SingleflightOutcome {
                        value: typed.clone(),
                        role: SingleflightRole::Waiter,
                    })
                }
                Err(error) => {
                    SINGLEFLIGHT_COUNTERS
                        .shared_failure
                        .fetch_add(1, Ordering::Relaxed);
                    Err(error.clone())
                }
            }
        } else {
            SINGLEFLIGHT_COUNTERS.leader.fetch_add(1, Ordering::Relaxed);
            SINGLEFLIGHT_COUNTERS
                .penetration
                .fetch_add(1, Ordering::Relaxed);
            let produced = execute();
            *cached = Some(match &produced {
                Ok(value) => Ok(Arc::new(value.clone()) as Erased),
                Err(error) => Err(error.clone()),
            });
            produced.map(|value| SingleflightOutcome {
                value,
                role: SingleflightRole::Leader,
            })
        }
    };
    if let Ok(mut guard) = erased_map().lock() {
        if Arc::strong_count(&slot) == 2 {
            guard.remove(&key);
        } else if outcome.is_err() {
            guard.remove(&key);
        }
    }
    outcome
}

/// Coalesce concurrent metric response artifact disk loads for the same cache key.
pub fn run_metric_response_artifact_load_singleflight<F>(
    key: String,
    execute: F,
) -> Result<ArtifactInflightValue, String>
where
    F: FnOnce() -> Result<ArtifactInflightValue, String>,
{
    Ok(run_singleflight(metric_artifact_inflight_map(), key, execute)?.value)
}
