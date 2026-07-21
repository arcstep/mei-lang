use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use mei_lang_kernel::{FilterIntent, MetricContract};
use moka::sync::Cache;

use super::serialize_cache_value;
use super::types::DatasetQueryOptions;

pub use super::l1_project::{
    metric_contract_eligible_for_node_pack, metric_id_eligible_for_node_pack,
    metric_id_is_scalar_rowset, project_metrics_map_for_l1, L1PinPolicy, L1ProjectStats,
};

const METRIC_RESPONSE_CACHE_TTL_MS: u64 = 300_000;
const DEMAND_CACHE_TTL_MS: u64 = 30_000;
static MOKA_EVICTIONS: AtomicU64 = AtomicU64::new(0);
static ROWSET_ADMISSION_REJECTS: AtomicU64 = AtomicU64::new(0);
static OVERSIZE_ADMISSION_REJECTS: AtomicU64 = AtomicU64::new(0);

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

fn l1_pin_policy_state() -> &'static Mutex<L1PinPolicy> {
    static STATE: OnceLock<Mutex<L1PinPolicy>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(L1PinPolicy::default()))
}

pub fn configure_l1_pin_policy(policy: L1PinPolicy) {
    if let Ok(mut guard) = l1_pin_policy_state().lock() {
        *guard = policy;
    }
}

pub fn current_l1_pin_policy() -> L1PinPolicy {
    l1_pin_policy_state()
        .lock()
        .map(|guard| guard.clone())
        .unwrap_or_default()
}

pub fn request_needs_bulk_l1_metrics(
    requested_metric_ids: &BTreeSet<String>,
    request_all_metrics: bool,
) -> bool {
    if request_all_metrics {
        return true;
    }
    let policy = current_l1_pin_policy();
    !policy.pin_rowsets
        && requested_metric_ids
            .iter()
            .any(|metric_id| metric_id_is_scalar_rowset(metric_id))
}

#[derive(Debug, Clone)]
pub struct CachedMetricResponse {
    pub total_rows: usize,
    pub metrics_map: Arc<BTreeMap<String, MetricContract>>,
    pub covered_metric_ids: BTreeSet<String>,
    pub complete: bool,
    expires_at: Instant,
}

#[derive(Debug, Clone, Copy, Default, serde::Serialize)]
pub struct MokaL1Stats {
    pub metric_entries: u64,
    pub metric_weighted_bytes: u64,
    pub demand_entries: u64,
    pub demand_weighted_bytes: u64,
    pub evictions: u64,
    pub rowset_admission_rejects: u64,
    pub oversize_admission_rejects: u64,
}

pub fn snapshot_moka_l1_stats() -> MokaL1Stats {
    let metric = metric_response_cache();
    let demand = demand_cache();
    MokaL1Stats {
        metric_entries: metric.entry_count(),
        metric_weighted_bytes: metric.weighted_size(),
        demand_entries: demand.entry_count(),
        demand_weighted_bytes: demand.weighted_size(),
        evictions: MOKA_EVICTIONS.load(Ordering::Relaxed),
        rowset_admission_rejects: ROWSET_ADMISSION_REJECTS.load(Ordering::Relaxed),
        oversize_admission_rejects: OVERSIZE_ADMISSION_REJECTS.load(Ordering::Relaxed),
    }
}

#[derive(Debug, Clone)]
struct PinnedCacheEntry {
    keys: Vec<String>,
    approx_bytes: usize,
}

#[derive(Default)]
struct MemoryPinState {
    pinned: VecDeque<PinnedCacheEntry>,
    last_trigger_ms_by_scope: BTreeMap<String, u64>,
    scope_miss_counts: BTreeMap<String, u64>,
    last_project_stats: L1ProjectStats,
    pinned_bytes: usize,
}

fn memory_pin_state() -> &'static Mutex<MemoryPinState> {
    static STATE: OnceLock<Mutex<MemoryPinState>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(MemoryPinState::default()))
}

