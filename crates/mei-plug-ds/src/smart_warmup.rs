use mei_host_core::HostContext;
use mei_host_graph::{
    collect_eval_frontier_with_hops, flush_telemetry_to_registry,
    record_navigation_edges_for_scope, warm_frontier_slots, MrgRegistryWriter, WarmupTier,
};
use mei_lang_datasets::{mark_smart_warmup_triggered, should_trigger_smart_warmup};
use mei_lang_kernel::load_mei_config_for_app;

use crate::warmup::frontier_targets_from_metrics;
use crate::warmup_orchestrator::run_warmup_targets_with_tier;

const SMART_WARMUP_MISS_THRESHOLD: u64 = 3;
const SMART_WARMUP_DEBOUNCE_MS: u64 = 60_000;

pub fn maybe_trigger_smart_warmup(ctx: &HostContext, scope_key: &str) {
    let config = load_mei_config_for_app(ctx.app_root().as_path(), None);
    let enabled = config
        .runtime
        .smart_warmup
        .as_ref()
        .map(|cfg| cfg.enabled)
        .unwrap_or(false);
    if !enabled {
        return;
    }
    if !should_trigger_smart_warmup(
        scope_key,
        SMART_WARMUP_MISS_THRESHOLD,
        SMART_WARMUP_DEBOUNCE_MS,
    ) {
        return;
    }
    if run_smart_warmup(ctx, scope_key).is_ok() {
        mark_smart_warmup_triggered(scope_key);
    }
}

pub fn run_smart_warmup(ctx: &HostContext, scope_key: &str) -> anyhow::Result<()> {
    run_frontier_warmup(ctx, scope_key, 1, "smart warmup completed")
}

pub fn run_activation_warmup(
    ctx: &HostContext,
    scope_key: &str,
    hops: usize,
) -> anyhow::Result<()> {
    let clamped_hops = hops.max(1);
    run_frontier_warmup(
        ctx,
        scope_key,
        clamped_hops,
        "scope activation warmup completed",
    )
}

fn run_frontier_warmup(
    ctx: &HostContext,
    scope_key: &str,
    hops: usize,
    log_message: &str,
) -> anyhow::Result<()> {
    let metrics = collect_eval_frontier_with_hops(ctx, scope_key, hops)?;
    let targets = frontier_targets_from_metrics(scope_key, &metrics);
    if targets.is_empty() {
        return Ok(());
    }
    let report = run_warmup_targets_with_tier(ctx, &targets, WarmupTier::All)?;
    let edges = record_navigation_edges_for_scope(ctx, scope_key, hops)?;
    let registry = MrgRegistryWriter::load(ctx.workspace_root.as_path(), ctx.app_id.as_str());
    let frontier = warm_frontier_slots(&registry, scope_key, hops);
    tracing::info!(
        app_id = %ctx.app_id,
        scope = %scope_key,
        slots = report.slot_count,
        memory_hydrated = report.memory_hydrated,
        client_manifest_scopes = ?report.client_manifest_scopes,
        navigation_edges = edges,
        scheduled_frontier = frontier.scheduled_slots.len(),
        "{log_message}"
    );
    flush_telemetry_to_registry(ctx.workspace_root.as_path(), ctx.app_id.as_str())?;
    Ok(())
}
