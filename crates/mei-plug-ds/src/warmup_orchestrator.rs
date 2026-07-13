use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::time::Instant;

use mei_host_core::{
    dir_tree_bytes, CacheLayersReady, EvalSlotDescriptor, HostContext, ProcessPhaseTimer,
};
use mei_host_graph::{
    client_bootstrap_scope_allowed, collect_eval_frontier, linked_t2_page_pack_scopes,
    record_slot_failed, record_slots_from_descriptors, write_client_bootstrap, MrgRegistryWriter,
    WarmupTier,
};
use mei_lang_datasets::{snapshot_eval_cache_io, take_eval_cache_io_delta};
use mei_lang_kernel::{
    load_mei_config_for_app, resolve_app_eval_cache_root, resolve_app_var_root,
    ClientBootstrapConfig, MemoryWarmupConfig, MetricContract,
};

use crate::eval::{eval_metric_ids, load_compiled_for_warmup};
use crate::memory_warmup::{
    hydrate_slots_to_memory, mark_descriptors_client_ready, mark_descriptors_memory_ready,
};
use crate::warmup::{frontier_targets_from_metrics, WarmupTarget};

#[derive(Debug, Clone, Default)]
pub struct WarmupOrchestratorReport {
    pub slot_count: usize,
    pub memory_hydrated: usize,
    pub client_manifest_written: bool,
    pub client_manifest_scopes: Vec<String>,
    pub failed_count: usize,
    pub elapsed_ms: u64,
    pub disk_tier_ms: u64,
    pub memory_tier_ms: u64,
    pub client_tier_ms: u64,
    pub disk_bytes: u64,
    pub eval_compute_count: usize,
    pub eval_cache_hit_count: usize,
    pub disk_artifact_hit_count: usize,
    pub l1_cache_hit_count: usize,
    pub target_count: usize,
    pub unique_content_hash_count: usize,
    pub rss_before_bytes: Option<u64>,
    pub rss_after_bytes: Option<u64>,
    pub cpu_user_ms: Option<u64>,
    pub cpu_system_ms: Option<u64>,
    pub io_read_ops: u64,
    pub io_read_bytes: u64,
    pub io_write_ops: u64,
    pub io_write_bytes: u64,
    pub content_hash_dedupe_skips: u64,
    pub node_pack_loads: u64,
    pub node_pack_stores: u64,
    pub node_pack_store_skipped_full_hit: u64,
}