fn configured_l1_capacity_bytes() -> u64 {
    std::env::var("MEI_MOKA_L1_MAX_BYTES")
        .ok()
        .and_then(|raw| raw.trim().parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(64 * 1024 * 1024)
}

fn cache_entry_weight(_key: &String, entry: &Arc<CachedMetricResponse>) -> u32 {
    approx_metrics_map_bytes(entry.metrics_map.as_ref())
        .saturating_add(
            entry
                .covered_metric_ids
                .iter()
                .map(String::len)
                .sum::<usize>(),
        )
        .clamp(1, u32::MAX as usize) as u32
}

fn demand_cache() -> &'static Cache<String, Arc<CachedMetricResponse>> {
    static CACHE: OnceLock<Cache<String, Arc<CachedMetricResponse>>> = OnceLock::new();
    CACHE.get_or_init(|| {
        Cache::builder()
            .max_capacity((configured_l1_capacity_bytes() / 4).max(1))
            .weigher(cache_entry_weight)
            .time_to_live(Duration::from_millis(DEMAND_CACHE_TTL_MS))
            .eviction_listener(|_, _, cause| {
                if matches!(
                    cause,
                    moka::notification::RemovalCause::Size
                        | moka::notification::RemovalCause::Expired
                ) {
                    MOKA_EVICTIONS.fetch_add(1, Ordering::Relaxed);
                }
            })
            .build()
    })
}

fn approx_metrics_map_bytes(metrics_map: &BTreeMap<String, MetricContract>) -> usize {
    serde_json::to_string(metrics_map)
        .map(|value| value.len())
        .unwrap_or(128)
}

pub fn last_l1_project_stats() -> L1ProjectStats {
    memory_pin_state()
        .lock()
        .map(|guard| guard.last_project_stats.clone())
        .unwrap_or_default()
}

pub fn memory_pinned_bytes() -> usize {
    memory_pin_state()
        .lock()
        .map(|guard| guard.pinned_bytes)
        .unwrap_or(0)
}

pub fn warm_from_artifact(
    cache_keys: &[String],
    artifact: &crate::result_artifact::LoadedMetricResponseArtifact,
) -> L1ProjectStats {
    populate_l1_from_loaded_metric_artifact(cache_keys, artifact)
}

pub fn evict_metric_response_cache_key(key: &str) -> bool {
    let cache = metric_response_cache();
    let existed = cache.contains_key(key);
    cache.invalidate(key);
    existed
}

fn evict_metric_response_cache_keys(keys: &[String]) {
    let cache = metric_response_cache();
    for key in keys {
        cache.invalidate(key);
    }
}

/// Pin projected L1 bytes for a group of alias keys; evict oldest groups on overflow.
pub fn enforce_memory_pin_limits(
    cache_keys: &[String],
    approx_bytes: usize,
    max_pinned_slots: usize,
    max_pinned_mb: usize,
) {
    let keys: Vec<String> = cache_keys
        .iter()
        .map(|key| key.trim().to_string())
        .filter(|key| !key.is_empty())
        .collect();
    if keys.is_empty() {
        return;
    }
    let mut evict_batches: Vec<Vec<String>> = Vec::new();
    {
        let Ok(mut pin_state) = memory_pin_state().lock() else {
            return;
        };
        pin_state.pinned.retain(|entry| {
            !entry
                .keys
                .iter()
                .any(|key| keys.iter().any(|incoming| incoming == key))
        });
        pin_state
            .pinned
            .push_back(PinnedCacheEntry { keys, approx_bytes });
        let max_bytes = max_pinned_mb.saturating_mul(1024 * 1024);
        loop {
            let slot_overflow = max_pinned_slots > 0 && pin_state.pinned.len() > max_pinned_slots;
            let total_bytes: usize = pin_state
                .pinned
                .iter()
                .map(|entry| entry.approx_bytes)
                .sum();
            pin_state.pinned_bytes = total_bytes;
            let byte_overflow = max_bytes > 0 && total_bytes > max_bytes;
            if !slot_overflow && !byte_overflow {
                break;
            }
            let Some(oldest) = pin_state.pinned.pop_front() else {
                break;
            };
            evict_batches.push(oldest.keys);
        }
        pin_state.pinned_bytes = pin_state
            .pinned
            .iter()
            .map(|entry| entry.approx_bytes)
            .sum();
    }
    for batch in evict_batches {
        evict_metric_response_cache_keys(&batch);
    }
}

