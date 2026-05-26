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
use clap::{Parser, Subcommand};
use mei_lang_kernel::{CompileWatchedFile, CompiledApp};
use std::time::Instant;
use tracing::Instrument;

mod agent_runtime;
mod gis_config;
mod http;
mod mei_agent;
mod resource_tool_bridge;

static REQUEST_ID_SEQ: AtomicU64 = AtomicU64::new(1);

#[derive(Parser)]
#[command(name = "mei")]
#[command(about = "MeiLang skeleton server", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Serve(ServeArgs),
    Agent(AgentArgs),
}

#[derive(clap::Args)]
struct AgentArgs {
    #[command(subcommand)]
    command: AgentCommand,
}

#[derive(clap::Args)]
struct ServeArgs {
    #[arg(long, default_value = "../workspaces")]
    source_root: PathBuf,
    #[arg(long, default_value = "127.0.0.1")]
    host: String,
    #[arg(long, default_value_t = 3000)]
    port: u16,
    /// 显式允许在 mei 启动时自动拉起托管的内置 Agent 运行时（默认关闭）
    #[arg(long)]
    auto_agent: bool,
}

#[derive(clap::Args)]
struct AgentRuntimeArgs {
    #[command(subcommand)]
    command: AgentCommand,
}

#[derive(clap::Args)]
struct AgentSkillArgs {
    #[arg(long, default_value = "../workspaces")]
    source_root: PathBuf,
    #[command(subcommand)]
    command: AgentSkillCommand,
}

#[derive(Subcommand)]
enum AgentSkillCommand {
    /// 查看当前 MeiLang skill 安装与同步状态
    Status,
    /// 手动同步 MeiLang skill 到运行时目录
    Sync,
}

#[derive(Subcommand)]
enum AgentCommand {
    Skill(AgentSkillArgs),
}

#[derive(Clone)]
pub(crate) struct AppState {
    package_root: Arc<PathBuf>,
    source_root: Arc<PathBuf>,
    agent_preferred_mode: Arc<String>,
    agent_preferred_server_url: Arc<String>,
    agent_auto_start: bool,
    agent_runtime: Arc<Mutex<agent_runtime::ManagedOpencodeRuntime>>,
    agent_session_context: Arc<Mutex<HashMap<String, SessionContextSnapshot>>>,
    compile_cache: Arc<Mutex<HashMap<String, CachedCompiledApp>>>,
    pub(crate) native_agent: Arc<mei_agent::NativeAgent>,
    pub(crate) gis_tiles: Arc<gis_config::GisTilesConfig>,
}

#[derive(Clone)]
pub(crate) struct CachedCompiledApp {
    pub coarse_revision: u128,
    pub compile_revision: String,
    pub watched_files: Vec<CompileWatchedFile>,
    pub components_revision: u128,
    pub compiled: CompiledApp,
}

#[derive(Clone)]
pub(crate) struct SessionContextSnapshot {
    pub signature: String,
    pub context: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .with_target(false)
        .compact()
        .init();

    let cli = Cli::parse();
    match cli.command {
        Command::Serve(args) => serve(args).await,
        Command::Agent(args) => agent_command(AgentRuntimeArgs {
            command: args.command,
        }),
    }
}

fn resolve_package_root() -> Result<PathBuf> {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .context("server crate manifest has no parent directory")
        .map(std::path::Path::to_path_buf)
}

