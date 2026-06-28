use std::fs;
use std::path::Path;
use std::sync::{Arc, RwLock};

use crate::cli::{
    BuildCleanArgs, BuildCommand, BuildFinalizeArgs, BuildMigrateEnvArgs, BuildPrepareArgs,
    BuildPromoteArgs, BuildRollbackArgs, BuildStatusArgs, Command, ImportArgs, MrgCommand,
    MrgStatusArgs, PrebuildArgs, PrebuildDataArgs, ReloadArgs, ServeArgs, VersionArgs, WarmupArgs,
    WorkspaceCommand, WorkspaceInitArgs,
};
use crate::state::{SharedState, ShellState};

pub async fn dispatch(command: Command) -> anyhow::Result<()> {
    match command {
        Command::Version(args) => run_version(args),
        Command::Import(args) => run_import(args),
        Command::Reload(args) => run_reload(args),
        Command::Prebuild(args) => run_prebuild(args).await,
        Command::PrebuildData(args) => run_prebuild_data(args),
        Command::Warmup(args) => run_warmup(args).await,
        Command::Mrg(sub) => run_mrg(sub),
        Command::Serve(args) => run_serve(args).await,
        Command::Build(sub) => run_build(sub),
        Command::Workspace(sub) => run_workspace(sub),
    }
}

