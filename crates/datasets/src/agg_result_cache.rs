//! Cross-request aggregation result cache keyed by filter fingerprint.
//!
//! Entries are L1-projected (no `__scalar_rowset__` / oversized values). Disk may
//! retain full packs; this cache must not keep per-metric rowset working sets.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use mei_lang_kernel::{FilterIntent, MetricContract, QueryState};
use moka::sync::Cache;

use crate::l1_project::{project_metrics_map_for_l1, L1PinPolicy};

const AGG_RESULT_CACHE_TTL_MS: u64 = 120_000;
const AGG_RESULT_CACHE_MAX_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Clone)]
struct CachedAggResult {
    metrics_map: BTreeMap<String, MetricContract>,
    total_rows: usize,
}

fn agg_result_cache() -> &'static Cache<String, Arc<CachedAggResult>> {
    static CACHE: OnceLock<Cache<String, Arc<CachedAggResult>>> = OnceLock::new();
    CACHE.get_or_init(|| {
        Cache::builder()
            .max_capacity(AGG_RESULT_CACHE_MAX_BYTES)
            .weigher(|_key: &String, value: &Arc<CachedAggResult>| {
                serde_json::to_vec(&value.metrics_map)
                    .map(|bytes| bytes.len().clamp(1, u32::MAX as usize) as u32)
                    .unwrap_or(128)
            })
            .time_to_live(Duration::from_millis(AGG_RESULT_CACHE_TTL_MS))
            .build()
    })
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
    let cached = agg_result_cache().get(key)?;
    Some((cached.metrics_map.clone(), cached.total_rows))
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
    agg_result_cache().insert(
        key.clone(),
        Arc::new(CachedAggResult {
            metrics_map: projected,
            total_rows,
        }),
    );
}

pub fn clear_agg_result_cache() -> usize {
    let cache = agg_result_cache();
    let cleared = cache.entry_count() as usize;
    cache.invalidate_all();
    cache.run_pending_tasks();
    cleared
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
