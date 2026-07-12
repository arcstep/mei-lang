use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use mei_lang_kernel::{FilterIntent, MetricContract};

use super::serialize_cache_value;
use super::types::DatasetQueryOptions;

const METRIC_RESPONSE_CACHE_TTL_MS: u64 = 300_000;
const METRIC_RESPONSE_CACHE_PRUNE_INTERVAL_MS: u64 = 5_000;

static METRIC_RESPONSE_CACHE_TTL_OVERRIDE: OnceLock<u64> = OnceLock::new();

pub fn configure_metric_response_cache_ttl_ms(ttl_ms: u64) {
    let _ = METRIC_RESPONSE_CACHE_TTL_OVERRIDE.set(ttl_ms.max(1));
}

fn metric_response_cache_ttl() -> Duration {
    let ms = METRIC_RESPONSE_CACHE_TTL_OVERRIDE
        .get()
        .copied()
        .unwrap_or(METRIC_RESPONSE_CACHE_TTL_MS);
    Duration::from_millis(ms)
}

#[derive(Debug, Clone)]
pub struct CachedMetricResponse {
    pub total_rows: usize,
    pub metrics_map: BTreeMap<String, MetricContract>,
    pub covered_metric_ids: BTreeSet<String>,
    pub complete: bool,
    expires_at: Instant,
}

#[derive(Default)]
struct MetricResponseCacheState {
    entries: BTreeMap<String, CachedMetricResponse>,
    next_prune_at: Option<Instant>,
}

#[derive(Debug, Clone)]
struct PinnedCacheEntry {
    key: String,
    approx_bytes: usize,
}

#[derive(Default)]
struct MemoryPinState {
    pinned: VecDeque<PinnedCacheEntry>,
    last_trigger_ms_by_scope: BTreeMap<String, u64>,
    scope_miss_counts: BTreeMap<String, u64>,
}

fn memory_pin_state() -> &'static Mutex<MemoryPinState> {
    static STATE: OnceLock<Mutex<MemoryPinState>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(MemoryPinState::default()))
}

fn approx_artifact_bytes(artifact: &crate::result_artifact::LoadedMetricResponseArtifact) -> usize {
    serde_json::to_string(&artifact.metrics_map)
        .map(|value| value.len())
        .unwrap_or(128)
}

pub fn warm_from_artifact(
    cache_keys: &[String],
    artifact: &crate::result_artifact::LoadedMetricResponseArtifact,
) {
    populate_l1_from_loaded_metric_artifact(cache_keys, artifact);
}

pub fn evict_metric_response_cache_key(key: &str) -> bool {
    let Ok(mut cache) = metric_response_cache().lock() else {
        return false;
    };
    cache.entries.remove(key).is_some()
}

pub fn enforce_memory_pin_limits(
    cache_key: &str,
    artifact: &crate::result_artifact::LoadedMetricResponseArtifact,
    max_pinned_slots: usize,
    max_pinned_mb: usize,
) {
    let Ok(mut pin_state) = memory_pin_state().lock() else {
        return;
    };
    let approx_bytes = approx_artifact_bytes(artifact);
    pin_state.pinned.retain(|entry| entry.key != cache_key);
    pin_state.pinned.push_back(PinnedCacheEntry {
        key: cache_key.to_string(),
        approx_bytes,
    });
    let max_bytes = max_pinned_mb.saturating_mul(1024 * 1024);
    loop {
        let slot_overflow = max_pinned_slots > 0 && pin_state.pinned.len() > max_pinned_slots;
        let total_bytes: usize = pin_state
            .pinned
            .iter()
            .map(|entry| entry.approx_bytes)
            .sum();
        let byte_overflow = max_bytes > 0 && total_bytes > max_bytes;
        if !slot_overflow && !byte_overflow {
            break;
        }
        let Some(oldest) = pin_state.pinned.pop_front() else {
            break;
        };
        let _ = evict_metric_response_cache_key(oldest.key.as_str());
    }
}

