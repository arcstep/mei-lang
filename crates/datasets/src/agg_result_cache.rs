//! Cross-request aggregation result cache keyed by filter fingerprint.
//!
//! Entries are L1-projected (no `__scalar_rowset__` / oversized values). Disk may
//! retain full packs; this cache must not keep per-metric rowset working sets.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use mei_lang_kernel::{FilterIntent, MetricContract, QueryState};

use crate::l1_project::{project_metrics_map_for_l1, L1PinPolicy};

const AGG_RESULT_CACHE_TTL_MS: u64 = 120_000;
const AGG_RESULT_CACHE_PRUNE_INTERVAL_MS: u64 = 5_000;
const MAX_AGG_RESULT_CACHE_ENTRIES: usize = 256;

#[derive(Clone)]
struct CachedAggResult {
    expires_at: Instant,
    metrics_map: BTreeMap<String, MetricContract>,
    total_rows: usize,
}

#[derive(Default)]
struct AggResultCacheState {
    entries: BTreeMap<String, CachedAggResult>,
    lru: VecDeque<String>,
    next_prune_at: Option<Instant>,
}

fn agg_result_cache() -> &'static Mutex<AggResultCacheState> {
    static CACHE: OnceLock<Mutex<AggResultCacheState>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(AggResultCacheState::default()))
}

fn cache_ttl() -> Duration {
    Duration::from_millis(AGG_RESULT_CACHE_TTL_MS)
}

pub fn filters_fingerprint(query_state: &QueryState, filter_intents: &[FilterIntent]) -> String {
    let mut parts = Vec::new();
    if let Some(search) = query_state.search.as_deref().map(str::trim) {
        if !search.is_empty() {
            parts.push(format!("search={search}"));
        }
    }
    for (key, value) in &query_state.filters {
        parts.push(format!("filter:{key}={value}"));
    }
    for key in &query_state.group {
        parts.push(format!("group:{key}"));
    }
    if let Some(time_range) = query_state.time_range.as_ref() {
        parts.push(format!("time={time_range:?}"));
    }
    for intent in filter_intents {
        parts.push(format!("intent:{intent:?}"));
    }
    parts.join("|")
}

pub fn agg_result_cache_key(
    app_id: &str,
    dataset_id: &str,
    metric_ids: &[String],
    query_state: &QueryState,
    filter_intents: &[FilterIntent],
    dependency_revision_key: &str,
) -> String {
    let mut metric_part = metric_ids.join(",");
    if metric_part.is_empty() {
        metric_part = "*".to_string();
    }
    format!(
        "{app_id}|{dataset_id}|metrics={metric_part}|filters={}|dep={dependency_revision_key}",
        filters_fingerprint(query_state, filter_intents)
    )
}

pub fn lookup_agg_result_cache(key: &str) -> Option<(BTreeMap<String, MetricContract>, usize)> {
    let mut guard = agg_result_cache().lock().ok()?;
    maybe_prune_agg_result_cache(&mut guard);
    let Some(cached) = guard.entries.get(key).cloned() else {
        return None;
    };
    if cached.expires_at <= Instant::now() {
        guard.entries.remove(key);
        guard.lru.retain(|value| value != key);
        return None;
    }
    if let Some(pos) = guard.lru.iter().position(|value| value == key) {
        guard.lru.remove(pos);
    }
    guard.lru.push_back(key.to_string());
    Some((cached.metrics_map, cached.total_rows))
}

pub fn store_agg_result_cache(
    key: String,
    metrics_map: BTreeMap<String, MetricContract>,
    total_rows: usize,
) {
    let covered: BTreeSet<String> = metrics_map.keys().cloned().collect();
    let (projected, _, _) =
        project_metrics_map_for_l1(&metrics_map, &covered, &L1PinPolicy::default());
    drop(metrics_map);
    let Ok(mut guard) = agg_result_cache().lock() else {
        return;
    };
    maybe_prune_agg_result_cache(&mut guard);
    guard.entries.insert(
        key.clone(),
        CachedAggResult {
            expires_at: Instant::now() + cache_ttl(),
            metrics_map: projected,
            total_rows,
        },
    );
    guard.lru.retain(|value| value != &key);
    guard.lru.push_back(key);
    while guard.entries.len() > MAX_AGG_RESULT_CACHE_ENTRIES {
        if let Some(oldest) = guard.lru.pop_front() {
            guard.entries.remove(&oldest);
        } else {
            break;
        }
    }
}

pub fn clear_agg_result_cache() -> usize {
    let Ok(mut guard) = agg_result_cache().lock() else {
        return 0;
    };
    let cleared = guard.entries.len();
    guard.entries.clear();
    guard.lru.clear();
    guard.next_prune_at = None;
    cleared
}

fn maybe_prune_agg_result_cache(state: &mut AggResultCacheState) {
    let now = Instant::now();
    if state
        .next_prune_at
        .is_some_and(|next| now < next && state.entries.len() <= MAX_AGG_RESULT_CACHE_ENTRIES)
    {
        return;
    }
    state.entries.retain(|key, entry| {
        if entry.expires_at <= now {
            state.lru.retain(|value| value != key);
            false
        } else {
            true
        }
    });
    state.next_prune_at = Some(now + Duration::from_millis(AGG_RESULT_CACHE_PRUNE_INTERVAL_MS));
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn contract(id: &str, value: serde_json::Value) -> MetricContract {
        MetricContract {
            id: id.into(),
            label: None,
            unit: None,
            value_format: None,
            purpose: None,
            shape: mei_lang_kernel::MetricShape::Scalar,
            schema: Vec::new(),
            dataset: None,
            transforms: Vec::new(),
            value,
        }
    }

    #[test]
    fn store_agg_result_cache_projects_out_rowsets() {
        clear_agg_result_cache();
        let mut map = BTreeMap::new();
        map.insert("kpi_count".into(), contract("kpi_count", json!(12)));
        map.insert(
            "ds::__scalar_rowset__".into(),
            contract("ds::__scalar_rowset__", json!([{"a": 1}, {"a": 2}])),
        );
        store_agg_result_cache("k1".into(), map, 2);
        let (cached, total) = lookup_agg_result_cache("k1").expect("cached");
        assert_eq!(total, 2);
        assert!(cached.contains_key("kpi_count"));
        assert!(!cached.keys().any(|id| id.contains("__scalar_rowset__")));
        clear_agg_result_cache();
    }
}
