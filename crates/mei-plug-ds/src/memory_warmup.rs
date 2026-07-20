use std::collections::BTreeMap;
use std::collections::BTreeSet;

use mei_host_core::{EvalSlotDescriptor, HostContext};
use mei_lang_datasets::{
    configure_l1_pin_policy, enforce_memory_pin_limits, load_metric_response_lite_artifact,
    metric_id_is_scalar_rowset, record_content_hash_dedupe_skips, warm_from_artifact, L1PinPolicy,
    L1ProjectStats,
};
use mei_lang_kernel::MemoryWarmupConfig;

pub fn apply_memory_warmup_pin_policy(memory_cfg: &MemoryWarmupConfig) {
    configure_l1_pin_policy(L1PinPolicy {
        pin_rowsets: memory_cfg.pin_rowsets,
        max_pinned_value_bytes: memory_cfg.max_pinned_value_bytes,
    });
}

pub fn hydrate_slots_to_memory(
    ctx: &HostContext,
    descriptors: &[EvalSlotDescriptor],
    memory_cfg: &MemoryWarmupConfig,
) -> anyhow::Result<(usize, L1ProjectStats)> {
    if !memory_cfg.enabled {
        return Ok((0, L1ProjectStats::default()));
    }
    apply_memory_warmup_pin_policy(memory_cfg);
    let app_root = ctx.app_root();
    let mut by_hash: BTreeMap<String, Vec<&EvalSlotDescriptor>> = BTreeMap::new();
    for descriptor in descriptors {
        if descriptor.payload_kind != "metric_response" {
            continue;
        }
        by_hash
            .entry(descriptor.content_hash.clone())
            .or_default()
            .push(descriptor);
    }
    let mut hydrated = 0usize;
    let mut dedupe_skips = 0u64;
    let mut aggregate_stats = L1ProjectStats::default();
    for (content_hash, group) in by_hash {
        // Memory hydrate must not load full metric-response JSON.
        let Some((artifact, _wall_ms, stats)) =
            load_metric_response_lite_artifact(app_root.as_path(), content_hash.as_str())?
        else {
            continue;
        };
        if group.len() > 1 {
            dedupe_skips += (group.len() - 1) as u64;
        }
        let mut keys: BTreeSet<String> = BTreeSet::new();
        keys.insert(content_hash.clone());
        for descriptor in &group {
            if !descriptor.content_hash.trim().is_empty() {
                keys.insert(descriptor.content_hash.clone());
            }
        }
        let key_list: Vec<String> = keys.into_iter().collect();
        let _ = warm_from_artifact(&key_list, &artifact);
        aggregate_stats.kept_metrics += stats.kept_metrics;
        aggregate_stats.skipped_rowsets += stats.skipped_rowsets;
        aggregate_stats.skipped_oversized += stats.skipped_oversized;
        aggregate_stats.projected_bytes = aggregate_stats
            .projected_bytes
            .saturating_add(stats.projected_bytes);
        enforce_memory_pin_limits(
            &key_list,
            stats.projected_bytes.max(1),
            memory_cfg.max_pinned_slots,
            memory_cfg.max_pinned_mb,
        );
        for descriptor in &group {
            let metric_id = descriptor
                .slot_key
                .rsplit("::")
                .next()
                .unwrap_or(descriptor.slot_key.as_str());
            if memory_cfg.pin_rowsets || !metric_id_is_scalar_rowset(metric_id) {
                hydrated += 1;
            }
        }
    }
    record_content_hash_dedupe_skips(dedupe_skips);
    Ok((hydrated, aggregate_stats))
}

pub fn mark_descriptors_memory_ready(descriptors: &mut [EvalSlotDescriptor], pin_rowsets: bool) {
    for descriptor in descriptors.iter_mut() {
        let metric_id = descriptor
            .slot_key
            .rsplit("::")
            .next()
            .unwrap_or(descriptor.slot_key.as_str());
        let is_rowset = metric_id_is_scalar_rowset(metric_id);
        if is_rowset && !pin_rowsets {
            descriptor.cache_layers_ready.memory = false;
            if descriptor.cache_layers_ready.disk {
                descriptor.cache_layer = "disk".to_string();
                descriptor.resident_tier = "disk_resident".to_string();
            }
            continue;
        }
        descriptor.cache_layers_ready.memory = true;
        descriptor.cache_layer = "memory".to_string();
        descriptor.resident_tier = "memory_resident".to_string();
    }
}

pub fn mark_descriptors_client_ready(
    descriptors: &mut [EvalSlotDescriptor],
    metric_ids: &BTreeSet<String>,
) {
    // Do not auto-expand __scalar_rowset__ into client eligibility (bootstrap stays lean).
    for descriptor in descriptors.iter_mut() {
        let metric_id = descriptor
            .slot_key
            .rsplit("::")
            .next()
            .unwrap_or(descriptor.slot_key.as_str());
        if metric_ids.contains(metric_id) && !metric_id_is_scalar_rowset(metric_id) {
            descriptor.cache_layers_ready.client = true;
            descriptor.client_eligible = true;
        }
    }
}