pub fn record_scope_cache_miss(scope_key: &str) {
    let Ok(mut pin_state) = memory_pin_state().lock() else {
        return;
    };
    *pin_state
        .scope_miss_counts
        .entry(scope_key.to_string())
        .or_insert(0) += 1;
}

pub fn should_trigger_smart_warmup(scope_key: &str, miss_threshold: u64, debounce_ms: u64) -> bool {
    let Ok(pin_state) = memory_pin_state().lock() else {
        return false;
    };
    let miss_count = pin_state
        .scope_miss_counts
        .get(scope_key)
        .copied()
        .unwrap_or(0);
    if miss_count < miss_threshold {
        return false;
    }
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0);
    let last_ms = pin_state
        .last_trigger_ms_by_scope
        .get(scope_key)
        .copied()
        .unwrap_or(0);
    now_ms.saturating_sub(last_ms) >= debounce_ms
}

pub fn mark_smart_warmup_triggered(scope_key: &str) {
    let Ok(mut pin_state) = memory_pin_state().lock() else {
        return;
    };
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0);
    pin_state
        .last_trigger_ms_by_scope
        .insert(scope_key.to_string(), now_ms);
    pin_state.scope_miss_counts.insert(scope_key.to_string(), 0);
}

impl MetricResponseCacheState {
    fn prune_if_due(&mut self, now: Instant) {
        if self.next_prune_at.is_some_and(|next| now < next) {
            return;
        }
        self.entries.retain(|_, entry| entry.expires_at > now);
        self.next_prune_at =
            Some(now + Duration::from_millis(METRIC_RESPONSE_CACHE_PRUNE_INTERVAL_MS));
    }
}

fn metric_response_cache() -> &'static Mutex<MetricResponseCacheState> {
    static CACHE: OnceLock<Mutex<MetricResponseCacheState>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(MetricResponseCacheState::default()))
}

pub fn metric_response_prebuild_query_tail(query: &DatasetQueryOptions) -> String {
    let group = serialize_cache_value(&query.group);
    let time_range = serialize_cache_value(&query.time_range);
    format!(
        "search={}|filters={}|group={group}|time_range={time_range}",
        query.search.as_deref().unwrap_or(""),
        serialize_cache_value(&query.filters),
    )
}

/// Prebuild 写入的 revision-agnostic 共享 key；访问态在 scoped key 因 compile revision
/// 漂移 miss 时可回退读取。
pub fn metric_response_prebuild_shared_key(
    app_id: &str,
    owner_dataset_id: &str,
    query: &DatasetQueryOptions,
    dependency_revision_key: &str,
) -> String {
    format!(
        "prebuild|response|app={app_id}|dataset={owner_dataset_id}|dependency={dependency_revision_key}|{}",
        metric_response_prebuild_query_tail(query)
    )
}

/// 不含 dependency revision 的 prebuild key；源数据指纹漂移时仍可命中已有产物。
pub fn metric_response_prebuild_dataset_key(
    app_id: &str,
    owner_dataset_id: &str,
    query: &DatasetQueryOptions,
) -> String {
    format!(
        "prebuild|response|app={app_id}|dataset={owner_dataset_id}|{}",
        metric_response_prebuild_query_tail(query)
    )
}

pub fn prebuild_metric_response_key_matches_dataset_query(
    response_cache_key: &str,
    app_id: &str,
    dataset_id: &str,
    query: &DatasetQueryOptions,
) -> bool {
    if !response_cache_key.starts_with("prebuild|response|") {
        return false;
    }
    let prefix = format!("prebuild|response|app={app_id}|dataset={dataset_id}|");
    if !response_cache_key.starts_with(prefix.as_str()) {
        return false;
    }
    let query_tail = metric_response_prebuild_query_tail(query);
    response_cache_key.ends_with(query_tail.as_str())
        || response_cache_key.contains(&format!("|{query_tail}"))
}

