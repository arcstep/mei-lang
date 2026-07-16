use mei_host_core::{format_bytes_human, log_timestamp_rfc3339, HostContext};
use mei_host_graph::WarmupTier;
use mei_lang_datasets::configure_metric_response_cache_ttl_ms;
use mei_lang_kernel::load_mei_config_for_app;

use crate::cli::WarmupArgs;
use crate::{
    collect_warmup_targets_for_scopes_with_filter, frontier_targets_from_metrics,
    run_warmup_targets_with_tier, WarmupScopeFilter,
};

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
    let dev_eval = WarmupScopeFilter::from_env();
    let warmup_scopes_label = if dev_eval.warmup_scopes.is_empty() {
        "-".to_string()
    } else {
        dev_eval.warmup_scopes.join(",")
    };
    println!(
        "[{}] warmup config: profile={} warmupScopes={}",
        log_timestamp_rfc3339(),
        if dev_eval.profile.is_empty() {
            "full"
        } else {
            dev_eval.profile.as_str()
        },
        warmup_scopes_label,
    );
    let targets = resolve_warmup_targets_with_filter(&ctx, &args, &dev_eval)?;
    println!(
        "[{}] warmup start: app={} policy={} tier={} worksets={}",
        log_timestamp_rfc3339(),
        ctx.app_id,
        args.policy,
        args.tier,
        targets.len()
    );
    let report = run_warmup_targets_with_tier(&ctx, &targets, tier)?;
    let _ = mei_host_graph::write_warmup_last_run(
        ctx.app_root().as_path(),
        &mei_host_graph::WarmupLastRunRecord {
            policy: args.policy.clone(),
            at_ms: mei_host_graph::warmup_last_run_time_ms(),
            eval_compute: report.eval_compute_count,
            cache_hit: report.eval_cache_hit_count,
            disk_hit: report.disk_artifact_hit_count,
            l1_hit: report.l1_cache_hit_count,
            slot_count: report.slot_count,
            elapsed_ms: report.elapsed_ms,
            disk_tier_ms: report.disk_tier_ms,
            memory_tier_ms: report.memory_tier_ms,
            client_tier_ms: report.client_tier_ms,
            disk_bytes: report.disk_bytes,
            target_count: report.target_count,
            unique_content_hash_count: report.unique_content_hash_count,
            rss_before_bytes: report.rss_before_bytes,
            rss_after_bytes: report.rss_after_bytes,
            cpu_user_ms: report.cpu_user_ms,
            cpu_system_ms: report.cpu_system_ms,
            io_read_ops: report.io_read_ops,
            io_read_bytes: report.io_read_bytes,
            io_write_ops: report.io_write_ops,
            io_write_bytes: report.io_write_bytes,
            content_hash_dedupe_skips: report.content_hash_dedupe_skips,
            node_pack_loads: report.node_pack_loads,
            node_pack_stores: report.node_pack_stores,
            node_pack_store_skipped_full_hit: report.node_pack_store_skipped_full_hit,
            tier: args.tier.clone(),
            memory_hydrated: report.memory_hydrated,
        },
    );
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
        "[{}] warmup ok: policy={} tier={} worksets={} targets={} slots={} unique_hash={} memory_hydrated={} client_manifest={} failed={} elapsed_ms={} disk_tier_ms={} memory_tier_ms={} client_tier_ms={} disk_bytes={} ({}) eval_compute={} cache_hit={} disk_hit={} l1_hit={} io_read_ops={} io_write_ops={} dedupe_skips={} node_pack_loads={} node_pack_stores={} node_pack_skip_full_hit={} rss_before={:?} rss_after={:?} cpu_user_ms={:?} cpu_system_ms={:?}",
        log_timestamp_rfc3339(),
        args.policy,
        args.tier,
        targets.len(),
        report.target_count,
        report.slot_count,
        report.unique_content_hash_count,
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
        report.l1_cache_hit_count,
        report.io_read_ops,
        report.io_write_ops,
        report.content_hash_dedupe_skips,
        report.node_pack_loads,
        report.node_pack_stores,
        report.node_pack_store_skipped_full_hit,
        report.rss_before_bytes,
        report.rss_after_bytes,
        report.cpu_user_ms,
        report.cpu_system_ms,
    );
    Ok(())
}

pub fn resolve_warmup_targets(
    ctx: &HostContext,
    args: &WarmupArgs,
) -> anyhow::Result<Vec<crate::WarmupTarget>> {
    let filter = WarmupScopeFilter::from_env();
    resolve_warmup_targets_with_filter(ctx, args, &filter)
}

fn resolve_warmup_targets_with_filter(
    ctx: &HostContext,
    args: &WarmupArgs,
    filter: &WarmupScopeFilter,
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
    let scopes = if args.policy.trim() == "all" {
        None
    } else {
        Some(vec![args.policy.trim().to_string()])
    };
    collect_warmup_targets_for_scopes_with_filter(ctx, scopes.as_deref(), filter)
}

pub async fn run_serve(args: crate::cli::ServeArgs) -> anyhow::Result<()> {
    let workspace = args
        .workspace
        .canonicalize()
        .unwrap_or_else(|_| args.workspace.clone());
    let app = args.app.clone();
    tracing::warn!(
        app_id = %app,
        "mei-plug-ds serve is retained as CLI/diagnostics; production Access data plane should use mei-app-runtime"
    );
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