fn run_workspace(command: WorkspaceCommand) -> anyhow::Result<()> {
    match command {
        WorkspaceCommand::Init(args) => run_workspace_init(args),
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

fn cli_toolchain_hint() -> &'static str {
    crate::build_info::CARGO_PACKAGE_VERSION
}

fn run_build_prepare(args: BuildPrepareArgs) -> anyhow::Result<()> {
    let workspace = args
        .workspace
        .canonicalize()
        .unwrap_or(args.workspace);
    let app_ids = resolve_build_app_ids(workspace.as_path(), &args.app)?;
    let generation = mei_lang_kernel::prepare_dev_build_generation_with_hint(
        workspace.as_path(),
        &app_ids,
        Some(cli_toolchain_hint()),
    )?;
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
        toolchain_version: mei_lang_kernel::resolve_toolchain_version_with_hint(
            workspace.as_path(),
            Some(cli_toolchain_hint()),
        ),
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
    println!("shell.build_version={}", crate::build_info::BUILD_VERSION);
    Ok(())
}

fn run_version(args: VersionArgs) -> anyhow::Result<()> {
    let workspace = args
        .workspace
        .map(|path| path.canonicalize().unwrap_or(path));
    crate::build_info::print_cli_version(workspace.as_deref(), args.json)
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
    let report = import_with_options(&args.workspace, &args.app, args.bundle)?;
    print_import_report(&report);
    Ok(())
}

fn run_reload(args: ReloadArgs) -> anyhow::Result<()> {
    let workspace = args
        .workspace
        .canonicalize()
        .unwrap_or(args.workspace.clone());
    let prev_revision = mei_host_graph::McgRegistryWriter::load(workspace.as_path(), args.app.as_str())
        .registry_revision
        .clone();
    let report = import_with_options(&workspace, &args.app, args.bundle)?;
    let changed = report.registry_revision != prev_revision;
    if args.json {
        let payload = serde_json::json!({
            "accepted": true,
            "blocks_changed": changed,
            "block_count": report.block_count,
            "registry_revision": report.registry_revision,
            "previous_revision": prev_revision,
        });
        println!("{}", serde_json::to_string_pretty(&payload)?);
    } else {
        print_import_report(&report);
        if !changed {
            println!("reload: registry unchanged (no structural diff)");
        } else {
            println!("reload: registry updated");
        }
    }
    Ok(())
}

fn import_with_options(
    workspace: &Path,
    app: &str,
    bundle: Option<std::path::PathBuf>,
) -> anyhow::Result<mei_host_core::ImportReport> {
    let workspace = workspace.canonicalize().unwrap_or_else(|_| workspace.to_path_buf());
    crate::build_info::log_host_identity(Some(workspace.as_path()), "import");
    let ctx = mei_host_core::HostContext::new(workspace, app.to_string());
    let options = mei_host_graph::ImportOptions { bundle_path: bundle };
  mei_host_graph::import_bundle(&ctx, &options).map_err(|e| anyhow::anyhow!("{e}"))
}

fn print_import_report(report: &mei_host_core::ImportReport) {
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
}

async fn run_prebuild(args: PrebuildArgs) -> anyhow::Result<()> {
    let workspace = args
        .workspace
        .canonicalize()
        .unwrap_or(args.workspace);
    let app = args.app.as_str();
    let app_root = mei_lang_kernel::resolve_app_root(workspace.as_path(), app);

    println!("==> build prepare");
    let generation = mei_lang_kernel::prepare_dev_build_generation_with_hint(
        workspace.as_path(),
        &[app.to_string()],
        Some(cli_toolchain_hint()),
    )?;
    let build_id = generation.env_version.clone();
    println!("envVersion={build_id}");

    println!("==> compile");
    compile_app_to_bundle(workspace.as_path(), app)?;

    println!("==> import");
    let report = import_with_options(workspace.as_path(), app, None)?;
    print_import_report(&report);

    println!("==> prebuild-data");
    run_prebuild_data(PrebuildDataArgs {
        workspace: workspace.clone(),
        app: app.to_string(),
    })?;

    println!("==> clear eval-cache");
    let eval_cache = mei_lang_kernel::resolve_app_eval_cache_root(app_root.as_path());
    if eval_cache.exists() {
        fs::remove_dir_all(&eval_cache)?;
    }

    println!("==> warmup policy={}", args.policy);
    run_warmup(WarmupArgs {
        workspace: workspace.clone(),
        app: app.to_string(),
        policy: args.policy.clone(),
        tier: "disk".to_string(),
        board: None,
        frontier: None,
        hops: 0,
    })
    .await?;

    println!("==> build finalize");
    run_build_finalize(BuildFinalizeArgs {
        workspace,
        app: vec![app.to_string()],
        build_id: build_id.clone(),
    })?;

    println!("Prebuild complete (envVersion={build_id}).");
    Ok(())
}

fn compile_app_to_bundle(workspace: &Path, app_id: &str) -> anyhow::Result<()> {
    let outcome = mei_graph::compile_app(workspace, app_id)
        .map_err(|e| anyhow::anyhow!("compile failed: {e}"))?;
    let templates_rel = read_templates_rel(workspace);
    let digest = mei_bundle::compute_workspace_digest(workspace, app_id, templates_rel.as_str());
    let bundle_path = mei_bundle::default_bundle_path(workspace, app_id);
    if let Some(parent) = bundle_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let stats = mei_bundle::write_bundle_from_outcome(
        &outcome,
        digest.as_str(),
        env!("CARGO_PKG_VERSION"),
        bundle_path.as_path(),
        false,
    )
    .map_err(|e| anyhow::anyhow!("write bundle: {e}"))?;
    println!(
        "wrote {} ({} blocks, {} bytes)",
        bundle_path.display(),
        stats.manifest.block_count,
        stats.bundle_bytes
    );
    Ok(())
}

fn read_templates_rel(workspace: &Path) -> String {
    let cfg = mei_lang_kernel::load_workspace_config(workspace);
    cfg.paths
        .templates
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("stock/templates")
        .to_string()
}

fn run_workspace_init(args: WorkspaceInitArgs) -> anyhow::Result<()> {
    let dir = args
        .dir
        .canonicalize()
        .unwrap_or(args.dir);
    let package_root = resolve_package_root()?;
    let profile_id = args
        .id
        .as_deref()
        .or_else(|| dir.file_name().and_then(|n| n.to_str()))
        .filter(|s| !s.is_empty())
        .unwrap_or("workspace");

    fs::create_dir_all(&dir)?;
    fs::create_dir_all(dir.join("apps"))?;
    fs::create_dir_all(dir.join("deploy"))?;
    fs::create_dir_all(dir.join("deploy/runtime"))?;
    fs::create_dir_all(dir.join("deploy/state"))?;

    let config_path = mei_lang_kernel::workspace_config_path(dir.as_path());
    if !config_path.is_file() {
        let pin = env!("CARGO_PKG_VERSION");
        let config = mei_lang_kernel::WorkspaceConfig {
            schema_version: 2,
            workspace: mei_lang_kernel::WorkspaceProfile {
                id: Some(profile_id.to_string()),
                label: args.label.clone(),
                deploy_host: None,
                default_app: args.app.clone(),
                version: None,
            },
            paths: mei_lang_kernel::WorkspacePathsConfig {
                apps: Some(mei_lang_kernel::DEFAULT_APPS_REL.to_string()),
                components: Some("stock/components".to_string()),
                templates: Some("stock/templates".to_string()),
                authoring: Some(mei_lang_kernel::DEFAULT_STOCK_AUTHORING_REL.to_string()),
                runtime: Some("deploy/runtime".to_string()),
                deploy: Some("deploy".to_string()),
                stock: Some("stock".to_string()),
                ..mei_lang_kernel::WorkspacePathsConfig::default()
            },
            stock: default_workspace_stock_config(),
            toolchain: mei_lang_kernel::WorkspaceToolchainConfig {
                pin: Some(pin.to_string()),
            },
            ..mei_lang_kernel::WorkspaceConfig::default()
        };
        mei_lang_kernel::write_workspace_config(&config_path, &config)?;
        let mei_lang_path = dir.join("mei.lang.json");
        if !mei_lang_path.is_file() {
            fs::write(
                mei_lang_path,
                format!(r#"{{"syntaxVersion":"{pin}","surface":"graph-native"}}"#),
            )?;
        }
    }

    mei_lang_toolchain::ensure_workspace_stock_materialized(dir.as_path(), package_root.as_path())?;

    if let Some(app_id) = args.app.as_deref().filter(|s| !s.trim().is_empty()) {
        create_v2_app_skeleton(dir.as_path(), app_id.trim())?;
    }

    println!("workspace init ok: {}", dir.display());
    Ok(())
}

fn default_workspace_stock_config() -> mei_lang_kernel::WorkspaceStockConfig {
    mei_lang_kernel::WorkspaceStockConfig {
        bootstrap: mei_lang_kernel::WorkspaceStockBootstrapConfig {
            source: Some("platform-default".to_string()),
        },
        catalog: mei_lang_kernel::WorkspaceStockCatalogConfig {
            components: mei_lang_kernel::WorkspaceStockCatalogKindConfig {
                enabled: true,
                exclude: Vec::new(),
            },
            templates: mei_lang_kernel::WorkspaceStockCatalogKindConfig {
                enabled: true,
                exclude: vec!["**/assets/**".to_string()],
            },
            authoring: mei_lang_kernel::WorkspaceStockCatalogKindConfig {
                enabled: true,
                exclude: Vec::new(),
            },
        },
        preview: mei_lang_kernel::WorkspaceStockPreviewConfig {
            workspace_only: true,
            ..mei_lang_kernel::WorkspaceStockPreviewConfig::default()
        },
        catalog_app: mei_lang_kernel::WorkspaceStockCatalogAppConfig::default(),
        sources: Vec::new(),
    }
}

fn create_v2_app_skeleton(workspace: &Path, app_id: &str) -> anyhow::Result<()> {
    let app_root = workspace.join("apps").join(app_id);
    if app_root.exists() {
        return Ok(());
    }
    fs::create_dir_all(app_root.join("src"))?;
    fs::create_dir_all(app_root.join("upload"))?;
    fs::write(
        app_root.join("src/app.mei"),
        format!(
            r#"# BlockId: app_skeleton:{app_id}

app_skeleton(
    id = "{app_id}",
    title = "{app_id}",
    default_scene = "home",
)

navigation(
    key = "default_access",
    scene = "home",
    url = "/apps/app/{app_id}/scene/home",
    assembly = assembly_ref("home@src/scene/home/assembly.mei"),
)
"#
        ),
    )?;
    fs::write(
        app_root.join("app.config.json"),
        r#"{
  "schemaVersion": 1,
  "entry": { "main": "src/app.mei" },
  "paths": { "upload": "upload" }
}
"#,
    )?;
    println!("created app skeleton: apps/{app_id}");
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
    let ctx = mei_host_core::HostContext::new(args.workspace.clone(), args.app.clone());
    #[cfg(feature = "ds")]
    {
        let tier = mei_plug_ds::WarmupTier::parse(args.tier.as_str());
        let targets = resolve_warmup_targets(&ctx, &args)?;
        let report = mei_plug_ds::run_warmup_targets_with_tier(&ctx, &targets, tier)?;
        if args.hops > 0 {
            let scope = args
                .frontier
                .as_deref()
                .or(Some(args.policy.as_str()))
                .unwrap_or("home");
            let edges =
                mei_host_graph::record_navigation_edges_for_scope(&ctx, scope, args.hops)?;
            if edges > 0 {
                println!("warmup navigation edges added: {edges}");
            }
        }
        println!(
            "warmup ok: policy={} tier={} worksets={} slots={} memory_hydrated={} client_manifest={} failed={}",
            args.policy,
            args.tier,
            targets.len(),
            report.slot_count,
            report.memory_hydrated,
            report.client_manifest_written,
            report.failed_count
        );
    }
    #[cfg(not(feature = "ds"))]
    {
        let _ = (ctx, args);
        anyhow::bail!("warmup requires feature ds");
    }
    Ok(())
}

#[cfg(feature = "ds")]
fn resolve_warmup_targets(
    ctx: &mei_host_core::HostContext,
    args: &WarmupArgs,
) -> anyhow::Result<Vec<mei_plug_ds::WarmupTarget>> {
    if let Some(board) = args
        .board
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let metrics = mei_host_graph::collect_eval_frontier(ctx, board)?;
        return Ok(mei_plug_ds::frontier_targets_from_metrics(board, &metrics));
    }
    if let Some(frontier) = args
        .frontier
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let metrics =
            mei_host_graph::collect_eval_frontier_with_hops(ctx, frontier, args.hops)?;
        return Ok(mei_plug_ds::frontier_targets_from_metrics(frontier, &metrics));
    }
    mei_plug_ds::collect_warmup_targets(ctx, Some(args.policy.as_str()))
}

