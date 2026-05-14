use std::{
    collections::HashMap,
    net::SocketAddr,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::{Context, Result};
use axum::{
    body::Body,
    http::StatusCode,
    http::{Method, Request, Uri},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    Router,
};
use clap::{Parser, Subcommand};
use reqwest::Client as HttpClient;
use std::time::Instant;

mod http;
mod opencode;

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
    Opencode(OpencodeArgs),
}

#[derive(clap::Args)]
struct ServeArgs {
    #[arg(long, default_value = "examples")]
    source_root: PathBuf,
    #[arg(long, default_value = "127.0.0.1")]
    host: String,
    #[arg(long, default_value_t = 3000)]
    port: u16,
    /// 显式允许在 mei 启动时自动拉起托管的 OpenCode（默认关闭，优先使用外部服务）
    #[arg(long)]
    auto_opencode: bool,
    /// 兼容旧参数；当前默认已不自动拉起托管 OpenCode
    #[arg(long, hide = true)]
    no_auto_opencode: bool,
}

#[derive(clap::Args)]
struct OpencodeArgs {
    #[command(subcommand)]
    command: OpencodeCommand,
}

#[derive(Subcommand)]
enum OpencodeCommand {
    Skill(OpencodeSkillArgs),
}

#[derive(clap::Args)]
struct OpencodeSkillArgs {
    #[command(subcommand)]
    command: OpencodeSkillCommand,
}

#[derive(Subcommand)]
enum OpencodeSkillCommand {
    /// 查看当前 MeiLang skill 安装与同步状态
    Status,
    /// 手动同步 MeiLang skill 到运行时目录
    Sync,
}

#[derive(Clone)]
pub(crate) struct AppState {
    package_root: Arc<PathBuf>,
    source_root: Arc<PathBuf>,
    opencode_preferred_mode: Arc<String>,
    opencode_preferred_server_url: Arc<String>,
    opencode_auto_start: bool,
    opencode_runtime: Arc<Mutex<opencode::ManagedOpencodeRuntime>>,
    opencode_session_context: Arc<Mutex<HashMap<String, SessionContextSnapshot>>>,
    opencode_http: Arc<HttpClient>,
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
        Command::Opencode(args) => opencode_command(args),
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
    opencode::runtime::load_repo_dotenv(&package_root);
    let source_root = if args.source_root.is_absolute() {
        args.source_root
    } else {
        package_root.join(args.source_root)
    };
    let preferred_mode = if args.auto_opencode {
        "managed".to_string()
    } else {
        opencode::runtime::preferred_opencode_mode()
    };
    let preferred_server_url = opencode::runtime::preferred_opencode_server_url();
    let auto_opencode = args.auto_opencode && !args.no_auto_opencode;
    let state = AppState {
        package_root: Arc::new(package_root.clone()),
        source_root: Arc::new(source_root.clone()),
        opencode_preferred_mode: Arc::new(preferred_mode.clone()),
        opencode_preferred_server_url: Arc::new(preferred_server_url.clone()),
        opencode_auto_start: auto_opencode,
        opencode_runtime: Arc::new(Mutex::new(opencode::ManagedOpencodeRuntime::default())),
        opencode_session_context: Arc::new(Mutex::new(HashMap::new())),
        opencode_http: Arc::new(HttpClient::new()),
    };
    tracing::info!(
        cwd = ?std::env::current_dir(),
        manifest_dir = env!("CARGO_MANIFEST_DIR"),
        package_root = %package_root.display(),
        source_root = %source_root.display(),
        opencode_mode = %preferred_mode,
        opencode_server_url = %preferred_server_url,
        opencode_auto_start = auto_opencode,
        "mei serve resolved paths"
    );
    match opencode::runtime::ensure_managed_opencode_skill_synced(&state) {
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
    let boot_state = state.clone();
    tokio::spawn(async move {
        if !boot_state.opencode_auto_start {
            tracing::info!("skip auto-start managed opencode: auto-start is disabled");
            return;
        }
        if boot_state.opencode_preferred_mode.as_str() != "managed" {
            tracing::info!(
                mode = %boot_state.opencode_preferred_mode,
                "skip auto-start managed opencode: preferred mode is not managed"
            );
            return;
        }
        tokio::time::sleep(Duration::from_millis(300)).await;
        let summary = opencode::runtime::managed_opencode_config_summary(&boot_state);
        if !summary.runtime_env_ready || !summary.config_content_ready {
            tracing::info!(
                missing_env = ?summary.missing_env,
                "skip auto-start managed opencode: runtime env or model config not ready"
            );
            return;
        }
        let request = opencode::StartManagedOpencodeRequest {
            host: None,
            port: Some(4099),
        };
        match opencode::runtime::start_managed_opencode(&boot_state, request).await {
            Ok(_) => tracing::info!("auto-started managed OpenCode (hosted) on mei-lang boot"),
            Err(error) => tracing::warn!(
                %error,
                "auto-start managed OpenCode failed; use panel 重连 or check port 4099 / API keys"
            ),
        }
    });
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

fn opencode_command(args: OpencodeArgs) -> Result<()> {
    let package_root = resolve_package_root()?;
    opencode::runtime::load_repo_dotenv(&package_root);
    match args.command {
        OpencodeCommand::Skill(skill_args) => match skill_args.command {
            OpencodeSkillCommand::Status => {
                let status = opencode::runtime::managed_opencode_skill_status_for_root(&package_root);
                println!("{}", serde_json::to_string_pretty(&status)?);
            }
            OpencodeSkillCommand::Sync => {
                let status = opencode::runtime::sync_managed_opencode_skill_for_root(&package_root)?;
                println!("{}", serde_json::to_string_pretty(&status)?);
            }
        },
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
        "/api/opencode/config"
            | "/api/opencode/runtime"
            | "/api/opencode/skill"
            | "/api/opencode/health"
            | "/api/opencode/session"
            | "/favicon.ico"
    ) || path.starts_with("/app-assets/")
        || path.starts_with("/workspace-components/")
        || path.ends_with("/events")
        || path.contains("/messages")
}

async fn log_request(request: Request<Body>, next: Next) -> Response {
    let method = request.method().clone();
    let uri = request.uri().clone();
    let started_at = Instant::now();
    let response = next.run(request).await;
    let status = response.status();
    let latency_ms = started_at.elapsed().as_millis();

    if status.is_server_error() || status.is_client_error() {
        tracing::error!(
            status = %status,
            latency_ms,
            method = %method,
            uri = %uri,
            "request finished with error status"
        );
    } else if !is_noisy_success_request(&method, &uri) {
        tracing::info!(
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
        tracing::error!(status = %self.status, error = %self.message, "request failed");
        (self.status, self.message).into_response()
    }
}
