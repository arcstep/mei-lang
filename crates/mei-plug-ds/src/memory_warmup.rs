use std::collections::BTreeMap;
use std::collections::BTreeSet;

use mei_host_core::{EvalSlotDescriptor, HostContext};
use mei_lang_datasets::{
    enforce_memory_pin_limits, load_metric_response_result_artifact,
    record_content_hash_dedupe_skips, warm_from_artifact,
};
use mei_lang_kernel::MemoryWarmupConfig;

pub fn hydrate_slots_to_memory(
    ctx: &HostContext,
    descriptors: &[EvalSlotDescriptor],
    memory_cfg: &MemoryWarmupConfig,
) -> anyhow::Result<usize> {
    if !memory_cfg.enabled {
        return Ok(0);
    }
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
    for (content_hash, group) in by_hash {
        let Some((artifact, _wall_ms)) =
            load_metric_response_result_artifact(app_root.as_path(), content_hash.as_str())?
        else {
            continue;
        };
        if group.len() > 1 {
            dedupe_skips += (group.len() - 1) as u64;
        }
        // content_hash 在 MRG 中存的是 metric response cache_key（scoped），
        // 同时再登记一份，保证 hydrate 后 batch lookup 能命中同一键。
        let mut keys: BTreeSet<String> = BTreeSet::new();
        keys.insert(content_hash.clone());
        for descriptor in &group {
            if !descriptor.content_hash.trim().is_empty() {
                keys.insert(descriptor.content_hash.clone());
            }
        }
        let key_list: Vec<String> = keys.into_iter().collect();
        warm_from_artifact(&key_list, &artifact);
        enforce_memory_pin_limits(
            content_hash.as_str(),
            &artifact,
            memory_cfg.max_pinned_slots,
            memory_cfg.max_pinned_mb,
        );
        hydrated += group.len();
    }
    record_content_hash_dedupe_skips(dedupe_skips);
    Ok(hydrated)
}

pub fn mark_descriptors_memory_ready(descriptors: &mut [EvalSlotDescriptor]) {
    for descriptor in descriptors.iter_mut() {
        descriptor.cache_layers_ready.memory = true;
        descriptor.cache_layer = "memory".to_string();
        descriptor.resident_tier = "memory_resident".to_string();
    }
}

pub fn mark_descriptors_client_ready(
    descriptors: &mut [EvalSlotDescriptor],
    metric_ids: &BTreeSet<String>,
) {
    let mut expanded = metric_ids.clone();
    for metric_id in metric_ids.iter() {
        if !metric_id.contains("::__scalar_rowset__") {
            expanded.insert(format!("{metric_id}::__scalar_rowset__"));
        }
    }
    for descriptor in descriptors.iter_mut() {
        let metric_id = descriptor
            .slot_key
            .rsplit("::")
            .next()
            .unwrap_or(descriptor.slot_key.as_str());
        if expanded.contains(metric_id) {
            descriptor.cache_layers_ready.client = true;
            descriptor.client_eligible = true;
        }
    }
}
