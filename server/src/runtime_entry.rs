use std::{
    collections::HashMap,
    fs,
    net::SocketAddr,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::Instant,
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
use http_body_util::BodyExt;
use mei_lang_kernel::{set_mei_package_root, HostSurface};
use tracing::Instrument;

use crate::cli::args::{AgentRuntimeArgs, Cli, Command, HostCommand, ServeArgs};
use crate::cli::util::{
    print_cli_version_if_requested, resolve_cli_source_root, resolve_package_root,
    resolve_source_root_arg,
};
use crate::cli::{
    agent_command, compile_or_check_command, editor_runtime_command, export_command,
    host_command, inspect_command, knowledge_command, mcp_command, query_command,
    runtime_command, workspace_command,
};

static REQUEST_ID_SEQ: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryFlavor {
    Compat,
    Toolchain,
    HostWeb,
}

impl BinaryFlavor {
    fn display_name(self) -> &'static str {
        match self {
            BinaryFlavor::Compat => "mei",
            BinaryFlavor::Toolchain => "mei-toolchain",
            BinaryFlavor::HostWeb => "mei-host-web",
        }
    }
}

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) package_root: Arc<PathBuf>,
    pub(crate) source_root: Arc<PathBuf>,
    pub(crate) agent_preferred_mode: Arc<String>,
    pub(crate) agent_preferred_server_url: Arc<String>,
    pub(crate) agent_auto_start: bool,
    pub(crate) auth_enforcement: crate::auth::AuthEnforcement,
    pub(crate) agent_runtime: Arc<Mutex<crate::agent_runtime::ManagedOpencodeRuntime>>,
    pub(crate) agent_session_context: Arc<Mutex<HashMap<String, SessionContextSnapshot>>>,
    pub(crate) native_agent: Arc<crate::mei_agent::NativeAgent>,
}

#[derive(Clone)]
pub(crate) struct SessionContextSnapshot {
    pub signature: String,
    pub context: String,
}

fn ensure_command_allowed(flavor: BinaryFlavor, command: &Command) -> Result<()> {
    if flavor == BinaryFlavor::Compat {
        let command_name = match command {
            Command::Serve(_) => "serve",
            Command::Agent(_) => "agent",
            Command::Host(args) => match &args.command {
                HostCommand::Describe(_) => "host describe",
                HostCommand::Auth(_) => "host auth",
            },
            Command::Workspace(_) => "workspace",
            Command::Knowledge(_) => "knowledge",
            Command::EditorRuntime(_) => "editor-runtime",
            Command::Compile(_) => "compile",
            Command::Check(_) => "check",
            Command::Inspect(_) => "inspect",
            Command::Export(_) => "export",
            Command::Query(_) => "query",
            Command::Runtime(_) => "runtime",
            Command::Mcp(_) => "mcp",
        };
        anyhow::bail!(
            "the `mei` compatibility entrypoint is retired; use `mei-toolchain` for `{}` or `mei-host-web` for host commands",
            command_name
        );
    }
    let allowed = match flavor {
        BinaryFlavor::Compat => false,
        BinaryFlavor::Toolchain => matches!(
            command,
            Command::Workspace(_)
                | Command::Knowledge(_)
                | Command::EditorRuntime(_)
                | Command::Compile(_)
                | Command::Check(_)
                | Command::Inspect(_)
                | Command::Export(_)
                | Command::Query(_)
                | Command::Runtime(_)
                | Command::Mcp(_)
        ),
        BinaryFlavor::HostWeb => matches!(
            command,
            Command::Serve(_) | Command::Agent(_) | Command::Host(_)
        ),
    };
    if allowed {
        return Ok(());
    }
    let hint = match flavor {
        BinaryFlavor::Compat => "mei-toolchain",
        BinaryFlavor::Toolchain => "mei-host-web",
        BinaryFlavor::HostWeb => "mei-toolchain",
    };
    let role = match flavor {
        BinaryFlavor::Compat => "retired compatibility entrypoint",
        BinaryFlavor::Toolchain => "toolchain-only entrypoint",
        BinaryFlavor::HostWeb => "host-web-only entrypoint",
    };
    let command_name = match command {
        Command::Serve(_) => "serve",
        Command::Agent(_) => "agent",
        Command::Host(args) => match &args.command {
            HostCommand::Describe(_) => "host describe",
            HostCommand::Auth(_) => "host auth",
        },
        Command::Workspace(_) => "workspace",
        Command::Knowledge(_) => "knowledge",
        Command::EditorRuntime(_) => "editor-runtime",
        Command::Compile(_) => "compile",
        Command::Check(_) => "check",
        Command::Inspect(_) => "inspect",
        Command::Export(_) => "export",
        Command::Query(_) => "query",
        Command::Runtime(_) => "runtime",
        Command::Mcp(_) => "mcp",
    };
    anyhow::bail!(
        "`{}` does not expose `{}` under the current split; use `{}`",
        role,
        command_name,
        hint
    )
}

