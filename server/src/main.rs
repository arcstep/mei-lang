use std::{
    collections::HashMap,
    fs,
    net::SocketAddr,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
};

use anyhow::{Context, Result};
use axum::{
    body::Body,
    http::StatusCode,
    http::{HeaderName, HeaderValue, Method, Request, Uri},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    Router,
};
use clap::Parser;
use mei_lang_kernel::{set_mei_package_root, HostSurface};
use std::time::Instant;
use tracing::Instrument;

mod agent_runtime;
mod auth;
mod cli;
mod build_info;
mod gis_config;
mod http;
mod mei_agent;
mod resource_tool_bridge;

static REQUEST_ID_SEQ: AtomicU64 = AtomicU64::new(1);

use cli::{
    agent_command, compile_or_check_command, export_command, host_command, inspect_command,
    mcp_command, query_command, runtime_command, workspace_command,
};
use cli::args::{AgentRuntimeArgs, Cli, Command, ServeArgs};
use cli::util::{
    print_cli_version_if_requested, resolve_cli_source_root, resolve_package_root,
    resolve_source_root_arg,
};

#[derive(Clone)]
pub(crate) struct AppState {
    package_root: Arc<PathBuf>,
    source_root: Arc<PathBuf>,
    agent_preferred_mode: Arc<String>,
    agent_preferred_server_url: Arc<String>,
    agent_auto_start: bool,
    pub(crate) auth_enforcement: auth::AuthEnforcement,
    agent_runtime: Arc<Mutex<agent_runtime::ManagedOpencodeRuntime>>,
    agent_session_context: Arc<Mutex<HashMap<String, SessionContextSnapshot>>>,
    pub(crate) native_agent: Arc<mei_agent::NativeAgent>,
}

#[derive(Clone)]
pub(crate) struct SessionContextSnapshot {
    pub signature: String,
    pub context: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    if print_cli_version_if_requested() {
        println!(
            "mei {} ({})",
            build_info::BUILD_VERSION,
            build_info::BUILD_TARGET_TAG
        );
        return Ok(());
    }
    let cli = Cli::parse();
    let package_root = resolve_package_root()?;
    set_mei_package_root(package_root.clone());
    let env_filter = match cli.command {
        Command::Serve(_) => "info",
        _ => "error",
    };
    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(false)
        .compact()
        .init();
    match cli.command {
        Command::Serve(args) => serve(args).await,
        Command::Agent(args) => agent_command(AgentRuntimeArgs {
            command: args.command,
        }),
        Command::Host(args) => host_command(args),
        Command::Workspace(args) => workspace_command(args),
        Command::Compile(args) => compile_or_check_command("compile", args),
        Command::Check(args) => compile_or_check_command("check", args),
        Command::Inspect(args) => inspect_command(args),
        Command::Export(args) => export_command(args),
        Command::Query(args) => query_command(args),
        Command::Runtime(args) => runtime_command(args),
        Command::Mcp(args) => mcp_command(args),
    }
}

