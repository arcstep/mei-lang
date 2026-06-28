use std::sync::{Arc, RwLock};

use crate::cli::{
    BuildCleanArgs, BuildCommand, BuildFinalizeArgs, BuildMigrateEnvArgs, BuildPrepareArgs,
    BuildPromoteArgs, BuildRollbackArgs, BuildStatusArgs, Command, ImportArgs, PrebuildDataArgs,
    ServeArgs, WarmupArgs,
};
use crate::state::{SharedState, ShellState};

pub async fn dispatch(command: Command) -> anyhow::Result<()> {
    match command {
        Command::Import(args) => run_import(args),
        Command::PrebuildData(args) => run_prebuild_data(args),
        Command::Warmup(args) => run_warmup(args).await,
        Command::Serve(args) => run_serve(args).await,
        Command::Build(sub) => run_build(sub),
    }
}

fn run_build(command: BuildCommand) -> anyhow::Result<()> {
    match command {
        BuildCommand::Prepare(args) => run_build_prepare(args),
        BuildCommand::Finalize(args) => run_build_finalize(args),
        BuildCommand::Promote(args) => run_build_promote(args),
        BuildCommand::Rollback(args) => run_build_rollback(args),
        BuildCommand::Clean(args) => run_build_clean(args),
        BuildCommand::MigrateEnv(args) => run_build_migrate_env(args),
        BuildCommand::Status(args) => run_build_status(args),
    }
}

fn resolve_build_app_ids(workspace: &std::path::Path, apps: &[String]) -> anyhow::Result<Vec<String>> {
    if !apps.is_empty() {
        return Ok(apps.to_vec());
    }
    let cfg = mei_lang_kernel::load_workspace_config(workspace);
    if let Some(default) = cfg
        .workspace
        .default_app
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Ok(vec![default.to_string()]);
    }
    anyhow::bail!("no --app specified and workspace has no defaultApp")
}

fn run_build_prepare(args: BuildPrepareArgs) -> anyhow::Result<()> {
    let workspace = args
        .workspace
        .canonicalize()
        .unwrap_or(args.workspace);
    let app_ids = resolve_build_app_ids(workspace.as_path(), &args.app)?;
    let generation =
        mei_lang_kernel::prepare_dev_build_generation(workspace.as_path(), &app_ids)?;
    println!("{}", generation.env_version);
    Ok(())
}

fn run_build_finalize(args: BuildFinalizeArgs) -> anyhow::Result<()> {
    let workspace = args
        .workspace
        .canonicalize()
        .unwrap_or(args.workspace);
    let app_ids = resolve_build_app_ids(workspace.as_path(), &args.app)?;
    let generation = mei_lang_kernel::PrebuildGeneration {
        env_version: args.build_id.clone(),
        toolchain_version: mei_lang_kernel::resolve_toolchain_version(workspace.as_path()),
        workspace_version: mei_lang_kernel::resolve_workspace_version(workspace.as_path()),
        store_dirs: app_ids
            .iter()
            .map(|app_id| {
                let app_root = mei_lang_kernel::resolve_app_root(workspace.as_path(), app_id);
                (
                    app_id.clone(),
                    mei_lang_kernel::app_env_build_dir(app_root.as_path(), args.build_id.as_str()),
                )
            })
            .collect(),
    };
    let promoted = mei_lang_kernel::finalize_and_promote_build(
        workspace.as_path(),
        &generation,
        &app_ids,
        None,
        None,
        true,
    )?;
    if let Some(build_id) = promoted {
        println!("promoted {build_id}");
    } else {
        println!("finalized candidate {}", args.build_id);
    }
    Ok(())
}

fn run_build_promote(args: BuildPromoteArgs) -> anyhow::Result<()> {
    let workspace = args
        .workspace
        .canonicalize()
        .unwrap_or(args.workspace);
    let build_id = mei_lang_kernel::promote_build(
        workspace.as_path(),
        args.build_id.as_deref(),
    )?;
    println!("promoted {build_id}");
    Ok(())
}

fn run_build_rollback(args: BuildRollbackArgs) -> anyhow::Result<()> {
    let workspace = args
        .workspace
        .canonicalize()
        .unwrap_or(args.workspace);
    let build_id = mei_lang_kernel::rollback_build(workspace.as_path())?;
    println!("rollback active -> {build_id}");
    Ok(())
}

fn run_build_status(args: BuildStatusArgs) -> anyhow::Result<()> {
    let workspace = args
        .workspace
        .canonicalize()
        .unwrap_or(args.workspace);
    let links = mei_lang_kernel::read_links_state(workspace.as_path())?;
    let identity = mei_lang_kernel::resolve_active_build_identity(workspace.as_path());
    let toolchain = links
        .toolchain
        .active
        .as_deref()
        .unwrap_or(identity.toolchain_version.as_str());
    let build_active = links.build.active.as_deref().unwrap_or("-");
    let build_candidate = links.build.candidate.as_deref().unwrap_or("-");
    let build_previous = links.build.previous.as_deref().unwrap_or("-");
    println!("toolchain.active={toolchain}");
    println!("workspace.version={}", identity.workspace_version);
    println!("env.active={build_active}");
    println!("env.candidate={build_candidate}");
    println!("env.previous={build_previous}");
    println!("display={}", mei_lang_kernel::resolve_build_footer_label(workspace.as_path()));
    Ok(())
}

fn run_build_clean(args: BuildCleanArgs) -> anyhow::Result<()> {
    let workspace = args
        .workspace
        .canonicalize()
        .unwrap_or(args.workspace);
    let app_ids = resolve_build_app_ids(workspace.as_path(), &args.app)?;
    let report = mei_lang_kernel::clean_env_generations(
        workspace.as_path(),
        &app_ids,
        &mei_lang_kernel::CleanEnvPolicy {
            dry_run: args.dry_run,
        },
    )?;
    if report.dry_run {
        println!("dry-run: would remove {} env dirs", report.removed.len());
    } else {
        println!("removed {} env dirs", report.removed.len());
    }
    for label in &report.removed {
        println!("  remove {label}");
    }
    for label in &report.retained {
        println!("  keep {label}");
    }
    Ok(())
}

fn run_build_migrate_env(args: BuildMigrateEnvArgs) -> anyhow::Result<()> {
    let workspace = args
        .workspace
        .canonicalize()
        .unwrap_or(args.workspace);
    let app_ids = resolve_build_app_ids(workspace.as_path(), &args.app)?;
    let reports = mei_lang_kernel::migrate_apps_to_env_layout(workspace.as_path(), &app_ids)?;
    for (app_id, report) in reports {
        println!(
            "migrated app={app_id} build_dirs={} var_dirs={} vers={:?} removed_legacy={:?} upgraded={:?}",
            report.migrated_build_dirs,
            report.migrated_var_dirs,
            report.env_versions,
            report.removed_legacy_dirs,
            report.upgraded_env_dirs
        );
    }
    Ok(())
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
