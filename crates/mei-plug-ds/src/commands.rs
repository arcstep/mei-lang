use mei_host_core::{format_bytes_human, log_timestamp_rfc3339, HostContext};
use mei_host_graph::WarmupTier;
use mei_lang_datasets::configure_metric_response_cache_ttl_ms;
use mei_lang_kernel::load_mei_config_for_app;

use crate::cli::WarmupArgs;
use crate::{collect_warmup_targets, frontier_targets_from_metrics, run_warmup_targets_with_tier};

pub async fn run_warmup(args: WarmupArgs) -> anyhow::Result<()> {
    let workspace = args
        .workspace
        .canonicalize()
        .unwrap_or_else(|_| args.workspace.clone());
    let app = args.app.clone();
    let ctx = HostContext::new(workspace, app);
    let tier = WarmupTier::parse(args.tier.as_str());
    let app_config = load_mei_config_for_app(ctx.app_root().as_path(), None);
    configure_metric_response_cache_ttl_ms(app_config.runtime.server_eval_cache.ttl_ms);
    let targets = resolve_warmup_targets(&ctx, &args)?;
    println!(
        "[{}] warmup start: app={} policy={} tier={} worksets={}",
        log_timestamp_rfc3339(),
        ctx.app_id,
        args.policy,
        args.tier,
        targets.len()
    );
    let report = run_warmup_targets_with_tier(&ctx, &targets, tier)?;
    if args.hops > 0 {
        let scope = args
            .frontier
            .as_deref()
            .or(Some(args.policy.as_str()))
            .unwrap_or("home");
        let edges = mei_host_graph::record_navigation_edges_for_scope(&ctx, scope, args.hops)?;
        if edges > 0 {
            println!(
                "[{}] warmup navigation edges added: {edges}",
                log_timestamp_rfc3339()
            );
        }
    }
    println!(
        "[{}] warmup ok: policy={} tier={} worksets={} slots={} memory_hydrated={} client_manifest={} failed={} elapsed_ms={} disk_tier_ms={} memory_tier_ms={} client_tier_ms={} disk_bytes={} ({}) eval_compute={} cache_hit={} disk_hit={} l1_hit={}",
        log_timestamp_rfc3339(),
        args.policy,
        args.tier,
        targets.len(),
        report.slot_count,
        report.memory_hydrated,
        report.client_manifest_written,
        report.failed_count,
        report.elapsed_ms,
        report.disk_tier_ms,
        report.memory_tier_ms,
        report.client_tier_ms,
        report.disk_bytes,
        format_bytes_human(report.disk_bytes),
        report.eval_compute_count,
        report.eval_cache_hit_count,
        report.disk_artifact_hit_count,
        report.l1_cache_hit_count
    );
    Ok(())
}

pub fn resolve_warmup_targets(
    ctx: &HostContext,
    args: &WarmupArgs,
) -> anyhow::Result<Vec<crate::WarmupTarget>> {
    if let Some(board) = args
        .board
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let metrics = mei_host_graph::collect_eval_frontier(ctx, board)?;
        return Ok(frontier_targets_from_metrics(board, &metrics));
    }
    if let Some(frontier) = args
        .frontier
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let metrics = mei_host_graph::collect_eval_frontier_with_hops(ctx, frontier, args.hops)?;
        return Ok(frontier_targets_from_metrics(frontier, &metrics));
    }
    collect_warmup_targets(ctx, Some(args.policy.as_str()))
}

pub async fn run_serve(args: crate::cli::ServeArgs) -> anyhow::Result<()> {
    let workspace = args
        .workspace
        .canonicalize()
        .unwrap_or_else(|_| args.workspace.clone());
    let app = args.app.clone();
    let ctx = HostContext::new(workspace, app);
    let addr = format!("{}:{}", args.host, args.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!(
        "mei-plug-ds listening on http://{addr} (app={})",
        ctx.app_id
    );
    let app = crate::http::router(ctx);
    axum::serve(listener, app)
        .await
        .map_err(|e| anyhow::anyhow!(e))
}