fn run_mrg(command: MrgCommand) -> anyhow::Result<()> {
    match command {
        MrgCommand::Status(args) => run_mrg_status(args),
    }
}

fn run_mrg_status(args: MrgStatusArgs) -> anyhow::Result<()> {
    let workspace = args
        .workspace
        .canonicalize()
        .unwrap_or(args.workspace.clone());
    let _ = mei_host_graph::flush_telemetry_to_registry(workspace.as_path(), args.app.as_str());
    let status = mei_host_graph::mrg_status_json(workspace.as_path(), args.app.as_str())?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&status)?);
    } else {
        println!(
            "mrg status: app={} slots={} disk_ready={} memory_resident={} client_eligible={}",
            status
                .get("appId")
                .and_then(|value| value.as_str())
                .unwrap_or("-"),
            status
                .get("slotCount")
                .and_then(|value| value.as_u64())
                .unwrap_or(0),
            status
                .get("slotsByTier")
                .and_then(|value| value.get("diskReady"))
                .and_then(|value| value.as_u64())
                .unwrap_or(0),
            status
                .get("slotsByTier")
                .and_then(|value| value.get("memoryResident"))
                .and_then(|value| value.as_u64())
                .unwrap_or(0),
            status
                .get("slotsByTier")
                .and_then(|value| value.get("clientEligible"))
                .and_then(|value| value.as_u64())
                .unwrap_or(0),
        );
    }
    Ok(())
}

