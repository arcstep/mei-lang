//! Persist last warmup summary for runtime snapshot / audits.

use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use mei_lang_kernel::resolve_app_var_root;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const WARMUP_LAST_RUN_REL: &str = "warmup-last-run.json";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WarmupLastRunRecord {
    pub policy: String,
    pub at_ms: u64,
    pub eval_compute: usize,
    pub cache_hit: usize,
    pub disk_hit: usize,
    pub l1_hit: usize,
    pub slot_count: usize,
    pub elapsed_ms: u64,
    #[serde(default)]
    pub disk_tier_ms: u64,
    #[serde(default)]
    pub memory_tier_ms: u64,
    #[serde(default)]
    pub client_tier_ms: u64,
    #[serde(default)]
    pub disk_bytes: u64,
    #[serde(default)]
    pub target_count: usize,
    #[serde(default)]
    pub unique_content_hash_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rss_before_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rss_after_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu_user_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu_system_ms: Option<u64>,
    #[serde(default)]
    pub io_read_ops: u64,
    #[serde(default)]
    pub io_read_bytes: u64,
    #[serde(default)]
    pub io_write_ops: u64,
    #[serde(default)]
    pub io_write_bytes: u64,
    #[serde(default)]
    pub content_hash_dedupe_skips: u64,
    #[serde(default)]
    pub node_pack_loads: u64,
    #[serde(default)]
    pub node_pack_stores: u64,
    #[serde(default)]
    pub node_pack_store_skipped_full_hit: u64,
    #[serde(default)]
    pub tier: String,
    #[serde(default)]
    pub memory_hydrated: usize,
    #[serde(default)]
    pub memory_pinned_bytes: u64,
    #[serde(default)]
    pub rowset_skipped: u64,
    #[serde(default)]
    pub oversized_skipped: u64,
    #[serde(default)]
    pub projected_metric_count: u64,
    #[serde(default)]
    pub lite_hydrated: u64,
    #[serde(default)]
    pub lite_bytes: u64,
    #[serde(default)]
    pub full_artifact_loads: u64,
    #[serde(default)]
    pub lite_backfill: u64,
}

pub fn write_warmup_last_run(app_root: &Path, record: &WarmupLastRunRecord) -> anyhow::Result<()> {
    let var_root = resolve_app_var_root(app_root);
    fs::create_dir_all(&var_root)?;
    let path = var_root.join(WARMUP_LAST_RUN_REL);
    let payload = serde_json::to_string_pretty(record)?;
    fs::write(path, payload)?;
    Ok(())
}

pub fn read_warmup_last_run(app_root: &Path) -> Option<WarmupLastRunRecord> {
    let path = resolve_app_var_root(app_root).join(WARMUP_LAST_RUN_REL);
    let raw = fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

pub fn warmup_last_run_json(app_root: &Path) -> Value {
    read_warmup_last_run(app_root)
        .map(|record| {
            json!({
                "policy": record.policy,
                "atMs": record.at_ms,
                "evalCompute": record.eval_compute,
                "cacheHit": record.cache_hit,
                "diskHit": record.disk_hit,
                "l1Hit": record.l1_hit,
                "slotCount": record.slot_count,
                "elapsedMs": record.elapsed_ms,
                "diskTierMs": record.disk_tier_ms,
                "memoryTierMs": record.memory_tier_ms,
                "clientTierMs": record.client_tier_ms,
                "diskBytes": record.disk_bytes,
                "targetCount": record.target_count,
                "uniqueContentHashCount": record.unique_content_hash_count,
                "rssBeforeBytes": record.rss_before_bytes,
                "rssAfterBytes": record.rss_after_bytes,
                "cpuUserMs": record.cpu_user_ms,
                "cpuSystemMs": record.cpu_system_ms,
                "ioReadOps": record.io_read_ops,
                "ioReadBytes": record.io_read_bytes,
                "ioWriteOps": record.io_write_ops,
                "ioWriteBytes": record.io_write_bytes,
                "contentHashDedupeSkips": record.content_hash_dedupe_skips,
                "nodePackLoads": record.node_pack_loads,
                "nodePackStores": record.node_pack_stores,
                "nodePackStoreSkippedFullHit": record.node_pack_store_skipped_full_hit,
                "tier": record.tier,
                "memoryHydrated": record.memory_hydrated,
                "memoryPinnedBytes": record.memory_pinned_bytes,
                "rowsetSkipped": record.rowset_skipped,
                "oversizedSkipped": record.oversized_skipped,
                "projectedMetricCount": record.projected_metric_count,
                "liteHydrated": record.lite_hydrated,
                "liteBytes": record.lite_bytes,
                "fullArtifactLoads": record.full_artifact_loads,
                "liteBackfill": record.lite_backfill,
            })
        })
        .unwrap_or_else(|| json!(null))
}

pub fn current_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}
