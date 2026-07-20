//! Shared projection rules for Memory L1 / metric-response-lite artifacts.
//! Disk may keep full packs; Memory and bootstrap consume projected maps only.

use std::collections::{BTreeMap, BTreeSet};

use mei_lang_kernel::MetricContract;

const DEFAULT_MAX_PINNED_VALUE_BYTES: usize = 256 * 1024;
const SCALAR_ROWSET_SUFFIX: &str = "__scalar_rowset__";

/// Policy for what may reside in the pinned L1 / lite artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct L1PinPolicy {
    pub pin_rowsets: bool,
    pub max_pinned_value_bytes: usize,
}

impl Default for L1PinPolicy {
    fn default() -> Self {
        Self {
            pin_rowsets: false,
            max_pinned_value_bytes: DEFAULT_MAX_PINNED_VALUE_BYTES,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct L1ProjectStats {
    pub kept_metrics: usize,
    pub skipped_rowsets: usize,
    pub skipped_oversized: usize,
    pub projected_bytes: usize,
}

pub fn metric_id_is_scalar_rowset(metric_id: &str) -> bool {
    let trimmed = metric_id.trim();
    trimmed == SCALAR_ROWSET_SUFFIX || trimmed.ends_with(&format!("::{SCALAR_ROWSET_SUFFIX}"))
}

fn approx_metric_contract_bytes(contract: &MetricContract) -> usize {
    serde_json::to_string(contract)
        .map(|value| value.len())
        .unwrap_or(64)
}

/// Project a metrics map for pinned L1 / lite disk: drop rowsets (unless allowed) and oversized values.
pub fn project_metrics_map_for_l1(
    metrics_map: &BTreeMap<String, MetricContract>,
    covered_metric_ids: &BTreeSet<String>,
    policy: &L1PinPolicy,
) -> (
    BTreeMap<String, MetricContract>,
    BTreeSet<String>,
    L1ProjectStats,
) {
    let mut projected = BTreeMap::new();
    let mut stats = L1ProjectStats::default();
    for (metric_id, contract) in metrics_map {
        if !policy.pin_rowsets && metric_id_is_scalar_rowset(metric_id) {
            stats.skipped_rowsets += 1;
            continue;
        }
        let bytes = approx_metric_contract_bytes(contract);
        if policy.max_pinned_value_bytes > 0 && bytes > policy.max_pinned_value_bytes {
            stats.skipped_oversized += 1;
            continue;
        }
        projected.insert(metric_id.clone(), contract.clone());
        stats.kept_metrics += 1;
    }
    let mut covered = BTreeSet::new();
    for metric_id in covered_metric_ids {
        if let Some(contract) = metrics_map.get(metric_id) {
            if projected.contains_key(metric_id) {
                covered.insert(metric_id.clone());
            } else {
                let _ = contract;
            }
            continue;
        }
        if policy.pin_rowsets || !metric_id_is_scalar_rowset(metric_id) {
            covered.insert(metric_id.clone());
        }
    }
    stats.projected_bytes = serde_json::to_string(&projected)
        .map(|value| value.len())
        .unwrap_or(0);
    (projected, covered, stats)
}