/// Compatibility wrapper: pin a single key using projected artifact size.
pub fn enforce_memory_pin_limits_for_artifact(
    cache_key: &str,
    artifact: &crate::result_artifact::LoadedMetricResponseArtifact,
    max_pinned_slots: usize,
    max_pinned_mb: usize,
) {
    let policy = current_l1_pin_policy();
    let (projected, _, stats) =
        project_metrics_map_for_l1(&artifact.metrics_map, &artifact.covered_metric_ids, &policy);
    let approx_bytes = if stats.projected_bytes > 0 {
        stats.projected_bytes
    } else {
        approx_metrics_map_bytes(&projected)
    };
    enforce_memory_pin_limits(
        &[cache_key.to_string()],
        approx_bytes,
        max_pinned_slots,
        max_pinned_mb,
    );
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

fn metric_response_cache() -> &'static Cache<String, Arc<CachedMetricResponse>> {
    static CACHE: OnceLock<Cache<String, Arc<CachedMetricResponse>>> = OnceLock::new();
    CACHE.get_or_init(|| {
        Cache::builder()
            .max_capacity(configured_l1_capacity_bytes())
            .weigher(cache_entry_weight)
            .time_to_live(metric_response_cache_ttl())
            .eviction_listener(|_, _, cause| {
                if matches!(
                    cause,
                    moka::notification::RemovalCause::Size
                        | moka::notification::RemovalCause::Expired
                ) {
                    MOKA_EVICTIONS.fetch_add(1, Ordering::Relaxed);
                }
            })
            .build()
    })
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
    let entry = metric_response_cache().get(key)?;
    cached_metric_response_covers_request(entry.as_ref(), requested_metric_ids, request_all_metrics)
        .then(|| entry.as_ref().clone())
}

pub fn store_cached_metric_response_aliases(
    keys: &[String],
    total_rows: usize,
    metrics_map: &BTreeMap<String, MetricContract>,
    covered_metric_ids: &BTreeSet<String>,
    complete: bool,
) -> L1ProjectStats {
    let policy = current_l1_pin_policy();
    let (projected_map, projected_covered, stats) =
        project_metrics_map_for_l1(metrics_map, covered_metric_ids, &policy);
    ROWSET_ADMISSION_REJECTS.fetch_add(stats.skipped_rowsets as u64, Ordering::Relaxed);
    OVERSIZE_ADMISSION_REJECTS.fetch_add(stats.skipped_oversized as u64, Ordering::Relaxed);
    if let Ok(mut pin_state) = memory_pin_state().lock() {
        pin_state.last_project_stats = stats.clone();
    }
    let projected_complete = complete && stats.skipped_rowsets == 0 && stats.skipped_oversized == 0;
    let shared_metrics = Arc::new(projected_map);
    let shared_covered = projected_covered;
    for key in keys {
        let trimmed = key.trim();
        if trimmed.is_empty() {
            continue;
        }
        store_cached_metric_response_shared(
            trimmed.to_string(),
            total_rows,
            Arc::clone(&shared_metrics),
            shared_covered.clone(),
            projected_complete,
        );
    }
    stats
}

pub fn populate_l1_from_loaded_metric_artifact(
    lookup_keys: &[String],
    artifact: &crate::result_artifact::LoadedMetricResponseArtifact,
) -> L1ProjectStats {
    store_cached_metric_response_aliases(
        lookup_keys,
        artifact.total_rows,
        &artifact.metrics_map,
        &artifact.covered_metric_ids,
        artifact.complete,
    )
}

pub fn store_cached_metric_response(
    key: String,
    total_rows: usize,
    metrics_map: &BTreeMap<String, MetricContract>,
    covered_metric_ids: &BTreeSet<String>,
    complete: bool,
) -> L1ProjectStats {
    store_cached_metric_response_aliases(
        &[key],
        total_rows,
        metrics_map,
        covered_metric_ids,
        complete,
    )
}

fn store_cached_metric_response_shared(
    key: String,
    total_rows: usize,
    metrics_map: Arc<BTreeMap<String, MetricContract>>,
    covered_metric_ids: BTreeSet<String>,
    complete: bool,
) {
    let expires_at = Instant::now() + metric_response_cache_ttl();
    let policy = current_l1_pin_policy();
    let cache = metric_response_cache();
    if let Some(existing) = cache.get(&key) {
        let mut updated = existing.as_ref().clone();
        updated.expires_at = expires_at;
        updated.total_rows = total_rows;
        if Arc::ptr_eq(&existing.metrics_map, &metrics_map) {
            // Same Arc payload — extend coverage only with projected ids.
            updated
                .covered_metric_ids
                .extend(covered_metric_ids.iter().cloned());
            updated.complete |= complete;
            cache.insert(key, Arc::new(updated));
            return;
        }
        let mut merged = (*updated.metrics_map).clone();
        merged.extend((*metrics_map).clone());
        let mut merged_covered = updated.covered_metric_ids.clone();
        merged_covered.extend(covered_metric_ids.iter().cloned());
        let (projected, projected_covered, _) =
            project_metrics_map_for_l1(&merged, &merged_covered, &policy);
        updated.metrics_map = Arc::new(projected);
        updated.covered_metric_ids = projected_covered;
        updated.complete |= complete;
        cache.insert(key, Arc::new(updated));
        return;
    }
    cache.insert(
        key,
        Arc::new(CachedMetricResponse {
            expires_at,
            total_rows,
            metrics_map,
            covered_metric_ids,
            complete,
        }),
    );
}

