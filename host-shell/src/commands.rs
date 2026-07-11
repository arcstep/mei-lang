use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex, RwLock};

use crate::build_ops::{import_with_options, prebuild_pipeline, resolve_app_id, toolchain_hint};
use crate::cli::{
    AppsCommand, AppsListArgs, BuildCleanArgs, BuildCommand, BuildFinalizeArgs,
    BuildMigrateEnvArgs, BuildPrepareArgs, BuildPromoteArgs, BuildRollbackArgs, BuildStatusArgs,
    Command, EvalCacheCommand, EvalCacheInvalidateArgs, ImportArgs, MrgCommand, MrgStatusArgs,
    PrebuildArgs, PrebuildDataArgs, ReloadArgs, ServeArgs, VersionArgs, WorkspaceCommand,
    WorkspaceInitArgs,
};
use crate::state::{HostHttpState, SharedState, ShellState};

pub async fn dispatch(command: Command) -> anyhow::Result<()> {
    match command {
        Command::Version(args) => run_version(args),
        Command::Import(args) => run_import(args),
        Command::Reload(args) => run_reload(args),
        Command::Prebuild(args) => run_prebuild(args).await,
        Command::PrebuildData(args) => run_prebuild_data(args),
        Command::Mrg(sub) => run_mrg(sub),
        Command::Auth(sub) => {
            mei_host_auth::run_auth_command(mei_host_auth::cli_args::AuthArgs { command: sub })
        }
        Command::Serve(args) => run_serve(args).await,
        Command::Build(sub) => run_build(sub),
        Command::Workspace(sub) => run_workspace(sub),
        Command::Apps(sub) => run_apps(sub),
        Command::EvalCache(sub) => run_eval_cache(sub),
    }
}

fn run_eval_cache(command: EvalCacheCommand) -> anyhow::Result<()> {
    match command {
        EvalCacheCommand::Invalidate(args) => run_eval_cache_invalidate(args),
    }
}

fn run_eval_cache_invalidate(args: EvalCacheInvalidateArgs) -> anyhow::Result<()> {
    let workspace = args.workspace.canonicalize().unwrap_or(args.workspace);
    let report = mei_host_graph::invalidate_app_eval_cache(
        workspace.as_path(),
        args.app.as_str(),
        args.force,
    )?;
    let legacy_cache_cleared = crate::access_page_cache::clear_legacy_page_render_cache_for_app(
        workspace.as_path(),
        args.app.as_str(),
    );
    println!(
        "[{}] eval-cache invalidate ok: app={} force={} removed={} retained={} cleared_bootstrap_scopes={} cleared_legacy_page_render_cache={} removed_bytes={} ({})",
        mei_host_core::log_timestamp_rfc3339(),
        args.app,
        report.force_cleared,
        report.removed_artifact_files,
        report.retained_artifact_files,
        report.cleared_bootstrap_scopes,
        legacy_cache_cleared,
        report.removed_bytes,
        mei_host_core::format_bytes_human(report.removed_bytes),
    );
    Ok(())
}

fn run_apps(command: AppsCommand) -> anyhow::Result<()> {
    match command {
        AppsCommand::List(args) => run_apps_list(args),
    }
}

fn run_apps_list(args: AppsListArgs) -> anyhow::Result<()> {
    let workspace = args.workspace.canonicalize().unwrap_or(args.workspace);
    let apps = crate::landing::discover_workspace_apps(workspace.as_path())?;
    if args.json {
        let ids: Vec<&str> = apps.iter().map(|app| app.id.as_str()).collect();
        println!("{}", serde_json::to_string(&ids)?);
    } else {
        for app in &apps {
            println!("{}", app.id);
        }
    }
    Ok(())
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

fn resolve_build_app_ids(
    workspace: &std::path::Path,
    apps: &[String],
) -> anyhow::Result<Vec<String>> {
    if !apps.is_empty() {
        return Ok(apps.to_vec());
    }
    Ok(vec![resolve_app_id(workspace, None)?])
}

fn cli_toolchain_hint() -> &'static str {
    toolchain_hint()
}

