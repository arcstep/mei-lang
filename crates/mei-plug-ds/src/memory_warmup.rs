use std::collections::BTreeSet;

use mei_host_core::{EvalSlotDescriptor, HostContext};
use mei_lang_datasets::{
    enforce_memory_pin_limits, load_metric_response_result_artifact, warm_from_artifact,
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
    let mut hydrated = 0usize;
    for descriptor in descriptors {
        if descriptor.payload_kind != "metric_response" {
            continue;
        }
        let Some((artifact, _wall_ms)) = load_metric_response_result_artifact(
            app_root.as_path(),
            descriptor.content_hash.as_str(),
        )?
        else {
            continue;
        };
        warm_from_artifact(&[descriptor.content_hash.clone()], &artifact);
        enforce_memory_pin_limits(
            descriptor.content_hash.as_str(),
            &artifact,
            memory_cfg.max_pinned_slots,
            memory_cfg.max_pinned_mb,
        );
        hydrated += 1;
    }
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
    for descriptor in descriptors.iter_mut() {
        let metric_id = descriptor
            .slot_key
            .rsplit("::")
            .next()
            .unwrap_or(descriptor.slot_key.as_str());
        if metric_ids.contains(metric_id) {
            descriptor.cache_layers_ready.client = true;
            descriptor.client_eligible = true;
        }
    }
}
