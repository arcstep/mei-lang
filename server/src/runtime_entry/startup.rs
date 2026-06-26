use super::prelude::*;

use super::types::AppState;

pub(crate) async fn serve(args: ServeArgs) -> Result<()> {
    let preliminary_source = resolve_source_root_arg(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .map(std::path::Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from(".")),
        args.workspace.as_deref(),
        &args.source_root,
    )?;
    let package_root = if args.toolchain_mode == "cargo" {
        resolve_cargo_package_root(preliminary_source.as_path())?
    } else {
        resolve_package_root()?
    };
    unsafe {
        std::env::set_var("MEI_TOOLCHAIN_MODE", args.toolchain_mode.as_str());
    }
    crate::agent_runtime::runtime::load_repo_dotenv(&package_root);
    let source_root = resolve_cli_source_root(
        &package_root,
        &resolve_source_root_arg(&package_root, args.workspace.as_deref(), &args.source_root)?,
    )?;
    fs::create_dir_all(&source_root).with_context(|| {
        format!(
            "failed to create or access source root {}",
            source_root.display()
        )
    })?;
    let source_root = source_root.canonicalize().with_context(|| {
        format!(
            "failed to canonicalize source root {}",
            source_root.display()
        )
    })?;
    let host_surface = args.host_surface.trim().to_ascii_lowercase();
    if host_surface == "access-only" {
        unsafe {
            std::env::set_var("MEI_HOST_SURFACE", "access-only");
        }
    } else {
        unsafe {
            std::env::remove_var("MEI_HOST_SURFACE");
        }
    }
    let host_surface_slug = if host_surface == "access-only" {
        HostSurface::AccessOnlyHost.as_slug()
    } else {
        HostSurface::AuthoringHost.as_slug()
    };
    let auth_enforcement = if args.auth {
        crate::auth::AuthEnforcement::Required
    } else {
        crate::auth::AuthEnforcement::Disabled
    };
    crate::auth::prepare_auth_for_serve(source_root.as_path(), auth_enforcement)?;
    if let Err(error) = mei_lang_toolchain::ensure_workspace_stock_materialized(
        source_root.as_path(),
        package_root.as_path(),
    ) {
        tracing::warn!(%error, "failed to ensure workspace stock before serve");
    }
    if !cfg!(test) {
        let probe = crate::http::pages::probe_landing_readiness(source_root.as_path());
        if probe.ready_app_id.is_none() {
            tracing::info!(
                app_count = probe.app_count,
                default_app = probe.configured_default_app.as_deref().unwrap_or("(none)"),
                "host landing probe: no default-scope ready app; serving /host shell (strict gate: MEI_HOST_LANDING_GATE=strict)"
            );
        }
        crate::http::pages::prepare_landing_artifacts_for_serve(source_root.as_path())?;
    }
    let preferred_mode = if args.auto_agent {
        "managed".to_string()
    } else {
        crate::agent_runtime::runtime::preferred_agent_mode()
    };
    let preferred_server_url = crate::agent_runtime::runtime::preferred_agent_server_url();
    let auto_agent = args.auto_agent;
    let _sync_agent_skill = args.sync_agent_skill || auto_agent;
    let native_agent = Arc::new(crate::mei_agent::NativeAgent::open_with_resource_tools(
        source_root.clone(),
        std::sync::Arc::new(crate::resource_tool_bridge::SceneResourceToolExecutor::default()),
    )?);
    let state = AppState {
        package_root: Arc::new(package_root.clone()),
        source_root: Arc::new(source_root.clone()),
        agent_preferred_mode: Arc::new(preferred_mode.clone()),
        agent_preferred_server_url: Arc::new(preferred_server_url.clone()),
        agent_auto_start: auto_agent,
        auth_enforcement,
        agent_runtime: Arc::new(Mutex::new(
            crate::agent_runtime::ManagedOpencodeRuntime::default(),
        )),
        agent_session_context: Arc::new(Mutex::new(HashMap::new())),
        native_agent,
    };
    tracing::debug!(
        cwd = ?std::env::current_dir(),
        manifest_dir = env!("CARGO_MANIFEST_DIR"),
        package_root = %package_root.display(),
        source_root = %source_root.display(),
        host_surface = host_surface_slug,
        auth = ?auth_enforcement,
        agent_backend = "native",
        "mei serve resolved paths"
    );
    match crate::agent_runtime::runtime::ensure_managed_agent_skill_for_root(
        package_root.as_path(),
        source_root.as_path(),
    ) {
        Ok(report) if report.installed_now => {
            tracing::info!(
                install_dir = %report.install_dir,
                file_count = report.file_count,
                "installed workspace-local MeiLang author skill from toolchain package"
            );
        }
        Ok(_) => {}
        Err(error) => tracing::warn!(
            %error,
            "failed to ensure workspace-local MeiLang author skill"
        ),
    }
    match crate::agent_runtime::runtime::managed_agent_skill_status(&state) {
        Ok(status) => {
            if status.installed {
                tracing::info!(
                    installed = status.installed,
                    file_count = status.file_count,
                    install_dir = %status.install_dir,
                    "using workspace-local MeiLang author skill"
                );
            } else {
                tracing::warn!(
                    install_dir = %status.install_dir,
                    "workspace-local MeiLang author skill is missing; run `mei-toolchain workspace runtime install --source-root <workspace>`"
                );
            }
        }
        Err(error) => tracing::warn!(%error, "failed to inspect workspace-local MeiLang skill"),
    }
    let app = Router::new()
        .merge(crate::http::router())
        .layer(middleware::from_fn_with_state(
            state.clone(),
            crate::auth::auth_middleware,
        ))
        .with_state(state)
        .layer(middleware::from_fn(super::request_logging::log_request));
    let addr: SocketAddr = format!("{}:{}", args.host, args.port).parse()?;
    let startup_policy = args.startup_policy.trim().to_ascii_lowercase();
    if startup_policy == "fail-fast-verify" {
        let verify_report = crate::http::host_api::verify_startup_artifacts(source_root.as_path())?;
        if !verify_report.ok {
            let summary = if verify_report.error_summary.is_empty() {
                "artifact verification failed before serve".to_string()
            } else {
                verify_report.error_summary.join("\n")
            };
            anyhow::bail!("host prebuild verify failed before bind:\n{summary}");
        }
    }
    crate::http::host_api::initialize_startup_readiness(
        source_root.as_path(),
        startup_policy.as_str(),
    );
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("serving MeiLang skeleton at http://{}", addr);
    crate::http::host_api::mark_host_bound();
    if startup_policy == "background-build" {
        if let Err(error) =
            crate::http::host_api::spawn_startup_build(source_root.as_path().to_path_buf())
        {
            tracing::warn!(%error, "failed to schedule startup background build");
        }
    }
    let source_root_for_preload = source_root.clone();
    if let Err(error) = tokio::task::spawn_blocking(move || {
        crate::http::host_api::preload_metric_response_indices_for_workspace(
            source_root_for_preload.as_path(),
        );
    })
    .await
    {
        tracing::warn!(%error, "metric response index preload worker join failed");
    }
    axum::serve(listener, app).await?;
    Ok(())
}