fn run_build_prepare(args: BuildPrepareArgs) -> anyhow::Result<()> {
    let workspace = args.workspace.canonicalize().unwrap_or(args.workspace);
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
    let workspace = args.workspace.canonicalize().unwrap_or(args.workspace);
    let app_ids = resolve_build_app_ids(workspace.as_path(), &args.app)?;
    let generation = mei_lang_kernel::PrebuildGeneration {
        env_version: args.build_id.clone(),
        build_generation: args.build_id.clone(),
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
    let workspace = args.workspace.canonicalize().unwrap_or(args.workspace);
    let build_id = mei_lang_kernel::promote_build(workspace.as_path(), args.build_id.as_deref())?;
    println!("promoted {build_id}");
    Ok(())
}

fn run_build_rollback(args: BuildRollbackArgs) -> anyhow::Result<()> {
    let workspace = args.workspace.canonicalize().unwrap_or(args.workspace);
    let build_id = mei_lang_kernel::rollback_build(workspace.as_path())?;
    println!("rollback active -> {build_id}");
    Ok(())
}

fn run_build_status(args: BuildStatusArgs) -> anyhow::Result<()> {
    let workspace = args.workspace.canonicalize().unwrap_or(args.workspace);
    let links = mei_lang_kernel::read_links_state(workspace.as_path())?;
    let identity = mei_lang_kernel::resolve_active_build_identity(workspace.as_path());
    let toolchain = links
        .toolchain
        .active
        .as_deref()
        .unwrap_or(identity.meilang_version.as_str());
    let build_candidate = links.build.candidate.as_deref().unwrap_or("-");
    let build_previous = links.build.previous.as_deref().unwrap_or("-");
    println!("toolchain.active={toolchain}");
    println!("workspace.version={}", identity.workspace_version);
    println!("links.candidate={build_candidate}");
    println!("links.previous={build_previous}");
    let apps = mei_lang_kernel::discover_apps(workspace.as_path())?;
    for app in &apps {
        let app_root = mei_lang_kernel::resolve_app_root(workspace.as_path(), app.id.as_str());
        let current =
            mei_lang_kernel::resolve_app_build_generation_from_current(app_root.as_path())
                .unwrap_or_else(|_| "-".to_string());
        println!("app={} current={current}", app.id);
    }
    println!(
        "display={}",
        mei_lang_kernel::resolve_build_footer_label(workspace.as_path())
    );
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
    let workspace = args.workspace.canonicalize().unwrap_or(args.workspace);
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
    let workspace = args.workspace.canonicalize().unwrap_or(args.workspace);
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
    let prev_revision =
        mei_host_graph::McgRegistryWriter::load(workspace.as_path(), args.app.as_str())
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
    let workspace = args.workspace.canonicalize().unwrap_or(args.workspace);
    let app = args.app.as_str();

    println!("==> build prepare + compile + import + prebuild-data + warmup + finalize");
    prebuild_pipeline(workspace.as_path(), app, args.policy.as_str())?;
    Ok(())
}

fn run_workspace_init(args: WorkspaceInitArgs) -> anyhow::Result<()> {
    let dir = args.dir.canonicalize().unwrap_or(args.dir);
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

    mei_host_core::ensure_workspace_stock_materialized(dir.as_path(), package_root.as_path())?;

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
    url = "/apps/{app_id}/home",
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
    let workspace = args.workspace.canonicalize().unwrap_or(args.workspace);
    let report =
        mei_host_graph::publish_app_data_snapshots(workspace.as_path(), args.app.as_str())?;
    println!(
        "[{}] prebuild-data ok: app={} discovered={} written={} skipped={} total_bytes={} ({}) manifest={}",
        mei_host_core::log_timestamp_rfc3339(),
        report.app_id,
        report.discovered_sources.len(),
        report.written.len(),
        report.skipped.len(),
        report.total_written_bytes,
        mei_host_core::format_bytes_human(report.total_written_bytes),
        report.manifest_path
    );
    for path in &report.written {
        let full = workspace.join(path);
        let size_label = full
            .metadata()
            .map(|metadata| mei_host_core::format_bytes_human(metadata.len()))
            .unwrap_or_else(|_| "?".to_string());
        println!("  wrote {path} ({size_label})");
    }
    for skip in &report.skipped {
        eprintln!("warning: skipped {skip}");
    }
    if report.written.is_empty()
        && !report.discovered_sources.is_empty()
        && report.skipped.is_empty()
    {
        eprintln!("warning: no parquet files written despite discovered sources");
    }
    Ok(())
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
    let early_bind = std::env::var("MEI_SERVE_EARLY_BIND")
        .map(|value| {
            let trimmed = value.trim();
            trimmed == "1" || trimmed.eq_ignore_ascii_case("true")
        })
        .unwrap_or(false);
    if early_bind {
        run_serve_early_bind(args).await
    } else {
        run_serve_blocking_init(args).await
    }
}

fn serve_data_mode_ceiling(args: &ServeArgs) -> anyhow::Result<mei_lang_kernel::DataModeCeiling> {
    crate::review_axes::parse_data_mode_ceiling_arg(args.data_mode_ceiling.as_str())
        .map_err(anyhow::Error::msg)
}

async fn run_serve_blocking_init(args: ServeArgs) -> anyhow::Result<()> {
    use std::collections::BTreeMap;

    let workspace = args
        .workspace
        .canonicalize()
        .unwrap_or(args.workspace.clone());
    crate::build_info::log_host_identity(Some(workspace.as_path()), "serve");
    let package_root = resolve_package_root()?;
    let data_mode_ceiling = serve_data_mode_ceiling(&args)?;
    if let Some(report) = mei_host_core::ensure_workspace_stock_materialized(
        workspace.as_path(),
        package_root.as_path(),
    )? {
        if report.components.copied_files > 0
            || report.templates.copied_files > 0
            || report.authoring.copied_files > 0
        {
            println!(
                "Stock:     refreshed workspace stock (components={} templates={} authoring={})",
                report.components.copied_files,
                report.templates.copied_files,
                report.authoring.copied_files,
            );
        }
    }
    let default_app_id = args.app.clone();
    let default_ctx = mei_host_core::HostContext::new(workspace.clone(), default_app_id.clone());
    ensure_registry_materialized(&default_ctx)?;
    let discovered = crate::landing::discover_workspace_apps(workspace.as_path())?;
    let app_ids: Vec<String> = if discovered.is_empty() {
        vec![default_app_id.clone()]
    } else {
        discovered.into_iter().map(|app| app.id).collect()
    };
    let external_plug_ds = crate::plug_proxy::configured_plug_ds_endpoint(&default_ctx);
    let mut managed_pool = None;
    let mut plug_ds_by_app = BTreeMap::new();
    if data_mode_ceiling.requires_plug_ds() {
        if let Some(endpoint) = external_plug_ds.as_ref() {
            plug_ds_by_app.insert(default_app_id.clone(), endpoint.clone());
        } else {
            let pool = crate::managed_plug::spawn_managed_plug_ds_pool(
                workspace.as_path(),
                app_ids.as_slice(),
            )
            .await?;
            plug_ds_by_app = pool.endpoints.clone();
            managed_pool = Some(pool);
        }
        if plug_ds_by_app.is_empty() {
            anyhow::bail!("no plug-ds endpoints available for serve");
        }
    }
    let auth_enforcement = if args.auth {
        mei_host_auth::AuthEnforcement::Required
    } else {
        mei_host_auth::AuthEnforcement::Disabled
    };
    mei_host_auth::prepare_auth_for_serve(workspace.as_path(), auth_enforcement, "mei-host-shell")?;
    let shell: SharedState = Arc::new(RwLock::new({
        let mut state = ShellState::new(
            workspace.clone(),
            default_app_id.clone(),
            package_root.clone(),
            plug_ds_by_app.clone(),
            managed_pool.is_some(),
        );
        state.data_mode_ceiling = data_mode_ceiling;
        state
    }));
    refresh_host_materialization_flags(&shell);
    let discovered =
        crate::landing::discover_workspace_apps(workspace.as_path()).unwrap_or_default();
    let app_ids: Vec<String> = if discovered.is_empty() {
        vec![default_app_id.clone()]
    } else {
        discovered.into_iter().map(|app| app.id).collect()
    };
    let _legacy_cleared = crate::access_page_cache::clear_legacy_page_render_cache_for_apps(
        workspace.as_path(),
        app_ids.as_slice(),
    );
    let addr = format!("{}:{}", args.host, args.port);
    let listen_url = format!("http://{addr}");
    let guard = shell.read().expect("state lock");
    let mut warmup_lines = crate::startup::build_access_ready_banner_lines(
        &guard,
        app_ids.as_slice(),
        "home",
        listen_url.as_str(),
    );
    warmup_lines.push("blocking serve — port opens after warmup".to_string());
    drop(guard);
    let warmup_refs: Vec<&str> = warmup_lines.iter().map(String::as_str).collect();
    crate::startup_banner::emit_access_warmup_ready_banner(warmup_refs.as_slice());
    let auth_state = mei_host_auth::AuthServeState::new(workspace.clone(), auth_enforcement);
    let managed_plug = Arc::new(Mutex::new(managed_pool));
    let state = HostHttpState {
        shell,
        auth: auth_state.clone(),
        managed_plug: managed_plug.clone(),
    };
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    if args.auth {
        println!("Auth:      enabled (login required for protected routes)");
    }
    if external_plug_ds.is_some() {
        println!(
            "Plug-ds:   external {} (default app {default_app_id})",
            external_plug_ds.as_deref().unwrap_or("-")
        );
    } else {
        println!(
            "Plug-ds:   managed by host-shell ({} app(s))",
            plug_ds_by_app.len()
        );
        for (app_id, endpoint) in &plug_ds_by_app {
            println!("           {app_id} -> {endpoint}");
        }
    }
    let version_line = crate::build_info::host_version_banner_line(workspace.as_path());
    let listen_detail = vec![
        version_line.as_str(),
        "blocking serve — access pages ready immediately",
    ];
    crate::startup_banner::emit_host_listening_banner(
        listen_url.as_str(),
        listen_detail.as_slice(),
    );
    let app = crate::http::router(state)
        .layer(axum::middleware::from_fn_with_state(
            auth_state,
            mei_host_auth::auth_middleware,
        ))
        .layer(axum::middleware::from_fn(
            crate::request_logging::log_request,
        ));
    let serve_result = axum::serve(listener, app)
        .await
        .map_err(|e| anyhow::anyhow!(e));
    if let Some(mut pool) = managed_plug.lock().ok().and_then(|mut guard| guard.take()) {
        if let Err(error) = pool.shutdown().await {
            tracing::warn!(detail = %error, "managed plug-ds pool shutdown failed");
        }
    }
    serve_result
}

async fn run_serve_early_bind(args: ServeArgs) -> anyhow::Result<()> {
    use std::collections::BTreeMap;

    let workspace = args
        .workspace
        .canonicalize()
        .unwrap_or(args.workspace.clone());
    crate::build_info::log_host_identity(Some(workspace.as_path()), "serve");
    let package_root = resolve_package_root()?;
    let data_mode_ceiling = serve_data_mode_ceiling(&args)?;
    let default_app_id = args.app.clone();
    let discovered = crate::landing::discover_workspace_apps(workspace.as_path())?;
    let app_ids: Vec<String> = if discovered.is_empty() {
        vec![default_app_id.clone()]
    } else {
        discovered.into_iter().map(|app| app.id).collect()
    };
    let auth_enforcement = if args.auth {
        mei_host_auth::AuthEnforcement::Required
    } else {
        mei_host_auth::AuthEnforcement::Disabled
    };
    mei_host_auth::prepare_auth_for_serve(workspace.as_path(), auth_enforcement, "mei-host-shell")?;
    let shell: SharedState = Arc::new(RwLock::new({
        let mut state = ShellState::new(
            workspace.clone(),
            default_app_id.clone(),
            package_root.clone(),
            BTreeMap::new(),
            false,
        );
        state.data_mode_ceiling = data_mode_ceiling;
        state
    }));
    let managed_plug = Arc::new(Mutex::new(None::<crate::managed_plug::ManagedPlugDsPool>));
    let auth_state = mei_host_auth::AuthServeState::new(workspace.clone(), auth_enforcement);
    let state = HostHttpState {
        shell: shell.clone(),
        auth: auth_state.clone(),
        managed_plug: managed_plug.clone(),
    };
    let addr = format!("{}:{}", args.host, args.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    let listen_url = format!("http://{addr}");
    if args.auth {
        println!("Auth:      enabled (login required for protected routes)");
    }
    let version_line = crate::build_info::host_version_banner_line(workspace.as_path());
    let defer_line = if crate::startup::defer_warmup_to_prebuild() {
        "early bind — unready routes redirect to /host/starting until ACCESS READY"
    } else {
        "early bind — background startup in progress"
    };
    crate::startup_banner::emit_host_listening_banner(
        listen_url.as_str(),
        &[version_line.as_str(), defer_line],
    );
    let startup_plan = crate::startup::ServeStartupPlan {
        workspace: workspace.clone(),
        package_root: package_root.clone(),
        default_app_id: default_app_id.clone(),
        listen_url,
        app_ids: app_ids.clone(),
        data_mode_ceiling,
        managed_plug_slot: managed_plug,
    };
    tokio::spawn(crate::startup::run_background_startup(shell, startup_plan));
    let managed_plug_for_shutdown = state.managed_plug.clone();
    let app = crate::http::router(state)
        .layer(axum::middleware::from_fn(
            crate::request_logging::log_request,
        ))
        .layer(axum::middleware::from_fn_with_state(
            auth_state,
            mei_host_auth::auth_middleware,
        ));
    let serve_result = axum::serve(listener, app)
        .await
        .map_err(|e| anyhow::anyhow!(e));
    if let Some(mut pool) = managed_plug_for_shutdown
        .lock()
        .ok()
        .and_then(|mut guard| guard.take())
    {
        if let Err(error) = pool.shutdown().await {
            tracing::warn!(detail = %error, "managed plug-ds pool shutdown failed");
        }
    }
    serve_result
}

fn ensure_registry_materialized(ctx: &mei_host_core::HostContext) -> anyhow::Result<()> {
    let mcg_path =
        mei_host_graph::mcg_registry_path(ctx.workspace_root.as_path(), ctx.app_id.as_str());
    if mcg_path.is_file() {
        let registry = mei_host_graph::McgRegistryWriter::load(
            ctx.workspace_root.as_path(),
            ctx.app_id.as_str(),
        );
        if !registry.nodes.is_empty() {
            return Ok(());
        }
    }
    let bundle_path = ctx.bundle_path();
    if !bundle_path.is_file() {
        anyhow::bail!(
            "MCG registry missing and bundle not found at {}; run prebuild or `mei-host-shell import`",
            mei_host_core::path_for_log(ctx.workspace_root.as_path(), bundle_path.as_path())
        );
    }
    tracing::info!(
        bundle = %mei_host_core::path_for_log(ctx.workspace_root.as_path(), bundle_path.as_path()),
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
    crate::build_ops::refresh_materialization_flags(&mut guard);
}

fn resolve_package_root() -> anyhow::Result<std::path::PathBuf> {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    Ok(manifest_dir.parent().unwrap_or(&manifest_dir).to_path_buf())
}