pub async fn run_cli_for_flavor(flavor: BinaryFlavor) -> Result<()> {
    if print_cli_version_if_requested() {
        println!(
            "{} {} ({})",
            flavor.display_name(),
            crate::build_info::BUILD_VERSION,
            crate::build_info::BUILD_TARGET_TAG
        );
        return Ok(());
    }
    let cli = Cli::parse();
    ensure_command_allowed(flavor, &cli.command)?;
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
        Command::Knowledge(args) => knowledge_command(args),
        Command::EditorRuntime(args) => editor_runtime_command(args),
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
    let package_root = resolve_package_root()?;
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
    let host_state = state.clone();
    let app = Router::new()
        .merge(crate::http::router())
        .layer(middleware::from_fn_with_state(
            state.clone(),
            crate::auth::auth_middleware,
        ))
        .with_state(state)
        .layer(middleware::from_fn(log_request));
    let addr: SocketAddr = format!("{}:{}", args.host, args.port).parse()?;
    crate::http::host_api::schedule_startup_warmup(host_state);
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
            | "/api/host/ready"
            | "/api/host/heartbeat"
            | "/favicon.ico"
    ) || path.starts_with("/app-assets/")
        || path.starts_with("/workspace-components/")
        || path.starts_with("/gis/")
        || path.ends_with("/events")
        || path.contains("/messages")
}

fn next_request_id() -> String {
    let id = REQUEST_ID_SEQ.fetch_add(1, Ordering::Relaxed);
    format!("req-{id:08x}")
}

fn is_expected_auth_client_error(uri: &Uri, status: StatusCode) -> bool {
    if status != StatusCode::UNAUTHORIZED && status != StatusCode::FORBIDDEN {
        return false;
    }
    let path = uri.path();
    path.starts_with("/api/agent/") || path == "/api/auth/session"
}

