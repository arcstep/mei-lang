use std::sync::{Arc, RwLock};

use crate::cli::{Command, ImportArgs, PrebuildDataArgs, ServeArgs, WarmupArgs};
use crate::state::{SharedState, ShellState};

pub async fn dispatch(command: Command) -> anyhow::Result<()> {
    match command {
        Command::Import(args) => run_import(args),
        Command::PrebuildData(args) => run_prebuild_data(args),
        Command::Warmup(args) => run_warmup(args).await,
        Command::Serve(args) => run_serve(args).await,
    }
}

fn run_import(args: ImportArgs) -> anyhow::Result<()> {
    let ctx = mei_host_core::HostContext::new(args.workspace, args.app);
    let options = mei_host_graph::ImportOptions {
        bundle_path: args.bundle,
    };
    let report = mei_host_graph::import_bundle(&ctx, &options)?;
    println!(
        "import ok: app={} blocks={} mcg_nodes={} cas_upserts={} revision={}",
        report.app_id,
        report.block_count,
        report.mcg_nodes,
        report.cas_upserts,
        report.registry_revision
    );
    if !report.warnings.is_empty() {
        for warning in &report.warnings {
            eprintln!("warning: {warning}");
        }
    }
    Ok(())
}

fn run_prebuild_data(args: PrebuildDataArgs) -> anyhow::Result<()> {
    let report = mei_host_graph::publish_app_data_snapshots(args.workspace.as_path(), args.app.as_str())?;
    println!(
        "prebuild-data ok: app={} discovered={} written={} skipped={} manifest={}",
        report.app_id,
        report.discovered_sources.len(),
        report.written.len(),
        report.skipped.len(),
        report.manifest_path
    );
    for path in &report.written {
        println!("  wrote {path}");
    }
    for skip in &report.skipped {
        eprintln!("warning: skipped {skip}");
    }
    if report.written.is_empty() && !report.discovered_sources.is_empty() && report.skipped.is_empty()
    {
        eprintln!("warning: no parquet files written despite discovered sources");
    }
    Ok(())
}

async fn run_warmup(args: WarmupArgs) -> anyhow::Result<()> {
    let ctx = mei_host_core::HostContext::new(args.workspace, args.app);
    #[cfg(feature = "ds")]
    {
        let targets = mei_plug_ds::collect_warmup_targets(&ctx, Some(args.policy.as_str()))?;
        let result = mei_plug_ds::materialize_targets(&ctx, &targets)?;
        mei_host_graph::record_slots_from_descriptors(
            ctx.workspace_root.as_path(),
            ctx.app_id.as_str(),
            &result.slots,
        )?;
        println!(
            "warmup ok: policy={} worksets={} slots={}",
            args.policy,
            targets.len(),
            result.slots.len()
        );
    }
    #[cfg(not(feature = "ds"))]
    {
        let _ = (ctx, args);
        anyhow::bail!("warmup requires feature ds");
    }
    Ok(())
}

async fn run_serve(args: ServeArgs) -> anyhow::Result<()> {
    let workspace = args
        .workspace
        .canonicalize()
        .unwrap_or(args.workspace.clone());
    let package_root = resolve_package_root()?;
    let ctx = mei_host_core::HostContext::new(workspace.clone(), args.app.clone());
    ensure_registry_materialized(&ctx)?;
    let state: SharedState = Arc::new(RwLock::new(ShellState::new(
        workspace,
        args.app,
        package_root,
    )));
    refresh_host_materialization_flags(&state);
    let addr = format!("{}:{}", args.host, args.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!("mei-host-shell listening on http://{addr}");
    let app = crate::http::router(state)
        .layer(axum::middleware::from_fn(crate::request_logging::log_request));
    axum::serve(listener, app)
        .await
        .map_err(|e| anyhow::anyhow!(e))
}

fn ensure_registry_materialized(ctx: &mei_host_core::HostContext) -> anyhow::Result<()> {
    let mcg_path = mei_host_graph::mcg_registry_path(
        ctx.workspace_root.as_path(),
        ctx.app_id.as_str(),
    );
    if mcg_path.is_file() {
        let registry =
            mei_host_graph::McgRegistryWriter::load(ctx.workspace_root.as_path(), ctx.app_id.as_str());
        if !registry.nodes.is_empty() {
            return Ok(());
        }
    }
    let bundle_path = ctx.bundle_path();
    if !bundle_path.is_file() {
        anyhow::bail!(
            "MCG registry missing and bundle not found at {}; run prebuild or `mei-host-shell import`",
            bundle_path.display()
        );
    }
    tracing::info!(
        bundle = %bundle_path.display(),
        "auto-importing meibundle before serve"
    );
    mei_host_graph::import_bundle(
        ctx,
        &mei_host_graph::ImportOptions {
            bundle_path: Some(bundle_path),
        },
    )?;
    Ok(())
}

fn refresh_host_materialization_flags(state: &SharedState) {
    let mut guard = state.write().expect("state lock");
    guard.imported = mei_host_graph::mcg_registry_path(
        guard.ctx.workspace_root.as_path(),
        guard.ctx.app_id.as_str(),
    )
    .is_file();
    guard.warmed_up = mei_host_graph::mrg_registry_path(
        guard.ctx.workspace_root.as_path(),
        guard.ctx.app_id.as_str(),
    )
    .is_file();
}

fn resolve_package_root() -> anyhow::Result<std::path::PathBuf> {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    Ok(manifest_dir.parent().unwrap_or(&manifest_dir).to_path_buf())
}