pub fn run_warmup_targets_with_tier(
    ctx: &HostContext,
    targets: &[WarmupTarget],
    tier: WarmupTier,
) -> anyhow::Result<WarmupOrchestratorReport> {
    let phase = ProcessPhaseTimer::start();
    let _ = snapshot_eval_cache_io();
    let started = Instant::now();
    let config = load_mei_config_for_app(ctx.app_root().as_path(), None);
    let memory_cfg = memory_warmup_config(&config.runtime.memory_warmup);
    let client_cfg = config.runtime.client_bootstrap.as_ref();
    let targets = expand_targets_for_client_neighbors(ctx, targets, client_cfg)?;

    let mut all_slots = Vec::new();
    let mut metrics_map: BTreeMap<String, MetricContract> = BTreeMap::new();
    let mut primary_scope = String::new();
    let mut primary_workset = String::new();
    let mut failed_count = 0usize;
    let mut disk_tier_ms = 0u64;
    let mut eval_compute_count = 0usize;
    let mut eval_cache_hit_count = 0usize;
    let mut disk_artifact_hit_count = 0usize;
    let mut l1_cache_hit_count = 0usize;
    let mut metric_total_rows: BTreeMap<String, usize> = BTreeMap::new();

    if tier.wants_disk() {
        let disk_started = Instant::now();
        for target in &targets {
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
                    if outcome.artifact_hit {
                        if outcome.cache_layer == "memory" {
                            l1_cache_hit_count += 1;
                        } else {
                            disk_artifact_hit_count += 1;
                        }
                        eval_cache_hit_count += 1;
                    } else {
                        eval_compute_count += 1;
                    }
                    for metric_id in &target.metric_ids {
                        metric_total_rows.insert(metric_id.clone(), outcome.total_rows);
                    }
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
        disk_tier_ms = disk_started.elapsed().as_millis() as u64;
    }

    let mut memory_hydrated = 0usize;
    let mut memory_tier_ms = 0u64;
    if tier.wants_memory() && memory_cfg.enabled && !all_slots.is_empty() {
        let memory_started = Instant::now();
        memory_hydrated = hydrate_slots_to_memory(ctx, &all_slots, &memory_cfg)?;
        mark_descriptors_memory_ready(&mut all_slots);
        if memory_hydrated == 0 && tier == WarmupTier::Memory {
            for target in &targets {
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
                    if outcome.artifact_hit {
                        if outcome.cache_layer == "memory" {
                            l1_cache_hit_count += 1;
                        } else {
                            disk_artifact_hit_count += 1;
                        }
                        eval_cache_hit_count += 1;
                    } else {
                        eval_compute_count += 1;
                    }
                    for metric_id in &target.metric_ids {
                        metric_total_rows.insert(metric_id.clone(), outcome.total_rows);
                    }
                    for metric in &outcome.metrics {
                        metrics_map.insert(metric.id.clone(), metric.clone());
                    }
                    all_slots.extend(outcome.descriptors);
                }
            }
            memory_hydrated = hydrate_slots_to_memory(ctx, &all_slots, &memory_cfg)?;
            mark_descriptors_memory_ready(&mut all_slots);
        }
        memory_tier_ms = memory_started.elapsed().as_millis() as u64;
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
    let mut client_manifest_scopes = Vec::new();
    let mut client_tier_ms = 0u64;
    let allowed_client_scopes = allowed_client_manifest_scopes(
        ctx,
        &targets,
        &all_slots,
        &client_scopes,
        client_cfg,
        primary_scope.as_str(),
    );
    if tier.wants_client() && client_enabled && !allowed_client_scopes.is_empty() {
        let client_started = Instant::now();
        let scope_worksets = preferred_workset_by_scope(&targets);
        let mut scope_revisions = BTreeMap::new();
        for scope in &allowed_client_scopes {
            let workset_id = scope_worksets
                .get(scope)
                .map(String::as_str)
                .unwrap_or(primary_workset.as_str());
            if let Some(manifest) = write_client_bootstrap(
                ctx.app_root().as_path(),
                ctx.app_id.as_str(),
                scope.as_str(),
                workset_id,
                &all_slots,
                &metrics_map,
                &metric_total_rows,
                max_client,
            )? {
                client_manifest_written = true;
                client_manifest_scopes.push(scope.clone());
                scope_revisions.insert(scope.clone(), manifest.client_revision.clone());
            }
        }
        if !scope_revisions.is_empty() {
            for slot in all_slots.iter_mut() {
                if slot.client_eligible && slot.cache_layers_ready.client {
                    if let Some(revision) = scope_revisions.get(slot.scope_key.as_str()) {
                        slot.client_revision = Some(revision.clone());
                    }
                }
            }
            record_slots_from_descriptors(
                ctx.workspace_root.as_path(),
                ctx.app_id.as_str(),
                &all_slots,
            )?;
        }
        client_tier_ms = client_started.elapsed().as_millis() as u64;
        if client_manifest_written {
            for scope in &client_manifest_scopes {
                let status = mei_host_graph::bootstrap_embed_status(
                    ctx.workspace_root.as_path(),
                    ctx.app_id.as_str(),
                    scope.as_str(),
                );
                tracing::info!(
                    app_id = %ctx.app_id,
                    scope = %scope,
                    allowed = status.allowed,
                    reason = %status.reason,
                    metric_count = status.metric_count,
                    "warmup client-bootstrap revision gate"
                );
            }
        }
    }

    let disk_bytes = warmup_disk_bytes(ctx);
    let process = phase.finish();
    let io = take_eval_cache_io_delta();
    let unique_content_hash_count = all_slots
        .iter()
        .map(|slot| slot.content_hash.as_str())
        .collect::<BTreeSet<_>>()
        .len();

    Ok(WarmupOrchestratorReport {
        slot_count: all_slots.len(),
        memory_hydrated,
        client_manifest_written,
        client_manifest_scopes,
        failed_count,
        elapsed_ms: started.elapsed().as_millis() as u64,
        disk_tier_ms,
        memory_tier_ms,
        client_tier_ms,
        disk_bytes,
        eval_compute_count,
        eval_cache_hit_count,
        disk_artifact_hit_count,
        l1_cache_hit_count,
        target_count: targets.len(),
        unique_content_hash_count,
        rss_before_bytes: process.rss_before_bytes,
        rss_after_bytes: process.rss_bytes,
        cpu_user_ms: process.cpu_user_ms,
        cpu_system_ms: process.cpu_system_ms,
        io_read_ops: io.read_ops,
        io_read_bytes: io.read_bytes,
        io_write_ops: io.write_ops,
        io_write_bytes: io.write_bytes,
        content_hash_dedupe_skips: io.content_hash_dedupe_skips,
        node_pack_loads: io.node_pack_loads,
        node_pack_stores: io.node_pack_stores,
        node_pack_store_skipped_full_hit: io.node_pack_store_skipped_full_hit,
    })
}

fn expand_targets_for_client_neighbors(
    ctx: &HostContext,
    targets: &[WarmupTarget],
    client_cfg: Option<&ClientBootstrapConfig>,
) -> anyhow::Result<Vec<WarmupTarget>> {
    let Some(cfg) = client_cfg else {
        return Ok(targets.to_vec());
    };
    if cfg.neighbor_hops == 0 || targets.is_empty() {
        return Ok(targets.to_vec());
    }
    let root_scope = targets[0].scope_key.as_str();
    let pack_scopes =
        linked_t2_page_pack_scopes(ctx, root_scope, cfg.neighbor_hops, cfg.max_neighbor_scopes)?;
    let mut expanded = targets.to_vec();
    let mut known: BTreeSet<String> = expanded.iter().map(|t| t.scope_key.clone()).collect();
    for scope in pack_scopes {
        if !known.insert(scope.clone()) {
            continue;
        }
        let metrics = collect_eval_frontier(ctx, scope.as_str())?;
        if metrics.is_empty() {
            continue;
        }
        expanded.extend(frontier_targets_from_metrics(root_scope, &metrics));
    }
    Ok(expanded)
}

fn preferred_workset_by_scope(targets: &[WarmupTarget]) -> BTreeMap<String, String> {
    let mut worksets = BTreeMap::new();
    for target in targets {
        worksets
            .entry(target.scope_key.clone())
            .or_insert_with(|| target.workset_id.clone());
    }
    worksets
}

fn allowed_client_manifest_scopes(
    ctx: &HostContext,
    targets: &[WarmupTarget],
    slots: &[EvalSlotDescriptor],
    client_scopes: &BTreeSet<String>,
    client_cfg: Option<&ClientBootstrapConfig>,
    primary_scope: &str,
) -> BTreeSet<String> {
    if targets.is_empty() && slots.is_empty() {
        return BTreeSet::new();
    }
    let configured: Vec<String> = client_scopes.iter().cloned().collect();
    let pack_scopes = client_cfg
        .filter(|cfg| cfg.neighbor_hops > 0)
        .and_then(|cfg| {
            linked_t2_page_pack_scopes(
                ctx,
                primary_scope,
                cfg.neighbor_hops,
                cfg.max_neighbor_scopes,
            )
            .ok()
        })
        .unwrap_or_default();
    let pack_scope_list = pack_scopes.clone();
    let mut candidates = BTreeSet::new();
    for target in targets {
        candidates.insert(target.scope_key.clone());
    }
    for scope in pack_scopes {
        candidates.insert(scope);
    }
    if let Some(cfg) = client_cfg {
        for scope in &cfg.scopes {
            candidates.insert(scope.clone());
        }
    }
    if !primary_scope.is_empty() {
        candidates.insert(primary_scope.to_string());
    }
    candidates
        .into_iter()
        .filter(|scope| {
            client_bootstrap_scope_allowed(scope, configured.as_slice(), pack_scope_list.as_slice())
                && slots.iter().any(|slot| slot.scope_key == *scope)
        })
        .collect()
}

pub fn hydrate_existing_l1_slots(ctx: &HostContext, scope_key: &str) -> anyhow::Result<usize> {
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
                wall_ms: slot
                    .last_eval
                    .as_ref()
                    .map(|eval| eval.wall_ms)
                    .unwrap_or(0),
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

fn warmup_disk_bytes(ctx: &HostContext) -> u64 {
    let app_root = ctx.app_root();
    let eval_cache = resolve_app_eval_cache_root(app_root.as_path());
    let client_bootstrap = resolve_app_var_root(app_root.as_path()).join("client-bootstrap");
    dir_tree_bytes(eval_cache.as_path()) + dir_tree_bytes(client_bootstrap.as_path())
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