async fn log_request(request: Request<Body>, next: Next) -> Response {
    let method = request.method().clone();
    let uri = request.uri().clone();
    let request_id = next_request_id();
    let request_bytes = crate::http::request_trace::request_content_length(request.headers());
    let (route_kind, app_id) = crate::http::request_trace::classify_route(&method, &uri);
    let span = tracing::info_span!(
        "http_request",
        request_id = %request_id,
        route_kind = %route_kind,
        app_id = %app_id,
        method = %method,
        uri = %uri
    );
    let started_at = Instant::now();
    let mut response = next.run(request).instrument(span).await;
    let status = response.status();
    let (parts, body) = response.into_parts();
    let body_bytes = match body.collect().await {
        Ok(buffer) => buffer.to_bytes(),
        Err(error) => {
            tracing::warn!(
                request_id = %request_id,
                error = %error,
                "failed to collect response body for request trace"
            );
            axum::body::Bytes::new()
        }
    };
    let response_bytes = body_bytes.len() as u64;
    response = Response::from_parts(parts, Body::from(body_bytes));
    let latency_ms = started_at.elapsed().as_millis();
    if let Ok(value) = HeaderValue::from_str(&request_id) {
        response
            .headers_mut()
            .insert(HeaderName::from_static("x-mei-request-id"), value);
    }

    crate::http::request_trace::record_request(
        &request_id,
        &method,
        &uri,
        &route_kind,
        &app_id,
        status,
        latency_ms,
        request_bytes,
        response_bytes,
    );

    if status.is_server_error() {
        tracing::error!(
            request_id = %request_id,
            route_kind = %route_kind,
            app_id = %app_id,
            status = %status,
            latency_ms,
            request_bytes,
            response_bytes,
            method = %method,
            uri = %uri,
            "request finished with error status"
        );
    } else if status.is_client_error() {
        if is_expected_auth_client_error(&uri, status) {
            tracing::debug!(
                request_id = %request_id,
                route_kind = %route_kind,
                app_id = %app_id,
                status = %status,
                latency_ms,
                request_bytes,
                response_bytes,
                method = %method,
                uri = %uri,
                "request finished with expected auth client error"
            );
        } else {
            tracing::warn!(
                request_id = %request_id,
                route_kind = %route_kind,
                app_id = %app_id,
                status = %status,
                latency_ms,
                request_bytes,
                response_bytes,
                method = %method,
                uri = %uri,
                "request finished with client error status"
            );
        }
    } else if !is_noisy_success_request(&method, &uri) {
        tracing::info!(
            request_id = %request_id,
            route_kind = %route_kind,
            app_id = %app_id,
            status = %status,
            latency_ms,
            request_bytes,
            response_bytes,
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::cli::args::{
        CheckArgs, CliAppSelectorArgs, Command, HostArgs, HostCommand, HostDescribeArgs,
        InspectArgs, InspectCommand, QueryArgs, QueryCommand, ServeArgs,
    };

    use super::{ensure_command_allowed, BinaryFlavor};

    fn app_selector() -> CliAppSelectorArgs {
        CliAppSelectorArgs {
            source_root: PathBuf::from("../workspaces/ws-dev"),
            app: "demo".into(),
            scene: None,
            target_file: None,
            json: false,
        }
    }

    #[test]
    fn toolchain_entry_allows_headless_commands() {
        let command = Command::Inspect(InspectArgs {
            command: InspectCommand::Summary(crate::cli::args::InspectSummaryArgs {
                app: app_selector(),
            }),
        });
        assert!(ensure_command_allowed(BinaryFlavor::Toolchain, &command).is_ok());
    }

    #[test]
    fn toolchain_entry_rejects_host_commands() {
        let command = Command::Serve(ServeArgs {
            workspace: None,
            source_root: PathBuf::from("../workspaces/ws-dev"),
            host_surface: "full".into(),
            auth: false,
            host: "127.0.0.1".into(),
            port: 3000,
            auto_agent: false,
            sync_agent_skill: false,
        });
        assert!(ensure_command_allowed(BinaryFlavor::Toolchain, &command).is_err());
    }

    #[test]
    fn host_web_entry_allows_host_commands() {
        let command = Command::Host(HostArgs {
            command: HostCommand::Describe(HostDescribeArgs { json: true }),
        });
        assert!(ensure_command_allowed(BinaryFlavor::HostWeb, &command).is_ok());
    }

    #[test]
    fn compat_entry_rejects_all_commands() {
        let command = Command::Check(CheckArgs {
            app: app_selector(),
        });
        assert!(ensure_command_allowed(BinaryFlavor::Compat, &command).is_err());
    }

    #[test]
    fn host_web_entry_rejects_toolchain_commands() {
        let command = Command::Query(QueryArgs {
            command: QueryCommand::Resource(crate::cli::args::QueryResourceArgs {
                app: app_selector(),
                id: "orders".into(),
            }),
        });
        assert!(ensure_command_allowed(BinaryFlavor::HostWeb, &command).is_err());
    }
}
