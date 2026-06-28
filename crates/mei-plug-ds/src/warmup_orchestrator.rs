use std::collections::BTreeMap;
use std::collections::BTreeSet;

use mei_host_core::{CacheLayersReady, EvalSlotDescriptor, HostContext};
use mei_host_graph::{
    record_slot_failed, record_slots_from_descriptors, write_client_bootstrap, MrgRegistryWriter,
    WarmupTier,
};
use mei_lang_kernel::{load_mei_config_for_app, MemoryWarmupConfig, MetricContract};

use crate::eval::{eval_metric_ids, load_compiled_for_warmup};
use crate::memory_warmup::{
    hydrate_slots_to_memory, mark_descriptors_client_ready, mark_descriptors_memory_ready,
};
use crate::warmup::WarmupTarget;

#[derive(Debug, Clone, Default)]
pub struct WarmupOrchestratorReport {
    pub slot_count: usize,
    pub memory_hydrated: usize,
    pub client_manifest_written: bool,
    pub failed_count: usize,
}

pub fn run_warmup_targets_with_tier(
    ctx: &HostContext,
    targets: &[WarmupTarget],
    tier: WarmupTier,
) -> anyhow::Result<WarmupOrchestratorReport> {
    let config = load_mei_config_for_app(ctx.app_root().as_path(), None);
    let memory_cfg = memory_warmup_config(&config.runtime.memory_warmup);
    let client_cfg = config.runtime.client_bootstrap.as_ref();

    let mut all_slots = Vec::new();
    let mut metrics_map: BTreeMap<String, MetricContract> = BTreeMap::new();
    let mut primary_scope = String::new();
    let mut primary_workset = String::new();
    let mut failed_count = 0usize;

    if tier.wants_disk() {
        for target in targets {
            let (compiled, compile_revision) =
                load_compiled_for_warmup(ctx, target.scope_key.as_str())?;
            match eval_metric_ids(
                ctx,
                &compiled,
                compile_revision.as_str(),
                target.scope_key.as_str(),
                target.owner_resource_id.as_str(),
                target.workset_id.as_str(),
                target.bundle_key.as_str(),
                &target.metric_ids,
            ) {
                Ok(outcome) => {
                    for metric in &outcome.metrics {
                        metrics_map.insert(metric.id.clone(), metric.clone());
                    }
                    all_slots.extend(outcome.descriptors);
                    if primary_scope.is_empty() {
                        primary_scope = target.scope_key.clone();
                        primary_workset = target.workset_id.clone();
                    }
                }
                Err(error) => {
                    failed_count += record_warmup_target_failure(ctx, target, error.to_string())?;
                }
            }
        }
    }

    let mut memory_hydrated = 0usize;
    if tier.wants_memory() && memory_cfg.enabled && !all_slots.is_empty() {
        memory_hydrated = hydrate_slots_to_memory(ctx, &all_slots, &memory_cfg)?;
        mark_descriptors_memory_ready(&mut all_slots);
        if memory_hydrated == 0 && tier == WarmupTier::Memory {
            for target in targets {
                let (compiled, compile_revision) =
                    load_compiled_for_warmup(ctx, target.scope_key.as_str())?;
                if let Ok(outcome) = eval_metric_ids(
                    ctx,
                    &compiled,
                    compile_revision.as_str(),
                    target.scope_key.as_str(),
                    target.owner_resource_id.as_str(),
                    target.workset_id.as_str(),
                    target.bundle_key.as_str(),
                    &target.metric_ids,
                ) {
                    for metric in &outcome.metrics {
                        metrics_map.insert(metric.id.clone(), metric.clone());
                    }
                    all_slots.extend(outcome.descriptors);
                }
            }
            memory_hydrated = hydrate_slots_to_memory(ctx, &all_slots, &memory_cfg)?;
            mark_descriptors_memory_ready(&mut all_slots);
        }
    }

    let max_client = client_cfg
        .map(|cfg| cfg.max_metrics_per_scope)
        .unwrap_or(32);
    let client_enabled = client_cfg.map(|cfg| cfg.enabled).unwrap_or(true);
    let client_scopes: BTreeSet<String> = client_cfg
        .map(|cfg| cfg.scopes.iter().cloned().collect())
        .unwrap_or_else(|| BTreeSet::from(["home".to_string()]));

    if tier.wants_client() && client_enabled {
        let metric_ids: BTreeSet<String> = metrics_map.keys().cloned().collect();
        mark_descriptors_client_ready(&mut all_slots, &metric_ids);
    }

    record_slots_from_descriptors(
        ctx.workspace_root.as_path(),
        ctx.app_id.as_str(),
        &all_slots,
    )?;

    let mut client_manifest_written = false;
    if tier.wants_client()
        && client_enabled
        && !primary_scope.is_empty()
        && (client_scopes.is_empty() || client_scopes.contains(primary_scope.as_str()))
    {
        if write_client_bootstrap(
            ctx.app_root().as_path(),
            ctx.app_id.as_str(),
            primary_scope.as_str(),
            primary_workset.as_str(),
            &all_slots,
            &metrics_map,
            max_client,
        )?
        .is_some()
        {
            client_manifest_written = true;
        }
    }

    Ok(WarmupOrchestratorReport {
        slot_count: all_slots.len(),
        memory_hydrated,
        client_manifest_written,
        failed_count,
    })
}