pub fn metric_eval_scope_key(scene_id: &str, scene_path: Option<&str>) -> String {
    let scene_id = scene_id.trim();
    if let Some(path) = scene_path.map(str::trim).filter(|value| !value.is_empty()) {
        if scene_id.is_empty() {
            path.to_string()
        } else {
            format!("{scene_id}@{path}")
        }
    } else if scene_id.is_empty() {
        "default".to_string()
    } else {
        scene_id.to_string()
    }
}

pub fn compute_metric_slot_revision(
    metric_def_bundle_revision: &str,
    data_source_revision: &str,
    scope_key: &str,
) -> String {
    let body = format!(
        "mdb={metric_def_bundle_revision}\nds={data_source_revision}\nscope={scope_key}\nengine=json_walk"
    );
    format!("sr:{}", crate::metric_cache_key::stable_slot_hash(&body))
}

pub fn metric_response_cache_scope_key(
    app_id: &str,
    scene_id: &str,
    scene_path: Option<&str>,
    dataset_id: &str,
    query: &DatasetQueryOptions,
    compile_revision: &str,
    dependency_revision_key: &str,
    filter_intents: &[FilterIntent],
    slot_revision: Option<&str>,
) -> String {
    let group = serialize_cache_value(&query.group);
    let time_range = serialize_cache_value(&query.time_range);
    format!(
        "{app_id}|compile={compile_revision}|{dependency_revision_key}|slot_rev={}|scene={scene_id}|target={}|dataset={dataset_id}|search={}|filters={}|group={}|time_range={}|filter_intents={}",
        slot_revision.unwrap_or(""),
        scene_path.unwrap_or(""),
        query.search.as_deref().unwrap_or(""),
        serialize_cache_value(&query.filters),
        group,
        time_range,
        serde_json::to_string(filter_intents).unwrap_or_else(|_| "[]".to_string())
    )
}

/// Partition-prefixed scope key for same-process multi-instance isolation.
pub fn metric_response_cache_key_partitioned(
    app_id: &str,
    generation: &str,
    config_digest: &str,
    inner_scope_key: &str,
) -> String {
    crate::cache_partition::partition_cache_key(app_id, generation, config_digest, inner_scope_key)
}

pub fn cached_metric_response_covers_request(
    entry: &CachedMetricResponse,
    requested_metric_ids: &BTreeSet<String>,
    request_all_metrics: bool,
) -> bool {
    if request_all_metrics {
        return entry.complete;
    }
    requested_metric_ids
        .iter()
        .all(|metric_id| entry.covered_metric_ids.contains(metric_id))
}

pub fn take_cached_metric_response(
    key: &str,
    requested_metric_ids: &BTreeSet<String>,
    request_all_metrics: bool,
) -> Option<CachedMetricResponse> {
    let Ok(mut cache) = metric_response_cache().lock() else {
        return None;
    };
    let now = Instant::now();
    cache.prune_if_due(now);
    let entry = cache.entries.get(key)?;
    cached_metric_response_covers_request(entry, requested_metric_ids, request_all_metrics)
        .then(|| entry.clone())
}

pub fn store_cached_metric_response_aliases(
    keys: &[String],
    total_rows: usize,
    metrics_map: &BTreeMap<String, MetricContract>,
    covered_metric_ids: &BTreeSet<String>,
    complete: bool,
) {
    for key in keys {
        let trimmed = key.trim();
        if trimmed.is_empty() {
            continue;
        }
        store_cached_metric_response(
            trimmed.to_string(),
            total_rows,
            metrics_map,
            covered_metric_ids,
            complete,
        );
    }
}

pub fn populate_l1_from_loaded_metric_artifact(
    lookup_keys: &[String],
    artifact: &crate::result_artifact::LoadedMetricResponseArtifact,
) {
    store_cached_metric_response_aliases(
        lookup_keys,
        artifact.total_rows,
        &artifact.metrics_map,
        &artifact.covered_metric_ids,
        artifact.complete,
    );
}