/// Store a short-TTL projected response for on-demand reads.
///
/// Whole-table rowsets are request working sets, never cross-request L1 values.
pub fn store_demand_metric_response(
    keys: &[String],
    total_rows: usize,
    metrics_map: &BTreeMap<String, MetricContract>,
    covered_metric_ids: &BTreeSet<String>,
    complete: bool,
) {
    let policy = current_l1_pin_policy();
    let (projected, projected_covered, stats) =
        project_metrics_map_for_l1(metrics_map, covered_metric_ids, &policy);
    ROWSET_ADMISSION_REJECTS.fetch_add(stats.skipped_rowsets as u64, Ordering::Relaxed);
    OVERSIZE_ADMISSION_REJECTS.fetch_add(stats.skipped_oversized as u64, Ordering::Relaxed);
    if projected.is_empty() {
        return;
    }
    let expires_at = Instant::now() + Duration::from_millis(DEMAND_CACHE_TTL_MS);
    let shared = Arc::new(projected);
    let projected_complete = complete && stats.skipped_rowsets == 0 && stats.skipped_oversized == 0;
    let cache = demand_cache();
    for key in keys {
        let trimmed = key.trim();
        if trimmed.is_empty() {
            continue;
        }
        cache.insert(
            trimmed.to_string(),
            Arc::new(CachedMetricResponse {
                total_rows,
                metrics_map: Arc::clone(&shared),
                covered_metric_ids: projected_covered.clone(),
                complete: projected_complete,
                expires_at,
            }),
        );
    }
}

pub fn take_demand_metric_response(
    key: &str,
    requested_metric_ids: &BTreeSet<String>,
    request_all_metrics: bool,
) -> Option<CachedMetricResponse> {
    let entry = demand_cache().get(key)?;
    cached_metric_response_covers_request(entry.as_ref(), requested_metric_ids, request_all_metrics)
        .then(|| entry.as_ref().clone())
}

pub fn clear_demand_metric_response_cache() -> usize {
    let cache = demand_cache();
    let removed = cache.entry_count() as usize;
    cache.invalidate_all();
    cache.run_pending_tasks();
    removed
}

pub fn clear_metric_response_cache() -> usize {
    let cache = metric_response_cache();
    let removed = cache.entry_count() as usize;
    cache.invalidate_all();
    cache.run_pending_tasks();
    removed
}

pub fn clear_metric_response_cache_for_partition(
    app_id: &str,
    generation: &str,
    config_digest: &str,
) -> usize {
    let cache = metric_response_cache();
    let keys: Vec<String> = cache
        .iter()
        .filter_map(|(key, _)| {
            crate::cache_partition::partition_matches_key(
                app_id,
                generation,
                config_digest,
                key.as_str(),
            )
            .then(|| key.as_ref().clone())
        })
        .collect();
    for key in &keys {
        cache.invalidate(key);
    }
    cache.run_pending_tasks();
    keys.len()
}