pub fn hydrate_existing_l1_slots(
    ctx: &HostContext,
    scope_key: &str,
) -> anyhow::Result<usize> {
    let config = load_mei_config_for_app(ctx.app_root().as_path(), None);
    let memory_cfg = memory_warmup_config(&config.runtime.memory_warmup);
    if !memory_cfg.enabled {
        return Ok(0);
    }
    let registry = MrgRegistryWriter::load(ctx.workspace_root.as_path(), ctx.app_id.as_str());
    let descriptors: Vec<_> = registry
        .slots
        .iter()
        .filter(|slot| slot.slot_id.scope_key == scope_key)
        .filter_map(|slot| {
            slot.payload_ref.as_ref().map(|pref| EvalSlotDescriptor {
                slot_key: slot.slot_id.node.key.clone(),
                scope_key: slot.slot_id.scope_key.clone(),
                owner_resource_id: slot.owner_resource_id.clone(),
                metric_def_bundle_revision: slot.metric_def_bundle_revision.clone(),
                data_source_revision: slot.data_source_revision.clone(),
                payload_kind: pref.kind.clone(),
                content_hash: pref.content_hash.clone(),
                schema_version: pref.schema_version.clone(),
                wall_ms: slot.last_eval.as_ref().map(|eval| eval.wall_ms).unwrap_or(0),
                artifact_hit: true,
                workset_id: slot.workset_id.clone().unwrap_or_default(),
                cache_layer: "disk".to_string(),
                cache_layers_ready: CacheLayersReady {
                    disk: true,
                    memory: false,
                    client: false,
                },
                client_revision: slot.client_revision.clone(),
                resident_tier: slot.resident_tier.clone(),
                client_eligible: slot.client_eligible,
                payload_bytes: slot.payload_bytes,
            })
        })
        .collect();
    hydrate_slots_to_memory(ctx, &descriptors, &memory_cfg)
}

fn memory_warmup_config(config: &Option<MemoryWarmupConfig>) -> MemoryWarmupConfig {
    config.clone().unwrap_or_default()
}

fn record_warmup_target_failure(
    ctx: &HostContext,
    target: &WarmupTarget,
    error_message: String,
) -> anyhow::Result<usize> {
    let bundle_revision = if target.bundle_key.is_empty() {
        target.owner_resource_id.as_str()
    } else {
        target.bundle_key.as_str()
    };
    let mut count = 0usize;
    for metric_id in &target.metric_ids {
        let slot_key = format!("{}::{}", target.workset_id, metric_id);
        record_slot_failed(
            ctx.workspace_root.as_path(),
            ctx.app_id.as_str(),
            slot_key.as_str(),
            target.scope_key.as_str(),
            target.owner_resource_id.as_str(),
            bundle_revision,
            "warmup",
            error_message.as_str(),
        )?;
        count += 1;
    }
    Ok(count)
}
