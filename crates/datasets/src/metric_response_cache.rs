use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use mei_lang_kernel::MetricContract;

use super::serialize_cache_value;
use super::types::DatasetQueryOptions;

const METRIC_RESPONSE_CACHE_TTL_MS: u64 = 300_000;
const METRIC_RESPONSE_CACHE_PRUNE_INTERVAL_MS: u64 = 5_000;

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

fn metric_response_cache_ttl() -> Duration {
    Duration::from_millis(METRIC_RESPONSE_CACHE_TTL_MS)
}

pub fn metric_response_cache_scope_key(
    app_id: &str,
    scene_id: &str,
    scene_path: Option<&str>,
    dataset_id: &str,
    query: &DatasetQueryOptions,
    compile_revision: &str,
    dependency_revision_key: &str,
) -> String {
    let group = serialize_cache_value(&query.group);
    let time_range = serialize_cache_value(&query.time_range);
    format!(
        "{app_id}|compile={compile_revision}|{dependency_revision_key}|scene={scene_id}|target={}|dataset={dataset_id}|search={}|filters={}|group={}|time_range={}",
        scene_path.unwrap_or(""),
        query.search.as_deref().unwrap_or(""),
        serialize_cache_value(&query.filters),
        group,
        time_range
    )
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

pub fn clear_all_metric_caches() -> (usize, usize) {
    (
        clear_metric_response_cache(),
        super::clear_metric_dataframe_result_cache() + super::clear_dataset_rows_cache(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
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