async fn run_serve(args: ServeArgs) -> anyhow::Result<()> {
    let workspace = args
        .workspace
        .canonicalize()
        .unwrap_or(args.workspace.clone());
    crate::build_info::log_host_identity(Some(workspace.as_path()), "serve");
    let package_root = resolve_package_root()?;
    let ctx = mei_host_core::HostContext::new(workspace.clone(), args.app.clone());
    ensure_registry_materialized(&ctx)?;
    #[cfg(feature = "ds")]
    if args.warm_on_start {
        let tier = mei_plug_ds::WarmupTier::parse(args.warm_tier.as_str());
        if tier == mei_plug_ds::WarmupTier::Memory {
            let hydrated = mei_plug_ds::hydrate_existing_l1_slots(&ctx, "home")?;
            tracing::info!(hydrated, "serve warm-on-start hydrated L1 slots to memory");
        } else {
            let targets = mei_plug_ds::collect_warmup_targets(&ctx, Some("home"))?;
            let report = mei_plug_ds::run_warmup_targets_with_tier(&ctx, &targets, tier)?;
            tracing::info!(
                slots = report.slot_count,
                memory_hydrated = report.memory_hydrated,
                "serve warm-on-start completed"
            );
        }
    }
    let state: SharedState = Arc::new(RwLock::new(ShellState::new(
        workspace.clone(),
        args.app,
        package_root,
    )));
    refresh_host_materialization_flags(&state);
    let addr = format!("{}:{}", args.host, args.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    let display = mei_lang_kernel::resolve_build_footer_label(workspace.as_path());
    println!(
        "mei-host-shell listening on http://{addr} (shell {} · {})",
        crate::build_info::BUILD_VERSION,
        display
    );
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