pub fn clear_all_metric_caches() -> (usize, usize) {
    let demand = clear_demand_metric_response_cache();
    let _ = super::clear_agg_result_cache();
    let _ = super::clear_table_handle_cache();
    let _ = super::clear_query_engine_sessions();
    (
        clear_metric_response_cache() + demand,
        super::clear_metric_dataframe_result_cache() + super::clear_dataset_rows_cache(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::DatasetQueryOptions;
    use std::collections::{BTreeMap, BTreeSet};
    use std::sync::{Mutex, OnceLock};

    fn cache_test_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn cached_metric_response_only_covers_all_metrics_when_complete() {
        let entry = CachedMetricResponse {
            total_rows: 0,
            metrics_map: Arc::new(BTreeMap::new()),
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
        let _guard = cache_test_lock().lock().expect("cache test lock");
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
        let _guard = cache_test_lock().lock().expect("cache test lock");
        clear_metric_response_cache();
        use mei_lang_kernel::MetricShape;
        let mut metrics_map = BTreeMap::new();
        metrics_map.insert(
            "metric.a".to_string(),
            MetricContract {
                id: "metric.a".to_string(),
                label: None,
                unit: None,
                purpose: None,
                shape: MetricShape::Scalar,
                schema: Vec::new(),
                value: serde_json::json!({"value": 1}),
                value_format: None,
                dataset: None,
                transforms: Vec::new(),
            },
        );
        let artifact = crate::result_artifact::LoadedMetricResponseArtifact {
            total_rows: 1,
            metrics_map,
            covered_metric_ids: BTreeSet::from(["metric.a".to_string()]),
            complete: true,
        };
        let stats_a = warm_from_artifact(&["pin-a".to_string()], &artifact);
        enforce_memory_pin_limits(
            &["pin-a".to_string()],
            stats_a.projected_bytes.max(1),
            1,
            128,
        );
        let stats_b = warm_from_artifact(&["pin-b".to_string()], &artifact);
        enforce_memory_pin_limits(
            &["pin-b".to_string()],
            stats_b.projected_bytes.max(1),
            1,
            128,
        );
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
    fn l1_projection_skips_scalar_rowset_by_default() {
        let _guard = cache_test_lock().lock().expect("cache test lock");
        use mei_lang_kernel::MetricShape;
        clear_metric_response_cache();
        configure_l1_pin_policy(L1PinPolicy::default());
        let mut metrics_map = BTreeMap::new();
        metrics_map.insert(
            "kpi_count".to_string(),
            MetricContract {
                id: "kpi_count".to_string(),
                label: None,
                unit: None,
                purpose: None,
                shape: MetricShape::Scalar,
                schema: Vec::new(),
                value: serde_json::json!({"value": 1}),
                value_format: None,
                dataset: None,
                transforms: Vec::new(),
            },
        );
        metrics_map.insert(
            "kpi_count::__scalar_rowset__".to_string(),
            MetricContract {
                id: "kpi_count::__scalar_rowset__".to_string(),
                label: None,
                unit: None,
                purpose: None,
                shape: MetricShape::Dataframe,
                schema: Vec::new(),
                value: serde_json::json!({"rows": [1, 2, 3, 4, 5]}),
                value_format: None,
                dataset: None,
                transforms: Vec::new(),
            },
        );
        let covered = BTreeSet::from([
            "kpi_count".to_string(),
            "kpi_count::__scalar_rowset__".to_string(),
        ]);
        let stats =
            store_cached_metric_response("proj-key".to_string(), 5, &metrics_map, &covered, true);
        assert_eq!(stats.skipped_rowsets, 1);
        assert_eq!(stats.kept_metrics, 1);
        let cached = take_cached_metric_response(
            "proj-key",
            &BTreeSet::from(["kpi_count".to_string()]),
            false,
        )
        .expect("kpi pinned");
        assert!(cached.metrics_map.contains_key("kpi_count"));
        assert!(!cached
            .metrics_map
            .contains_key("kpi_count::__scalar_rowset__"));
        assert!(take_cached_metric_response(
            "proj-key",
            &BTreeSet::from(["kpi_count::__scalar_rowset__".to_string()]),
            false
        )
        .is_none());
        clear_metric_response_cache();
    }

    #[test]
    fn dual_partition_metric_response_entries_are_isolated() {
        let _guard = cache_test_lock().lock().expect("cache test lock");
        clear_metric_response_cache();
        use mei_lang_kernel::MetricShape;
        // Unique partition names avoid races with other tests sharing the process-global cache.
        let nonce = format!("{}-{}", std::process::id(), now_epoch_ms_for_test());
        let app = format!("mini-data-{nonce}");
        let ws = format!("WS-{nonce}");
        let key_a = metric_response_cache_key_partitioned(
            app.as_str(),
            ws.as_str(),
            "cfg-scoped",
            "scope|metric.a",
        );
        let key_b = metric_response_cache_key_partitioned(
            app.as_str(),
            ws.as_str(),
            "cfg-full",
            "scope|metric.a",
        );
        assert_ne!(key_a, key_b);
        let mut metrics_map = BTreeMap::new();
        metrics_map.insert(
            "metric.a".to_string(),
            MetricContract {
                id: "metric.a".to_string(),
                label: None,
                unit: None,
                purpose: None,
                shape: MetricShape::Scalar,
                schema: Vec::new(),
                value: serde_json::json!({"value": 1}),
                value_format: None,
                dataset: None,
                transforms: Vec::new(),
            },
        );
        store_cached_metric_response(
            key_a.clone(),
            1,
            &metrics_map,
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
        let _removed =
            clear_metric_response_cache_for_partition(app.as_str(), ws.as_str(), "cfg-scoped");
        assert!(take_cached_metric_response(
            &key_a,
            &BTreeSet::from(["metric.a".to_string()]),
            false
        )
        .is_none());
        clear_metric_response_cache();
    }

    fn now_epoch_ms_for_test() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
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