async fn serve(args: ServeArgs) -> Result<()> {
    // 二进制可能在任意 cwd 下启动；不要用 current_dir 推导源码与静态资源路径。
    // `mei-lang-server` 位于 `mei-lang/server/`，仓库根为上一级 `mei-lang/`。
    let package_root = resolve_package_root()?;
    agent_runtime::runtime::load_repo_dotenv(&package_root);
    let source_root = resolve_cli_source_root(
        &package_root,
        &resolve_source_root_arg(
            &package_root,
            args.workspace.as_deref(),
            &args.source_root,
        )?,
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
        auth::AuthEnforcement::Required
    } else {
        auth::AuthEnforcement::Disabled
    };
    auth::prepare_auth_for_serve(source_root.as_path(), auth_enforcement)?;
    let preferred_mode = if args.auto_agent {
        "managed".to_string()
    } else {
        agent_runtime::runtime::preferred_agent_mode()
    };
    let preferred_server_url = agent_runtime::runtime::preferred_agent_server_url();
    let auto_agent = args.auto_agent;
    let sync_agent_skill = args.sync_agent_skill || auto_agent;
    let native_agent = Arc::new(mei_agent::NativeAgent::open_with_resource_tools(
        source_root.clone(),
        std::sync::Arc::new(resource_tool_bridge::SceneResourceToolExecutor::default()),
    )?);
    let state = AppState {
        package_root: Arc::new(package_root.clone()),
        source_root: Arc::new(source_root.clone()),
        agent_preferred_mode: Arc::new(preferred_mode.clone()),
        agent_preferred_server_url: Arc::new(preferred_server_url.clone()),
        agent_auto_start: auto_agent,
        auth_enforcement,
        agent_runtime: Arc::new(Mutex::new(agent_runtime::ManagedOpencodeRuntime::default())),
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
    if sync_agent_skill {
        match agent_runtime::runtime::ensure_managed_agent_skill_synced(&state) {
            Ok(status) => {
                if status.source_present {
                    tracing::info!(
                        installed = status.installed,
                        stale = status.stale,
                        file_count = status.file_count,
                        install_dir = %status.install_dir,
                        "synced MeiLang skill on startup"
                    );
                } else {
                    tracing::warn!(
                        source_dir = %status.source_dir,
                        "MeiLang skill source directory is missing on startup"
                    );
                }
            }
            Err(error) => tracing::warn!(%error, "failed to sync MeiLang skill on startup"),
        }
    } else {
        tracing::info!(
            "skipped MeiLang skill sync on startup (pass --sync-agent-skill or --auto-agent to enable)"
        );
    }
    let app = Router::new()
        .merge(http::router())
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::auth_middleware,
        ))
        .with_state(state)
        .layer(middleware::from_fn(log_request));
    let addr: SocketAddr = format!("{}:{}", args.host, args.port).parse()?;
    tracing::info!("serving MeiLang skeleton at http://{}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

fn is_noisy_success_request(method: &Method, uri: &Uri) -> bool {
    if *method != Method::GET {
        return false;
    }
    let path = uri.path();
    matches!(
        path,
        "/api/agent/config"
            | "/api/agent/runtime"
            | "/api/agent/skill"
            | "/api/agent/health"
            | "/api/agent/session"
            | "/favicon.ico"
    ) || path.starts_with("/app-assets/")
        || path.starts_with("/workspace-components/")
        || path.ends_with("/events")
        || path.contains("/messages")
}

fn next_request_id() -> String {
    let id = REQUEST_ID_SEQ.fetch_add(1, Ordering::Relaxed);
    format!("req-{id:08x}")
}

fn route_kind_and_app_id(method: &Method, uri: &Uri) -> (&'static str, String) {
    let path = uri.path();
    let app_tail = |prefix: &str| {
        path.strip_prefix(prefix)
            .unwrap_or_default()
            .trim_start_matches('/')
            .to_string()
    };
    if *method == Method::GET && path.starts_with("/apps/build/") {
        return ("build_page", app_tail("/apps/build/"));
    }
    if *method == Method::GET && path.starts_with("/apps/app/") {
        return ("app_page", app_tail("/apps/app/"));
    }
    if *method == Method::GET && path.starts_with("/apps/config/") {
        return ("config_page", app_tail("/apps/config/"));
    }
    if *method == Method::GET && path.starts_with("/apps/upload/") {
        return ("upload_page", app_tail("/apps/upload/"));
    }
    if *method == Method::GET && path.starts_with("/apps/manage/") {
        return ("manage_page_legacy", app_tail("/apps/manage/"));
    }
    if *method == Method::GET && path.starts_with("/apps/access/") {
        return ("access_page_legacy", app_tail("/apps/access/"));
    }
    if *method == Method::POST && path.starts_with("/api/datasets/query/") {
        return ("dataset_query", app_tail("/api/datasets/query/"));
    }
    if *method == Method::POST && path.starts_with("/api/datasets/metrics/") {
        return ("metric_query", app_tail("/api/datasets/metrics/"));
    }
    ("http_request", String::new())
}

async fn log_request(request: Request<Body>, next: Next) -> Response {
    let method = request.method().clone();
    let uri = request.uri().clone();
    let request_id = next_request_id();
    let (route_kind, app_id) = route_kind_and_app_id(&method, &uri);
    let span = tracing::info_span!(
        "http_request",
        request_id = %request_id,
        route_kind = route_kind,
        app_id = %app_id,
        method = %method,
        uri = %uri
    );
    let started_at = Instant::now();
    let mut response = next.run(request).instrument(span).await;
    let status = response.status();
    let latency_ms = started_at.elapsed().as_millis();
    if let Ok(value) = HeaderValue::from_str(&request_id) {
        response
            .headers_mut()
            .insert(HeaderName::from_static("x-mei-request-id"), value);
    }

    if status.is_server_error() || status.is_client_error() {
        tracing::error!(
            request_id = %request_id,
            route_kind = route_kind,
            app_id = %app_id,
            status = %status,
            latency_ms,
            method = %method,
            uri = %uri,
            "request finished with error status"
        );
    } else if !is_noisy_success_request(&method, &uri) {
        tracing::info!(
            request_id = %request_id,
            route_kind = route_kind,
            app_id = %app_id,
            status = %status,
            latency_ms,
            method = %method,
            uri = %uri,
            "request finished"
        );
    }

    response
}

#[derive(Debug)]
pub(crate) struct AppError {
    status: StatusCode,
    message: String,
}

impl AppError {
    pub(crate) fn msg(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
        }
    }

    pub(crate) fn status(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }
}

impl From<anyhow::Error> for AppError {
    fn from(value: anyhow::Error) -> Self {
        Self::msg(value.to_string())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        if self.status.is_server_error() {
            tracing::error!(status = %self.status, error = %self.message, "request failed");
        } else {
            tracing::warn!(status = %self.status, error = %self.message, "request failed");
        }
        (self.status, self.message).into_response()
    }
}

/// 集成测试与 HTTP 级用例构造 `AppState`（依赖仓库内 `mei-lang/../ws-dev`）。
#[cfg(test)]
pub(crate) mod test_support {
    use std::{
        collections::HashMap,
        path::PathBuf,
        sync::{Arc, Mutex},
    };

    use anyhow::Context;

    pub(crate) fn package_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("server crate parent (mei-lang/)")
            .to_path_buf()
    }

    pub(crate) fn test_app_state() -> anyhow::Result<super::AppState> {
        let package_root = package_root();
        let source_root = package_root
            .join("../workspaces/ws-dev")
            .canonicalize()
            .context("workspace root (mei-lang/../workspaces/ws-dev)")?;
        let native_agent = Arc::new(crate::mei_agent::NativeAgent::open_with_resource_tools(
            source_root.clone(),
            Arc::new(crate::resource_tool_bridge::SceneResourceToolExecutor::default()),
        )?);
        Ok(super::AppState {
            package_root: Arc::new(package_root),
            source_root: Arc::new(source_root),
            agent_preferred_mode: Arc::new("native".into()),
            agent_preferred_server_url: Arc::new(String::new()),
            agent_auto_start: false,
            auth_enforcement: crate::auth::AuthEnforcement::Disabled,
            agent_runtime: Arc::new(Mutex::new(
                crate::agent_runtime::ManagedOpencodeRuntime::default(),
            )),
            agent_session_context: Arc::new(Mutex::new(HashMap::new())),
            native_agent,
        })
    }
}