async fn serve(args: ServeArgs) -> Result<()> {
    // 二进制可能在任意 cwd 下启动；不要用 current_dir 推导源码与静态资源路径。
    // `mei-lang-server` 位于 `mei-lang/server/`，仓库根为上一级 `mei-lang/`。
    let package_root = resolve_package_root()?;
    agent_runtime::runtime::load_repo_dotenv(&package_root);
    let source_root = if args.source_root.is_absolute() {
        args.source_root
    } else {
        package_root.join(args.source_root)
    };
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
    let preferred_mode = if args.auto_agent {
        "managed".to_string()
    } else {
        agent_runtime::runtime::preferred_agent_mode()
    };
    let preferred_server_url = agent_runtime::runtime::preferred_agent_server_url();
    let auto_agent = args.auto_agent;
    let native_agent = Arc::new(mei_agent::NativeAgent::open_with_resource_tools(
        source_root.clone(),
        package_root.clone(),
        std::sync::Arc::new(resource_tool_bridge::SceneResourceToolExecutor::default()),
    )?);
    let gis_tiles = Arc::new(gis_config::GisTilesConfig::resolve());
    tracing::info!(
        tiles_base_url = %gis_tiles.base_url,
        tiles_json_path = %gis_tiles.json_path,
        tilejson_url = %gis_tiles.tilejson_url(),
        "GIS basemap (Martin) — start tiles separately; see mei-projects/scripts/start_martin_docker.sh"
    );
    let state = AppState {
        package_root: Arc::new(package_root.clone()),
        source_root: Arc::new(source_root.clone()),
        agent_preferred_mode: Arc::new(preferred_mode.clone()),
        agent_preferred_server_url: Arc::new(preferred_server_url.clone()),
        agent_auto_start: auto_agent,
        agent_runtime: Arc::new(Mutex::new(agent_runtime::ManagedOpencodeRuntime::default())),
        agent_session_context: Arc::new(Mutex::new(HashMap::new())),
        compile_cache: Arc::new(Mutex::new(HashMap::new())),
        native_agent,
        gis_tiles: gis_tiles.clone(),
    };
    tracing::info!(
        cwd = ?std::env::current_dir(),
        manifest_dir = env!("CARGO_MANIFEST_DIR"),
        package_root = %package_root.display(),
        source_root = %source_root.display(),
        agent_mode = %preferred_mode,
        agent_server_url = %preferred_server_url,
        agent_auto_start = auto_agent,
        agent_backend = "native",
        "mei serve resolved paths"
    );
    match agent_runtime::runtime::ensure_managed_agent_skill_synced(&state) {
        Ok(status) => {
            if status.source_present {
                tracing::info!(
                    installed = status.installed,
                    stale = status.stale,
                    file_count = status.file_count,
                    install_dir = %status.install_dir,
                    "ensured MeiLang skill is synced on startup"
                );
            } else {
                tracing::warn!(
                    source_dir = %status.source_dir,
                    "MeiLang skill source directory is missing on startup"
                );
            }
        }
        Err(error) => tracing::warn!(%error, "failed to auto-sync MeiLang skill on startup"),
    }
    let app = Router::new()
        .merge(http::router())
        .with_state(state)
        .layer(middleware::from_fn(log_request));
    let addr: SocketAddr = format!("{}:{}", args.host, args.port).parse()?;
    tracing::info!("serving MeiLang skeleton at http://{}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

fn agent_command(args: AgentRuntimeArgs) -> Result<()> {
    let package_root = resolve_package_root()?;
    agent_runtime::runtime::load_repo_dotenv(&package_root);
    match args.command {
        AgentCommand::Skill(skill_args) => {
            let AgentSkillArgs {
                source_root,
                command,
            } = skill_args;
            let source_root = if source_root.is_absolute() {
                source_root
            } else {
                package_root.join(source_root)
            };
            match command {
                AgentSkillCommand::Status => {
                    let status = agent_runtime::runtime::managed_agent_skill_status_for_root(
                        &package_root,
                        &source_root,
                    );
                    println!("{}", serde_json::to_string_pretty(&status)?);
                }
                AgentSkillCommand::Sync => {
                    let status = agent_runtime::runtime::sync_managed_agent_skill_for_root(
                        &package_root,
                        &source_root,
                    )?;
                    println!("{}", serde_json::to_string_pretty(&status)?);
                }
            }
        }
    }
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
    if *method == Method::GET && path.starts_with("/apps/manage/") {
        return ("manage_page", app_tail("/apps/manage/"));
    }
    if *method == Method::GET && path.starts_with("/apps/access/") {
        return ("access_page", app_tail("/apps/access/"));
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

/// 集成测试与 HTTP 级用例构造 `AppState`（依赖仓库内 `mei-lang/../workspaces`）。
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
            .join("../workspaces")
            .canonicalize()
            .context("workspaces root (mei-lang/../workspaces)")?;
        let native_agent = Arc::new(crate::mei_agent::NativeAgent::open_with_resource_tools(
            source_root.clone(),
            package_root.clone(),
            Arc::new(crate::resource_tool_bridge::SceneResourceToolExecutor::default()),
        )?);
        Ok(super::AppState {
            package_root: Arc::new(package_root),
            source_root: Arc::new(source_root),
            agent_preferred_mode: Arc::new("native".into()),
            agent_preferred_server_url: Arc::new(String::new()),
            agent_auto_start: false,
            agent_runtime: Arc::new(Mutex::new(
                crate::agent_runtime::ManagedOpencodeRuntime::default(),
            )),
            agent_session_context: Arc::new(Mutex::new(HashMap::new())),
            compile_cache: Arc::new(Mutex::new(HashMap::new())),
            native_agent,
            gis_tiles: Arc::new(super::gis_config::GisTilesConfig::resolve()),
        })
    }
}