pub fn store_cached_metric_response(
    key: String,
    total_rows: usize,
    metrics_map: &BTreeMap<String, MetricContract>,
    covered_metric_ids: &BTreeSet<String>,
    complete: bool,
) {
    let Ok(mut cache) = metric_response_cache().lock() else {
        return;
    };
    let now = Instant::now();
    cache.prune_if_due(now);
    let expires_at = Instant::now() + metric_response_cache_ttl();
    if let Some(existing) = cache.entries.get_mut(&key) {
        existing.expires_at = expires_at;
        existing.total_rows = total_rows;
        existing.metrics_map.extend(metrics_map.clone());
        existing
            .covered_metric_ids
            .extend(covered_metric_ids.iter().cloned());
        existing.complete |= complete;
        return;
    }
    cache.entries.insert(
        key,
        CachedMetricResponse {
            expires_at,
            total_rows,
            metrics_map: metrics_map.clone(),
            covered_metric_ids: covered_metric_ids.clone(),
            complete,
        },
    );
}

pub fn clear_metric_response_cache() -> usize {
    let Ok(mut cache) = metric_response_cache().lock() else {
        return 0;
    };
    let removed = cache.entries.len();
    cache.entries.clear();
    cache.next_prune_at = None;
    removed
}

pub fn clear_metric_response_cache_for_partition(
    app_id: &str,
    generation: &str,
    config_digest: &str,
) -> usize {
    let Ok(mut cache) = metric_response_cache().lock() else {
        return 0;
    };
    let before = cache.entries.len();
    cache.entries.retain(|key, _| {
        !crate::cache_partition::partition_matches_key(app_id, generation, config_digest, key)
    });
    before.saturating_sub(cache.entries.len())
}

pub fn clear_all_metric_caches() -> (usize, usize) {
    (
        clear_metric_response_cache(),
        super::clear_metric_dataframe_result_cache() + super::clear_dataset_rows_cache(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::DatasetQueryOptions;
    use std::collections::{BTreeMap, BTreeSet};

    #[test]
    fn cached_metric_response_only_covers_all_metrics_when_complete() {
        let entry = CachedMetricResponse {
            total_rows: 0,
            metrics_map: BTreeMap::new(),
            covered_metric_ids: BTreeSet::from(["a".to_string(), "b".to_string()]),
            complete: false,
            expires_at: Instant::now(),
        };
        assert!(cached_metric_response_covers_request(
            &entry,
            &BTreeSet::from(["a".to_string()]),
            false
        ));
        assert!(!cached_metric_response_covers_request(
            &entry,
            &BTreeSet::new(),
            true
        ));
    }

    #[test]
    fn prebuild_metric_response_key_matches_dataset_query_ignores_dependency_revision() {
        let query = DatasetQueryOptions::default();
        let key = metric_response_prebuild_shared_key(
            "zhifa",
            "__world_metrics__::scenes/08-监督成效.mei::metrics",
            &query,
            "materialize=l3v3|deps=[\"csv-changed\"]",
        );
        assert!(prebuild_metric_response_key_matches_dataset_query(
            key.as_str(),
            "zhifa",
            "__world_metrics__::scenes/08-监督成效.mei::metrics",
            &query,
        ));
    }

    #[test]
    fn metric_response_cache_merges_partial_metric_coverage_by_scope() {
        clear_metric_response_cache();
        let key = "scope-key".to_string();
        store_cached_metric_response(
            key.clone(),
            12,
            &BTreeMap::new(),
            &BTreeSet::from(["metric.a".to_string()]),
            false,
        );
        store_cached_metric_response(
            key.clone(),
            12,
            &BTreeMap::new(),
            &BTreeSet::from(["metric.b".to_string()]),
            false,
        );
        let cached = take_cached_metric_response(
            &key,
            &BTreeSet::from(["metric.a".to_string(), "metric.b".to_string()]),
            false,
        )
        .expect("merged cache entry");
        assert_eq!(cached.total_rows, 12);
        assert!(cached.covered_metric_ids.contains("metric.a"));
        assert!(cached.covered_metric_ids.contains("metric.b"));
        clear_metric_response_cache();
    }

    #[test]
    fn memory_pin_evicts_oldest_entry_when_slot_limit_exceeded() {
        clear_metric_response_cache();
        let artifact = crate::result_artifact::LoadedMetricResponseArtifact {
            total_rows: 1,
            metrics_map: BTreeMap::new(),
            covered_metric_ids: BTreeSet::from(["metric.a".to_string()]),
            complete: true,
        };
        warm_from_artifact(&["pin-a".to_string()], &artifact);
        enforce_memory_pin_limits("pin-a", &artifact, 1, 128);
        warm_from_artifact(&["pin-b".to_string()], &artifact);
        enforce_memory_pin_limits("pin-b", &artifact, 1, 128);
        assert!(take_cached_metric_response(
            "pin-a",
            &BTreeSet::from(["metric.a".to_string()]),
            false
        )
        .is_none());
        assert!(take_cached_metric_response(
            "pin-b",
            &BTreeSet::from(["metric.a".to_string()]),
            false
        )
        .is_some());
        clear_metric_response_cache();
    }

    #[test]
    fn dual_partition_metric_response_entries_are_isolated() {
        clear_metric_response_cache();
        let key_a = metric_response_cache_key_partitioned(
            "mini-data",
            "WS-1",
            "cfg-scoped",
            "scope|metric.a",
        );
        let key_b = metric_response_cache_key_partitioned(
            "mini-data",
            "WS-1",
            "cfg-full",
            "scope|metric.a",
        );
        assert_ne!(key_a, key_b);
        store_cached_metric_response(
            key_a.clone(),
            1,
            &BTreeMap::new(),
            &BTreeSet::from(["metric.a".to_string()]),
            true,
        );
        assert!(take_cached_metric_response(
            &key_a,
            &BTreeSet::from(["metric.a".to_string()]),
            false
        )
        .is_some());
        assert!(take_cached_metric_response(
            &key_b,
            &BTreeSet::from(["metric.a".to_string()]),
            false
        )
        .is_none());
        assert_eq!(
            clear_metric_response_cache_for_partition("mini-data", "WS-1", "cfg-scoped"),
            1
        );
        assert!(take_cached_metric_response(
            &key_a,
            &BTreeSet::from(["metric.a".to_string()]),
            false
        )
        .is_none());
        clear_metric_response_cache();
    }
}

#[cfg(test)]
mod scope_tests {
    use super::super::metric_access::runtime_metric_scope_requested;
    use mei_lang_kernel::{
        FilterIntent, FilterIntentSource, FilterOperator, QueryState, QueryTimeRange,
    };
    use std::collections::BTreeMap;

    #[test]
    fn runtime_metric_scope_requested_is_false_for_context_free_request() {
        assert!(!runtime_metric_scope_requested(&QueryState::default(), &[]));
    }

    #[test]
    fn runtime_metric_scope_requested_is_true_for_query_state_context() {
        assert!(runtime_metric_scope_requested(
            &QueryState {
                filters: BTreeMap::from([("status".to_string(), "待办".to_string())]),
                search: None,
                group: vec!["park".to_string()],
                time_range: Some(QueryTimeRange {
                    dimension: Some("created_at".to_string()),
                    start: Some("2024-01-01".to_string()),
                    end: Some("2024-12-31".to_string()),
                    preset: None,
                }),
            },
            &[],
        ));
    }

    #[test]
    fn runtime_metric_scope_requested_is_true_for_filter_intents() {
        assert!(runtime_metric_scope_requested(
            &QueryState::default(),
            &[FilterIntent {
                dimension: "status".to_string(),
                operator: FilterOperator::Eq,
                value: "待办".to_string(),
                source: FilterIntentSource::FilterBar,
            }],
        ));
    }
}
