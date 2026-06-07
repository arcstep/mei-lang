use std::{
    collections::{BTreeMap, HashMap},
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
use clap::{Args, Parser, Subcommand};
use mei_lang_kernel::{
    host_runtime_capabilities_catalog, host_runtime_contract_descriptor, HostSurface,
    CompileOptions, CompileWatchedFile, Diagnostic, Severity,
};
use mei_lang_toolchain::{self as toolchain, HeadlessExportOptions};
use serde::Serialize;
use serde_json::json;
use std::time::Instant;
use tracing::Instrument;

mod agent_runtime;
mod auth;
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
    Host(HostArgs),
    Compile(CheckArgs),
    Check(CheckArgs),
    Inspect(InspectArgs),
    Export(ExportArgs),
    Query(QueryArgs),
    Runtime(RuntimeArgs),
    Mcp(McpArgs),
}

#[derive(clap::Args)]
struct AgentArgs {
    #[command(subcommand)]
    command: AgentCommand,
}

#[derive(Args, Clone)]
struct HostArgs {
    #[command(subcommand)]
    command: HostCommand,
}

#[derive(Subcommand, Clone)]
enum HostCommand {
    Describe(HostDescribeArgs),
    Auth(HostAuthArgs),
}

#[derive(Args, Clone)]
struct HostDescribeArgs {
    #[arg(long)]
    json: bool,
}

#[derive(Args, Clone)]
struct HostAuthArgs {
    #[command(subcommand)]
    command: HostAuthCommand,
}

#[derive(Subcommand, Clone)]
enum HostAuthCommand {
    /// 生成 JWT 密钥与登录 RSA 密钥对（写入工作区 `.mei-workspace.json`，不涉及用户密码）
    EnsureKeys(HostAuthEnsureKeysArgs),
    /// 一次性初始化 super/admin/guest 用户并生成临时密码（不使用固定默认密码）
    BootstrapUsers(HostAuthBootstrapUsersArgs),
    /// 新增或更新单个用户；密码通过 stdin 传入（禁止命令行明文密码）
    AddUser(HostAuthAddUserArgs),
    /// 禁用用户（写入 `disabled=true`）
    DisableUser(HostAuthSetUserEnabledArgs),
    /// 启用用户（写入 `disabled=false`）
    EnableUser(HostAuthSetUserEnabledArgs),
    RotateKeys(HostAuthRotateKeysArgs),
    /// 从标准输入读取密码并输出 Argon2 哈希（供写入配置 `passwordHash`，禁止在命令行传明文密码）
    HashPassword(HostAuthHashPasswordArgs),
    Describe(HostAuthDescribeArgs),
}

#[derive(Args, Clone)]
struct HostAuthEnsureKeysArgs {
    #[arg(long, default_value = "../workspaces")]
    source_root: PathBuf,
    #[arg(long)]
    json: bool,
}

#[derive(Args, Clone)]
struct HostAuthRotateKeysArgs {
    #[arg(long, default_value = "../workspaces")]
    source_root: PathBuf,
    #[arg(long)]
    json: bool,
}

#[derive(Args, Clone)]
struct HostAuthBootstrapUsersArgs {
    #[arg(long, default_value = "../workspaces")]
    source_root: PathBuf,
    #[arg(long, default_value = "super")]
    super_username: String,
    #[arg(long, default_value = "超级管理员")]
    super_profile: String,
    #[arg(long, default_value = "admin")]
    admin_username: String,
    #[arg(long, default_value = "管理员")]
    admin_profile: String,
    #[arg(long, default_value = "guest")]
    guest_username: String,
    #[arg(long, default_value = "访客")]
    guest_profile: String,
    #[arg(long = "guest-app-allow")]
    guest_app_allow: Vec<String>,
    #[arg(long = "guest-scene-allow", help = "格式: app_id:scene_id")]
    guest_scene_allow: Vec<String>,
    /// 从 stdin 读取统一初始密码（super/admin/guest 共用）；未指定时为各账号随机生成。
    #[arg(long)]
    default_password_stdin: bool,
    #[arg(long)]
    json: bool,
}

#[derive(Args, Clone)]
struct HostAuthAddUserArgs {
    #[arg(long, default_value = "../workspaces")]
    source_root: PathBuf,
    #[arg(long)]
    username: String,
    #[arg(long, default_value = "guest", value_parser = ["super", "admin", "guest"])]
    role: String,
    #[arg(long, default_value = "")]
    profile: String,
    #[arg(long = "app-allow")]
    app_allow: Vec<String>,
    #[arg(long = "scene-allow", help = "格式: app_id:scene_id")]
    scene_allow: Vec<String>,
    /// 必须显式声明从 stdin 读取密码，避免误将明文放进命令行参数。
    #[arg(long)]
    password_stdin: bool,
    #[arg(long)]
    json: bool,
}

#[derive(Args, Clone)]
struct HostAuthSetUserEnabledArgs {
    #[arg(long, default_value = "../workspaces")]
    source_root: PathBuf,
    #[arg(long)]
    username: String,
    #[arg(long)]
    json: bool,
}

#[derive(Args, Clone)]
struct HostAuthHashPasswordArgs {
    #[arg(long)]
    json: bool,
}

#[derive(Args, Clone)]
struct HostAuthDescribeArgs {
    #[arg(long, default_value = "../workspaces")]
    source_root: PathBuf,
    #[arg(long)]
    json: bool,
}

#[derive(Args, Clone)]
struct CliAppSelectorArgs {
    #[arg(long, default_value = "../workspaces")]
    source_root: PathBuf,
    #[arg(long)]
    app: String,
    #[arg(long)]
    scene: Option<String>,
    #[arg(long, alias = "target")]
    target_file: Option<String>,
    #[arg(long)]
    json: bool,
}

#[derive(Args, Clone)]
struct CheckArgs {
    #[command(flatten)]
    app: CliAppSelectorArgs,
}

#[derive(Args, Clone)]
struct InspectArgs {
    #[command(subcommand)]
    command: InspectCommand,
}

#[derive(Subcommand, Clone)]
enum InspectCommand {
    World(InspectWorldArgs),
    Inventory(InspectInventoryArgs),
    Layout(InspectLayoutArgs),
}

#[derive(Args, Clone)]
struct InspectWorldArgs {
    #[command(flatten)]
    app: CliAppSelectorArgs,
}

#[derive(Args, Clone)]
struct InspectInventoryArgs {
    #[command(flatten)]
    app: CliAppSelectorArgs,
}

#[derive(Args, Clone)]
struct InspectLayoutArgs {
    #[command(flatten)]
    app: CliAppSelectorArgs,
}

#[derive(Args, Clone)]
struct ExportArgs {
    #[command(subcommand)]
    command: ExportCommand,
}

#[derive(Subcommand, Clone)]
enum ExportCommand {
    Inventory(ExportInventoryArgs),
    SemanticDag(ExportSemanticDagArgs),
    Contracts(ExportContractsArgs),
    EvalPlan(ExportEvalPlanArgs),
    RuntimeTrace(ExportRuntimeTraceArgs),
}

#[derive(Args, Clone)]
struct ExportInventoryArgs {
    #[command(flatten)]
    app: CliAppSelectorArgs,
    #[arg(long)]
    write_store: bool,
}

#[derive(Args, Clone)]
struct ExportSemanticDagArgs {
    #[command(flatten)]
    app: CliAppSelectorArgs,
    #[arg(long = "dataset-id")]
    dataset_id: String,
    #[arg(long = "metric-id")]
    metric_ids: Vec<String>,
    #[arg(long)]
    write_store: bool,
}

#[derive(Args, Clone)]
struct ExportContractsArgs {
    #[command(flatten)]
    app: CliAppSelectorArgs,
    #[arg(long = "dataset-id")]
    dataset_id: String,
    #[arg(long = "metric-id")]
    metric_ids: Vec<String>,
    #[arg(long)]
    write_store: bool,
}

#[derive(Args, Clone)]
struct ExportEvalPlanArgs {
    #[command(flatten)]
    app: CliAppSelectorArgs,
    #[arg(long = "dataset-id")]
    dataset_id: String,
    #[arg(long = "metric-id")]
    metric_ids: Vec<String>,
    #[arg(long)]
    search: Option<String>,
    #[arg(long = "filter")]
    filters: Vec<String>,
    #[arg(long)]
    write_store: bool,
}

#[derive(Args, Clone)]
struct ExportRuntimeTraceArgs {
    #[command(flatten)]
    app: CliAppSelectorArgs,
    #[arg(long)]
    trace_limit: Option<usize>,
    #[arg(long)]
    write_store: bool,
}

#[derive(Args, Clone)]
struct QueryArgs {
    #[command(subcommand)]
    command: QueryCommand,
}

#[derive(Subcommand, Clone)]
enum QueryCommand {
    Dataset(QueryDatasetArgs),
    Metric(QueryMetricArgs),
    Resource(QueryResourceArgs),
}

#[derive(Args, Clone)]
struct QueryDatasetArgs {
    #[command(flatten)]
    app: CliAppSelectorArgs,
    #[arg(long)]
    id: String,
    #[arg(long)]
    search: Option<String>,
    #[arg(long = "filter")]
    filters: Vec<String>,
    #[arg(long = "column")]
    columns: Vec<String>,
    #[arg(long)]
    limit: Option<usize>,
}

#[derive(Args, Clone)]
struct QueryMetricArgs {
    #[command(flatten)]
    app: CliAppSelectorArgs,
    #[arg(long)]
    id: String,
    #[arg(long = "metric-id")]
    metric_ids: Vec<String>,
    #[arg(long)]
    search: Option<String>,
    #[arg(long = "filter")]
    filters: Vec<String>,
}

#[derive(Args, Clone)]
struct QueryResourceArgs {
    #[command(flatten)]
    app: CliAppSelectorArgs,
    #[arg(long)]
    id: String,
}

#[derive(Args, Clone)]
struct RuntimeArgs {
    #[command(subcommand)]
    command: RuntimeCommand,
}

#[derive(Subcommand, Clone)]
enum RuntimeCommand {
    Peek(RuntimePeekArgs),
}

#[derive(Args, Clone)]
struct RuntimePeekArgs {
    #[command(flatten)]
    app: CliAppSelectorArgs,
    #[arg(long)]
    trace_limit: Option<usize>,
}

#[derive(Args, Clone)]
struct McpArgs {
    #[command(subcommand)]
    command: McpCommand,
}

#[derive(Subcommand, Clone)]
enum McpCommand {
    Describe(McpDescribeArgs),
}

#[derive(Args, Clone)]
struct McpDescribeArgs {
    #[arg(long, value_parser = ["editor", "access"])]
    surface: String,
    #[arg(long)]
    json: bool,
}

#[derive(clap::Args)]
struct ServeArgs {
    #[arg(long, default_value = "../workspaces")]
    source_root: PathBuf,
    #[arg(long, default_value = "full", value_parser = ["full", "access-only"])]
    host_surface: String,
    /// 启用宿主登录鉴权（须已配置用户，否则启动失败）
    #[arg(long)]
    auth: bool,
    #[arg(long, default_value = "127.0.0.1")]
    host: String,
    #[arg(long, default_value_t = 9527)]
    port: u16,
    /// 显式允许在 mei 启动时自动拉起托管的内置 Agent 运行时（默认关闭）
    #[arg(long)]
    auto_agent: bool,
    /// 启动时将 MeiLang skill 同步到工作区（默认关闭；与 `--auto-agent` 联用时自动开启）
    #[arg(long)]
    sync_agent_skill: bool,
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
    let cli = Cli::parse();
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
        Command::Compile(args) => compile_or_check_command("compile", args),
        Command::Check(args) => compile_or_check_command("check", args),
        Command::Inspect(args) => inspect_command(args),
        Command::Export(args) => export_command(args),
        Command::Query(args) => query_command(args),
        Command::Runtime(args) => runtime_command(args),
        Command::Mcp(args) => mcp_command(args),
    }
}

fn resolve_package_root() -> Result<PathBuf> {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .context("server crate manifest has no parent directory")
        .map(std::path::Path::to_path_buf)
}

fn resolve_cli_source_root(package_root: &std::path::Path, raw: &PathBuf) -> Result<PathBuf> {
    let source_root = if raw.is_absolute() {
        raw.clone()
    } else {
        package_root.join(raw)
    };
    if !source_root.exists() {
        anyhow::bail!(
            "source root `{}` does not exist; create it first or pass a valid --source-root",
            source_root.display()
        );
    }
    if !source_root.is_dir() {
        anyhow::bail!(
            "source root `{}` is not a directory; pass a directory path to --source-root",
            source_root.display()
        );
    }
    source_root.canonicalize().with_context(|| {
        format!(
            "failed to canonicalize source root {}",
            source_root.display()
        )
    })
}

fn normalize_optional_arg(value: &Option<String>) -> Option<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn compile_options_from_selector(args: &CliAppSelectorArgs) -> CompileOptions {
    CompileOptions {
        scene: normalize_optional_arg(&args.scene),
        preview_target: normalize_optional_arg(&args.target_file),
    }
}

fn world_scope_from_selector(args: &CliAppSelectorArgs) -> Option<http::scene_api::WorldScope> {
    let scene_id = normalize_optional_arg(&args.scene);
    let target_file = normalize_optional_arg(&args.target_file);
    if scene_id.is_none() && target_file.is_none() {
        None
    } else {
        Some(http::scene_api::WorldScope {
            scene_id,
            target_file,
        })
    }
}

fn inspect_layout_for_app(source_root: &std::path::Path, app_id: &str) -> toolchain::SourceLayoutInspection {
    toolchain::inspect_source_layout(source_root, app_id)
}

fn ensure_cli_layout_ready(layout: &toolchain::SourceLayoutInspection) -> Result<()> {
    let errors: Vec<&toolchain::LayoutCheck> = layout
        .checks
        .iter()
        .filter(|item| item.level == "error")
        .collect();
    if errors.is_empty() {
        return Ok(());
    }
    let summary = errors
        .iter()
        .map(|item| {
            if let Some(hint) = item.hint.as_deref() {
                format!("- {} ({hint})", item.message)
            } else {
                format!("- {}", item.message)
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    anyhow::bail!(
        "layout checks failed for app `{}`:\n{}\nRun `mei inspect layout --app {} --source-root {}` for full report.",
        layout.app_id,
        summary,
        layout.app_id,
        layout.roots.source_root
    );
}

fn attach_layout_to_envelope(
    envelope: &mut toolchain::HeadlessArtifactEnvelope,
    layout: &toolchain::SourceLayoutInspection,
) -> Result<()> {
    let layout_value = serde_json::to_value(layout)?;
    if let Some(obj) = envelope.artifact.as_object_mut() {
        obj.insert("layout".to_string(), layout_value);
    } else {
        let current = envelope.artifact.clone();
        envelope.artifact = json!({
            "value": current,
            "layout": layout_value,
        });
    }
    Ok(())
}

fn parse_cli_filters(filters: &[String]) -> Result<std::collections::BTreeMap<String, String>> {
    let mut out = std::collections::BTreeMap::new();
    for item in filters {
        let raw = item.trim();
        let Some((key, value)) = raw.split_once('=') else {
            anyhow::bail!("invalid --filter `{raw}`; expected key=value");
        };
        let key = key.trim();
        let value = value.trim();
        if key.is_empty() || value.is_empty() {
            anyhow::bail!("invalid --filter `{raw}`; expected non-empty key=value");
        }
        out.insert(key.to_string(), value.to_string());
    }
    Ok(out)
}

fn diagnostics_summary(diagnostics: &[Diagnostic]) -> serde_json::Value {
    let mut errors = 0usize;
    let mut warnings = 0usize;
    let mut infos = 0usize;
    for item in diagnostics {
        match item.severity {
            Severity::Error => errors += 1,
            Severity::Warning => warnings += 1,
            Severity::Info => infos += 1,
        }
    }
    json!({
        "errors": errors,
        "warnings": warnings,
        "infos": infos,
    })
}

fn watched_files_json(files: &[CompileWatchedFile]) -> Vec<serde_json::Value> {
    files.iter()
        .map(|item| {
            json!({
                "rel_path": item.rel_path,
                "modified_ms": item.modified_ms,
                "size_bytes": item.size_bytes,
            })
        })
        .collect()
}

fn scope_json(scope: Option<&http::scene_api::WorldScope>) -> serde_json::Value {
    match scope {
        Some(scope) => json!({
            "scene_id": scope.scene_id,
            "target_file": scope.target_file,
        }),
        None => serde_json::Value::Null,
    }
}

fn print_json_output<T: Serialize>(value: &T, pretty: bool) -> Result<()> {
    if pretty {
        println!("{}", serde_json::to_string_pretty(value)?);
    } else {
        println!("{}", serde_json::to_string(value)?);
    }
    Ok(())
}

fn compile_or_check_command(command: &str, args: CheckArgs) -> Result<()> {
    let package_root = resolve_package_root()?;
    agent_runtime::runtime::load_repo_dotenv(&package_root);
    let source_root = resolve_cli_source_root(&package_root, &args.app.source_root)?;
    let app_id = args.app.app.trim();
    if app_id.is_empty() {
        anyhow::bail!("--app is required");
    }
    let layout = inspect_layout_for_app(&source_root, app_id);
    ensure_cli_layout_ready(&layout)?;
    let options = compile_options_from_selector(&args.app);
    let report = toolchain::compile_report(&source_root, app_id, options.clone())?;
    let compiled = report.compiled;
    let output = json!({
        "schema_version": "mei-cli-v1",
        "command": command,
        "app_id": app_id,
        "source_root": source_root,
        "requested": {
            "scene_id": options.scene,
            "target_file": options.preview_target,
        },
        "active": {
            "scene_id": compiled.active_scene,
            "target_file": compiled.active_target_file,
        },
        "ok": !compiled.diagnostics.iter().any(|item| matches!(item.severity, Severity::Error)),
        "diagnostics_summary": diagnostics_summary(&compiled.diagnostics),
        "diagnostics": compiled.diagnostics,
        "scene_routes": compiled.scene_routes,
        "revision": {
            "token": report.revision_token,
            "components_revision": report.components_revision,
            "watched_files": watched_files_json(&report.watched_files),
        },
        "layout": layout,
    });
    print_json_output(&output, args.app.json)
}

fn inspect_command(args: InspectArgs) -> Result<()> {
    match args.command {
        InspectCommand::World(args) => inspect_world_command(args),
        InspectCommand::Inventory(args) => inspect_inventory_command(args),
        InspectCommand::Layout(args) => inspect_layout_command(args),
    }
}

fn inspect_world_command(args: InspectWorldArgs) -> Result<()> {
    let package_root = resolve_package_root()?;
    let source_root = resolve_cli_source_root(&package_root, &args.app.source_root)?;
    let app_id = args.app.app.trim();
    if app_id.is_empty() {
        anyhow::bail!("--app is required");
    }
    let layout = inspect_layout_for_app(&source_root, app_id);
    ensure_cli_layout_ready(&layout)?;
    let scope = world_scope_from_selector(&args.app);
    let snapshot = toolchain::build_world_context_snapshot(&source_root, app_id, scope.as_ref())?;
    let output = json!({
        "schema_version": "mei-cli-v1",
        "command": "inspect.world",
        "app_id": app_id,
        "scope": scope_json(scope.as_ref()),
        "world_context": snapshot,
        "layout": layout,
    });
    print_json_output(&output, args.app.json)
}

fn inspect_inventory_command(args: InspectInventoryArgs) -> Result<()> {
    let package_root = resolve_package_root()?;
    let source_root = resolve_cli_source_root(&package_root, &args.app.source_root)?;
    let app_id = args.app.app.trim();
    if app_id.is_empty() {
        anyhow::bail!("--app is required");
    }
    let layout = inspect_layout_for_app(&source_root, app_id);
    ensure_cli_layout_ready(&layout)?;
    let scope = world_scope_from_selector(&args.app);
    let snapshot = toolchain::build_world_context_snapshot(&source_root, app_id, scope.as_ref())?;
    let output = json!({
        "schema_version": "mei-cli-v1",
        "command": "inspect.inventory",
        "app_id": app_id,
        "scope": scope_json(scope.as_ref()),
        "active_target_file": snapshot.active_target_file,
        "inventory": snapshot.resource_inventory,
        "layout": layout,
    });
    print_json_output(&output, args.app.json)
}

fn inspect_layout_command(args: InspectLayoutArgs) -> Result<()> {
    let package_root = resolve_package_root()?;
    let source_root = resolve_cli_source_root(&package_root, &args.app.source_root)?;
    let app_id = args.app.app.trim();
    if app_id.is_empty() {
        anyhow::bail!("--app is required");
    }
    let layout = inspect_layout_for_app(&source_root, app_id);
    let output = json!({
        "schema_version": "mei-cli-v1",
        "command": "inspect.layout",
        "app_id": app_id,
        "layout": layout,
    });
    print_json_output(&output, args.app.json)
}

fn export_command(args: ExportArgs) -> Result<()> {
    match args.command {
        ExportCommand::Inventory(args) => export_inventory_command(args),
        ExportCommand::SemanticDag(args) => export_semantic_dag_command(args),
        ExportCommand::Contracts(args) => export_contracts_command(args),
        ExportCommand::EvalPlan(args) => export_eval_plan_command(args),
        ExportCommand::RuntimeTrace(args) => export_runtime_trace_command(args),
    }
}

fn export_inventory_command(args: ExportInventoryArgs) -> Result<()> {
    let package_root = resolve_package_root()?;
    let source_root = resolve_cli_source_root(&package_root, &args.app.source_root)?;
    let app_id = args.app.app.trim();
    if app_id.is_empty() {
        anyhow::bail!("--app is required");
    }
    let layout = inspect_layout_for_app(&source_root, app_id);
    ensure_cli_layout_ready(&layout)?;
    let scope = world_scope_from_selector(&args.app).unwrap_or_default();
    let mut envelope = toolchain::export_inventory_snapshot(
        &source_root,
        app_id,
        &scope,
        HeadlessExportOptions {
            write_store: args.write_store,
        },
    )?;
    attach_layout_to_envelope(&mut envelope, &layout)?;
    print_json_output(&envelope, args.app.json)
}

fn export_semantic_dag_command(args: ExportSemanticDagArgs) -> Result<()> {
    let package_root = resolve_package_root()?;
    let source_root = resolve_cli_source_root(&package_root, &args.app.source_root)?;
    let app_id = args.app.app.trim();
    if app_id.is_empty() {
        anyhow::bail!("--app is required");
    }
    let layout = inspect_layout_for_app(&source_root, app_id);
    ensure_cli_layout_ready(&layout)?;
    let scope = world_scope_from_selector(&args.app).unwrap_or_default();
    let mut envelope = toolchain::export_semantic_dag(
        &source_root,
        app_id,
        &scope,
        args.dataset_id.trim(),
        &args.metric_ids,
        HeadlessExportOptions {
            write_store: args.write_store,
        },
    )?;
    attach_layout_to_envelope(&mut envelope, &layout)?;
    print_json_output(&envelope, args.app.json)
}

fn export_contracts_command(args: ExportContractsArgs) -> Result<()> {
    let package_root = resolve_package_root()?;
    let source_root = resolve_cli_source_root(&package_root, &args.app.source_root)?;
    let app_id = args.app.app.trim();
    if app_id.is_empty() {
        anyhow::bail!("--app is required");
    }
    let layout = inspect_layout_for_app(&source_root, app_id);
    ensure_cli_layout_ready(&layout)?;
    let scope = world_scope_from_selector(&args.app).unwrap_or_default();
    let mut envelope = toolchain::export_analysis_contracts(
        &source_root,
        app_id,
        &scope,
        args.dataset_id.trim(),
        &args.metric_ids,
        HeadlessExportOptions {
            write_store: args.write_store,
        },
    )?;
    attach_layout_to_envelope(&mut envelope, &layout)?;
    print_json_output(&envelope, args.app.json)
}

fn export_eval_plan_command(args: ExportEvalPlanArgs) -> Result<()> {
    let package_root = resolve_package_root()?;
    let source_root = resolve_cli_source_root(&package_root, &args.app.source_root)?;
    let app_id = args.app.app.trim();
    if app_id.is_empty() {
        anyhow::bail!("--app is required");
    }
    let layout = inspect_layout_for_app(&source_root, app_id);
    ensure_cli_layout_ready(&layout)?;
    let scope = world_scope_from_selector(&args.app).unwrap_or_default();
    let filters = parse_cli_filters(&args.filters)?;
    let mut envelope = toolchain::export_eval_plan(
        &source_root,
        app_id,
        &scope,
        args.dataset_id.trim(),
        &args.metric_ids,
        args.search.as_deref(),
        &filters,
        HeadlessExportOptions {
            write_store: args.write_store,
        },
    )?;
    attach_layout_to_envelope(&mut envelope, &layout)?;
    print_json_output(&envelope, args.app.json)
}

fn export_runtime_trace_command(args: ExportRuntimeTraceArgs) -> Result<()> {
    let package_root = resolve_package_root()?;
    let source_root = resolve_cli_source_root(&package_root, &args.app.source_root)?;
    let app_id = args.app.app.trim();
    if app_id.is_empty() {
        anyhow::bail!("--app is required");
    }
    let layout = inspect_layout_for_app(&source_root, app_id);
    ensure_cli_layout_ready(&layout)?;
    let scope = world_scope_from_selector(&args.app).unwrap_or_default();
    let mut envelope = toolchain::export_runtime_trace(
        &source_root,
        app_id,
        &scope,
        args.trace_limit,
        HeadlessExportOptions {
            write_store: args.write_store,
        },
    )?;
    attach_layout_to_envelope(&mut envelope, &layout)?;
    print_json_output(&envelope, args.app.json)
}

fn query_command(args: QueryArgs) -> Result<()> {
    match args.command {
        QueryCommand::Dataset(args) => query_dataset_command(args),
        QueryCommand::Metric(args) => query_metric_command(args),
        QueryCommand::Resource(args) => query_resource_command(args),
    }
}

fn query_dataset_command(args: QueryDatasetArgs) -> Result<()> {
    let package_root = resolve_package_root()?;
    let source_root = resolve_cli_source_root(&package_root, &args.app.source_root)?;
    let app_id = args.app.app.trim();
    if app_id.is_empty() {
        anyhow::bail!("--app is required");
    }
    let layout = inspect_layout_for_app(&source_root, app_id);
    ensure_cli_layout_ready(&layout)?;
    let scope = world_scope_from_selector(&args.app);
    let filters = parse_cli_filters(&args.filters)?;
    let columns = if args.columns.is_empty() {
        None
    } else {
        Some(args.columns.as_slice())
    };
    let result = http::scene_api::query_resource_dataset(
        &source_root,
        app_id,
        scope.as_ref(),
        args.id.trim(),
        args.search.as_deref(),
        &filters,
        columns,
        args.limit,
    )?;
    let output = json!({
        "schema_version": "mei-cli-v1",
        "command": "query.dataset",
        "app_id": app_id,
        "scope": scope_json(scope.as_ref()),
        "dataset_id": args.id.trim(),
        "result": result,
        "layout": layout,
    });
    print_json_output(&output, args.app.json)
}

fn query_metric_command(args: QueryMetricArgs) -> Result<()> {
    let package_root = resolve_package_root()?;
    let source_root = resolve_cli_source_root(&package_root, &args.app.source_root)?;
    let app_id = args.app.app.trim();
    if app_id.is_empty() {
        anyhow::bail!("--app is required");
    }
    let layout = inspect_layout_for_app(&source_root, app_id);
    ensure_cli_layout_ready(&layout)?;
    let scope = world_scope_from_selector(&args.app);
    let filters = parse_cli_filters(&args.filters)?;
    let result = http::scene_api::query_resource_dataset_metric(
        &source_root,
        app_id,
        scope.as_ref(),
        args.id.trim(),
        &args.metric_ids,
        args.search.as_deref(),
        &filters,
    )?;
    let output = json!({
        "schema_version": "mei-cli-v1",
        "command": "query.metric",
        "app_id": app_id,
        "scope": scope_json(scope.as_ref()),
        "dataset_id": args.id.trim(),
        "metric_ids": args.metric_ids,
        "result": result,
        "layout": layout,
    });
    print_json_output(&output, args.app.json)
}

fn query_resource_command(args: QueryResourceArgs) -> Result<()> {
    let package_root = resolve_package_root()?;
    let source_root = resolve_cli_source_root(&package_root, &args.app.source_root)?;
    let app_id = args.app.app.trim();
    if app_id.is_empty() {
        anyhow::bail!("--app is required");
    }
    let layout = inspect_layout_for_app(&source_root, app_id);
    ensure_cli_layout_ready(&layout)?;
    let scope = world_scope_from_selector(&args.app);
    let result = toolchain::query_world_asset(&source_root, app_id, scope.as_ref(), args.id.trim())?;
    let output = json!({
        "schema_version": "mei-cli-v1",
        "command": "query.resource",
        "app_id": app_id,
        "scope": scope_json(scope.as_ref()),
        "resource_id": args.id.trim(),
        "result": result,
        "layout": layout,
    });
    print_json_output(&output, args.app.json)
}

fn runtime_command(args: RuntimeArgs) -> Result<()> {
    match args.command {
        RuntimeCommand::Peek(args) => runtime_peek_command(args),
    }
}

fn host_command(args: HostArgs) -> Result<()> {
    match args.command {
        HostCommand::Describe(args) => host_describe_command(args),
        HostCommand::Auth(args) => host_auth_command(args),
    }
}

fn host_describe_command(args: HostDescribeArgs) -> Result<()> {
    let output = json!({
        "schema_version": "mei-cli-v1",
        "command": "host.describe",
        "host_contract": host_runtime_contract_descriptor(),
    });
    print_json_output(&output, args.json)
}

fn host_auth_command(args: HostAuthArgs) -> Result<()> {
    match args.command {
        HostAuthCommand::EnsureKeys(args) => host_auth_ensure_keys_command(args),
        HostAuthCommand::BootstrapUsers(args) => host_auth_bootstrap_users_command(args),
        HostAuthCommand::AddUser(args) => host_auth_add_user_command(args),
        HostAuthCommand::DisableUser(args) => host_auth_set_user_enabled_command(args, false),
        HostAuthCommand::EnableUser(args) => host_auth_set_user_enabled_command(args, true),
        HostAuthCommand::RotateKeys(args) => host_auth_rotate_keys_command(args),
        HostAuthCommand::HashPassword(args) => host_auth_hash_password_command(args),
        HostAuthCommand::Describe(args) => host_auth_describe_command(args),
    }
}

fn host_auth_ensure_keys_command(args: HostAuthEnsureKeysArgs) -> Result<()> {
    let package_root = resolve_package_root()?;
    let source_root = resolve_cli_source_root(&package_root, &args.source_root)?;
    let bundle = auth::ensure_workspace_auth_base(&source_root)?;
    let runtime = auth::load_auth_runtime(&source_root)?;
    let output = json!({
        "schema_version": "mei-cli-v1",
        "command": "host.auth.ensure-keys",
        "source_root": source_root.display().to_string(),
        "config_path": bundle.config_path.display().to_string(),
        "enabled": runtime.enabled,
        "user_count": runtime.user_count(),
        "cookie_name": runtime.cookie_name,
        "jwt_ttl_seconds": runtime.jwt_ttl_seconds,
        "public_key_pem_present": !runtime.public_key_pem.trim().is_empty(),
        "private_key_pem_present": !runtime.private_key_pem.trim().is_empty(),
    });
    print_json_output(&output, args.json)
}

fn read_password_from_stdin() -> Result<String> {
    use std::io::Read;
    let mut password = String::new();
    std::io::stdin()
        .read_to_string(&mut password)
        .context("failed to read password from stdin")?;
    let password = password.trim().to_string();
    if password.is_empty() {
        anyhow::bail!("password must not be empty");
    }
    Ok(password)
}

fn parse_scene_allow_entries(entries: &[String]) -> Result<BTreeMap<String, Vec<String>>> {
    let mut allow = BTreeMap::<String, Vec<String>>::new();
    for entry in entries {
        let trimmed = entry.trim();
        let Some((app_raw, scene_raw)) = trimmed.split_once(':') else {
            anyhow::bail!("invalid scene-allow `{trimmed}`; expected app_id:scene_id");
        };
        let app_id = auth::normalize_id(app_raw);
        let scene_id = scene_raw.trim().to_string();
        if app_id.is_empty() || scene_id.is_empty() {
            anyhow::bail!("invalid scene-allow `{trimmed}`; app and scene are required");
        }
        allow.entry(app_id).or_default().push(scene_id);
    }
    for scenes in allow.values_mut() {
        scenes.sort();
        scenes.dedup();
    }
    Ok(allow)
}

fn host_auth_bootstrap_users_command(args: HostAuthBootstrapUsersArgs) -> Result<()> {
    let package_root = resolve_package_root()?;
    let source_root = resolve_cli_source_root(&package_root, &args.source_root)?;
    let _ = auth::ensure_workspace_auth_base(&source_root)?;
    let guest_app_allow = args
        .guest_app_allow
        .iter()
        .map(|value| auth::normalize_id(value))
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    let guest_scene_allow = parse_scene_allow_entries(&args.guest_scene_allow)?;

    let (super_password, admin_password, guest_password, password_mode, shared_hash) =
        if args.default_password_stdin {
            let password = read_password_from_stdin()?;
            let hash = auth::hash_password(password.as_str())?;
            (
                password.clone(),
                password.clone(),
                password,
                "default_password_stdin",
                Some(hash),
            )
        } else {
            (
                auth::generate_temporary_password(),
                auth::generate_temporary_password(),
                auth::generate_temporary_password(),
                "random_temporary",
                None,
            )
        };

    let super_hash = match shared_hash.as_deref() {
        Some(hash) => hash.to_string(),
        None => auth::hash_password(super_password.as_str())?,
    };
    auth::upsert_workspace_user(
        &source_root,
        args.super_username.as_str(),
        args.super_profile.as_str(),
        auth::AuthRole::Super,
        super_hash.as_str(),
        &[],
        &[],
        &BTreeMap::new(),
    )?;

    let admin_hash = match shared_hash.as_deref() {
        Some(hash) => hash.to_string(),
        None => auth::hash_password(admin_password.as_str())?,
    };
    auth::upsert_workspace_user(
        &source_root,
        args.admin_username.as_str(),
        args.admin_profile.as_str(),
        auth::AuthRole::Admin,
        admin_hash.as_str(),
        &[],
        &[],
        &BTreeMap::new(),
    )?;

    let guest_hash = match shared_hash.as_deref() {
        Some(hash) => hash.to_string(),
        None => auth::hash_password(guest_password.as_str())?,
    };
    auth::upsert_workspace_user(
        &source_root,
        args.guest_username.as_str(),
        args.guest_profile.as_str(),
        auth::AuthRole::Guest,
        guest_hash.as_str(),
        &guest_app_allow,
        &[],
        &guest_scene_allow,
    )?;

    let runtime = auth::load_auth_runtime(&source_root)?;
    let output = json!({
        "schema_version": "mei-cli-v1",
        "command": "host.auth.bootstrap-users",
        "source_root": source_root.display().to_string(),
        "config_path": runtime.config_path.display().to_string(),
        "enabled": runtime.enabled,
        "user_count": runtime.user_count(),
        "password_mode": password_mode,
        "warning": if password_mode == "random_temporary" {
            "temporary_password is shown once; rotate immediately via login change-password flow"
        } else {
            "default_password_stdin is for local debugging only; do not use in production"
        },
        "users": [
            {
                "username": args.super_username.trim(),
                "role": "super",
                "profile": args.super_profile.trim(),
                "temporary_password": super_password
            },
            {
                "username": args.admin_username.trim(),
                "role": "admin",
                "profile": args.admin_profile.trim(),
                "temporary_password": admin_password
            },
            {
                "username": args.guest_username.trim(),
                "role": "guest",
                "profile": args.guest_profile.trim(),
                "temporary_password": guest_password,
                "app_allowlist": guest_app_allow,
                "scene_allowlist": guest_scene_allow
            }
        ]
    });
    print_json_output(&output, args.json)
}

fn host_auth_add_user_command(args: HostAuthAddUserArgs) -> Result<()> {
    if !args.password_stdin {
        anyhow::bail!("--password-stdin is required; plaintext password flags are forbidden");
    }
    let package_root = resolve_package_root()?;
    let source_root = resolve_cli_source_root(&package_root, &args.source_root)?;
    let _ = auth::ensure_workspace_auth_base(&source_root)?;
    let role = auth::AuthRole::from_slug(args.role.as_str())
        .ok_or_else(|| anyhow::anyhow!("invalid role `{}`", args.role))?;
    let password = read_password_from_stdin()?;
    let password_hash = auth::hash_password(password.as_str())?;
    let app_allow = args
        .app_allow
        .iter()
        .map(|value| auth::normalize_id(value))
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    let scene_allow = parse_scene_allow_entries(&args.scene_allow)?;
    auth::upsert_workspace_user(
        &source_root,
        args.username.as_str(),
        args.profile.as_str(),
        role,
        password_hash.as_str(),
        &app_allow,
        &[],
        &scene_allow,
    )?;
    let runtime = auth::load_auth_runtime(&source_root)?;
    let output = json!({
        "schema_version": "mei-cli-v1",
        "command": "host.auth.add-user",
        "source_root": source_root.display().to_string(),
        "config_path": runtime.config_path.display().to_string(),
        "enabled": runtime.enabled,
        "username": args.username.trim(),
        "role": role.as_str(),
        "app_allowlist": app_allow,
        "scene_allowlist": scene_allow,
        "password_hash_written": true,
    });
    print_json_output(&output, args.json)
}

fn host_auth_set_user_enabled_command(args: HostAuthSetUserEnabledArgs, enabled: bool) -> Result<()> {
    let package_root = resolve_package_root()?;
    let source_root = resolve_cli_source_root(&package_root, &args.source_root)?;
    auth::set_workspace_user_disabled(&source_root, args.username.as_str(), !enabled)?;
    let runtime = auth::load_auth_runtime(&source_root)?;
    let output = json!({
        "schema_version": "mei-cli-v1",
        "command": if enabled { "host.auth.enable-user" } else { "host.auth.disable-user" },
        "source_root": source_root.display().to_string(),
        "config_path": runtime.config_path.display().to_string(),
        "enabled": runtime.enabled,
        "username": args.username.trim(),
        "disabled": !enabled,
    });
    print_json_output(&output, args.json)
}

fn host_auth_hash_password_command(args: HostAuthHashPasswordArgs) -> Result<()> {
    let password = read_password_from_stdin()?;
    let password_hash = auth::hash_password(password.as_str())?;
    let output = json!({
        "schema_version": "mei-cli-v1",
        "command": "host.auth.hash-password",
        "password_hash": password_hash,
    });
    print_json_output(&output, args.json)
}

fn host_auth_rotate_keys_command(args: HostAuthRotateKeysArgs) -> Result<()> {
    let package_root = resolve_package_root()?;
    let source_root = resolve_cli_source_root(&package_root, &args.source_root)?;
    auth::rotate_workspace_key_pair(&source_root)?;
    let runtime = auth::load_auth_runtime(&source_root)?;
    let output = json!({
        "schema_version": "mei-cli-v1",
        "command": "host.auth.rotate-keys",
        "source_root": source_root.display().to_string(),
        "config_path": runtime.config_path.display().to_string(),
        "enabled": runtime.enabled,
    });
    print_json_output(&output, args.json)
}

fn host_auth_describe_command(args: HostAuthDescribeArgs) -> Result<()> {
    let package_root = resolve_package_root()?;
    let source_root = resolve_cli_source_root(&package_root, &args.source_root)?;
    let runtime = auth::load_auth_runtime(&source_root)?;
    let bundle = mei_lang_kernel::load_workspace_auth_bundle(&source_root);
    let journal = mei_lang_kernel::AuthJournal::load(&source_root);
    let users = bundle
        .auth
        .users
        .iter()
        .map(|user| {
            json!({
                "username": user.username,
                "profile": user.profile,
                "roles": user.roles,
                "disabled": user.disabled,
                "app_allowlist": user.app_allowlist,
                "app_denylist": user.app_denylist,
                "scene_allowlist": user.scene_allowlist,
                "password_hash_present": !user.password_hash.trim().is_empty(),
            })
        })
        .collect::<Vec<_>>();
    let output = json!({
        "schema_version": "mei-cli-v1",
        "command": "host.auth.describe",
        "source_root": source_root.display().to_string(),
        "config_path": runtime.config_path.display().to_string(),
        "enabled": runtime.enabled,
        "user_count": runtime.user_count(),
        "journal_revision": journal.revision,
        "users": users,
        "cookie_name": runtime.cookie_name,
        "jwt_ttl_seconds": runtime.jwt_ttl_seconds,
        "public_key_pem_present": !runtime.public_key_pem.trim().is_empty(),
        "private_key_pem_present": !runtime.private_key_pem.trim().is_empty(),
    });
    print_json_output(&output, args.json)
}

fn runtime_peek_command(args: RuntimePeekArgs) -> Result<()> {
    let package_root = resolve_package_root()?;
    let source_root = resolve_cli_source_root(&package_root, &args.app.source_root)?;
    let app_id = args.app.app.trim();
    if app_id.is_empty() {
        anyhow::bail!("--app is required");
    }
    let layout = inspect_layout_for_app(&source_root, app_id);
    ensure_cli_layout_ready(&layout)?;
    let scope = world_scope_from_selector(&args.app);
    let result =
        toolchain::query_world_runtime(&source_root, app_id, scope.as_ref(), args.trace_limit)?;
    let output = json!({
        "schema_version": "mei-cli-v1",
        "command": "runtime.peek",
        "app_id": app_id,
        "scope": scope_json(scope.as_ref()),
        "runtime_capabilities": host_runtime_capabilities_catalog(),
        "host_contract": host_runtime_contract_descriptor(),
        "result": result,
        "layout": layout,
    });
    print_json_output(&output, args.app.json)
}

fn mcp_command(args: McpArgs) -> Result<()> {
    match args.command {
        McpCommand::Describe(args) => mcp_describe_command(args),
    }
}

fn mcp_describe_command(args: McpDescribeArgs) -> Result<()> {
    let surface = args.surface.trim().to_ascii_lowercase();
    let descriptor = match surface.as_str() {
        "editor" => json!({
            "schema_version": "mei-mcp-surface-v1",
            "surface": "editor",
            "profile": "editor_readonly_minimal_v1",
            "transport": {
                "status": "adapter_ready",
                "recommended": "run `npm run mcp:editor-adapter` for stdio MCP and `npm run test:mcp:editor-adapter` for smoke validation"
            },
            "adapter": {
                "reference": "scripts/mcp/mei-editor-stdio-adapter.mjs",
                "entrypoint": "node ./scripts/mcp/mei-editor-stdio-adapter.mjs",
                "smoke_test": "npm run test:mcp:editor-adapter"
            },
            "runtime": {
                "cli_entrypoint": "mei",
                "lsp_entrypoint": "mei-lsp (stdio)",
                "adapter_entrypoint": "node ./scripts/mcp/mei-editor-stdio-adapter.mjs"
            },
            "tools": [
                {
                    "name": "mei_check",
                    "description": "Compile an app and return diagnostics plus revision metadata.",
                    "backed_by": "mei check --app <app> [--source-root <dir>] [--scene <scene>] [--target-file <file>] --json",
                    "input_schema": {
                        "type": "object",
                        "properties": {
                            "app": { "type": "string" },
                            "source_root": { "type": "string" },
                            "scene": { "type": "string" },
                            "target_file": { "type": "string" }
                        },
                        "required": ["app"]
                    }
                },
                {
                    "name": "mei_compile",
                    "description": "Compile an app and return the same JSON contract as check for scripted consumers.",
                    "backed_by": "mei compile --app <app> [--source-root <dir>] [--scene <scene>] [--target-file <file>] --json",
                    "input_schema": {
                        "type": "object",
                        "properties": {
                            "app": { "type": "string" },
                            "source_root": { "type": "string" },
                            "scene": { "type": "string" },
                            "target_file": { "type": "string" }
                        },
                        "required": ["app"]
                    }
                },
                {
                    "name": "mei_host_describe",
                    "description": "Return machine-readable host runtime contract descriptor.",
                    "backed_by": "mei host describe --json",
                    "input_schema": {
                        "type": "object",
                        "properties": {},
                        "additionalProperties": false
                    }
                },
                {
                    "name": "mei_inspect_world",
                    "description": "Return the structured world/runtime snapshot for the selected app scope.",
                    "backed_by": "mei inspect world --app <app> [--source-root <dir>] [--scene <scene>] [--target-file <file>] --json",
                    "input_schema": {
                        "type": "object",
                        "properties": {
                            "app": { "type": "string" },
                            "source_root": { "type": "string" },
                            "scene": { "type": "string" },
                            "target_file": { "type": "string" }
                        },
                        "required": ["app"]
                    }
                },
                {
                    "name": "mei_inspect_inventory",
                    "description": "Return the app inventory/resource index for the selected scope.",
                    "backed_by": "mei inspect inventory --app <app> [--source-root <dir>] [--scene <scene>] [--target-file <file>] --json",
                    "input_schema": {
                        "type": "object",
                        "properties": {
                            "app": { "type": "string" },
                            "source_root": { "type": "string" },
                            "scene": { "type": "string" },
                            "target_file": { "type": "string" }
                        },
                        "required": ["app"]
                    }
                },
                {
                    "name": "mei_query_dataset",
                    "description": "Run bounded dataset row/schema queries.",
                    "backed_by": "mei query dataset --app <app> --source-root <dir> --id <dataset_id> [--scene <scene>] [--filter key=value]... [--column name]... [--limit N] --json",
                    "input_schema": {
                        "type": "object",
                        "properties": {
                            "app": { "type": "string" },
                            "source_root": { "type": "string" },
                            "dataset_id": { "type": "string" },
                            "scene": { "type": "string" },
                            "target_file": { "type": "string" },
                            "search": { "type": "string" },
                            "filters": {
                                "type": "object",
                                "additionalProperties": { "type": "string" }
                            },
                            "columns": {
                                "type": "array",
                                "items": { "type": "string" }
                            },
                            "limit": { "type": "integer", "minimum": 1 }
                        },
                        "required": ["app", "dataset_id"]
                    }
                },
                {
                    "name": "mei_query_metric",
                    "description": "Run bounded runtime metric queries for a dataset.",
                    "backed_by": "mei query metric --app <app> --source-root <dir> --id <dataset_id> [--metric-id <metric>]... [--scene <scene>] [--filter key=value]... --json",
                    "input_schema": {
                        "type": "object",
                        "properties": {
                            "app": { "type": "string" },
                            "source_root": { "type": "string" },
                            "dataset_id": { "type": "string" },
                            "metric_ids": {
                                "type": "array",
                                "items": { "type": "string" }
                            },
                            "scene": { "type": "string" },
                            "target_file": { "type": "string" },
                            "search": { "type": "string" },
                            "filters": {
                                "type": "object",
                                "additionalProperties": { "type": "string" }
                            }
                        },
                        "required": ["app", "dataset_id"]
                    }
                },
                {
                    "name": "mei_runtime_peek",
                    "description": "Peek current runtime phase/result/actions for the selected scope.",
                    "backed_by": "mei runtime peek --app <app> [--source-root <dir>] [--scene <scene>] [--target-file <file>] [--trace-limit N] --json",
                    "input_schema": {
                        "type": "object",
                        "properties": {
                            "app": { "type": "string" },
                            "source_root": { "type": "string" },
                            "scene": { "type": "string" },
                            "target_file": { "type": "string" },
                            "trace_limit": { "type": "integer", "minimum": 1 }
                        },
                        "required": ["app"]
                    }
                },
                {
                    "name": "mei_query_resource",
                    "description": "Fetch a single world resource/entity payload.",
                    "backed_by": "mei query resource --app <app> --source-root <dir> --id <resource_id> [--scene <scene>] [--target-file <file>] --json",
                    "input_schema": {
                        "type": "object",
                        "properties": {
                            "app": { "type": "string" },
                            "source_root": { "type": "string" },
                            "resource_id": { "type": "string" },
                            "scene": { "type": "string" },
                            "target_file": { "type": "string" }
                        },
                        "required": ["app", "resource_id"]
                    }
                }
            ],
            "write_policy": {
                "default": "read_only",
                "note": "Editor-side MCP currently wraps semantic read/check/query surfaces only; file writes stay in the external dev tool."
            },
            "host_contract": host_runtime_contract_descriptor()
        }),
        "access" => json!({
            "schema_version": "mei-mcp-surface-v1",
            "surface": "access",
            "transport": {
                "status": "descriptor_ready",
                "recommended": "bind these tools to host-side access agents after scope/auth is enforced"
            },
            "context_ir": {
                "primary": "world-first",
                "producer": "mei inspect world --app <app> [--scene <scene>] [--target-file <file>] --json"
            },
            "tools": [
                {
                    "name": "dataset_query",
                    "description": "Bounded dataset schema/sample-row query for visitor-facing QA.",
                    "backed_by": "mei query dataset --app <app> --id <dataset_id> [--scene <scene>] [--filter key=value]... [--column name]... [--limit N] --json"
                },
                {
                    "name": "dataset_metric",
                    "description": "Bounded aggregate metric query for visitor-facing QA.",
                    "backed_by": "mei query metric --app <app> --id <dataset_id> [--metric-id <metric>]... [--scene <scene>] [--filter key=value]... --json"
                },
                {
                    "name": "resource_list",
                    "description": "List world assets/resources visible in the current scope.",
                    "backed_by": "mei inspect inventory --app <app> [--scene <scene>] [--target-file <file>] --json"
                },
                {
                    "name": "resource_get",
                    "description": "Fetch a single world resource/entity payload.",
                    "backed_by": "mei query resource --app <app> --id <resource_id> [--scene <scene>] [--target-file <file>] --json"
                },
                {
                    "name": "resource_runtime_peek",
                    "description": "Peek runtime phase/result/actions for the current scope.",
                    "backed_by": "mei runtime peek --app <app> [--scene <scene>] [--target-file <file>] --json"
                },
                {
                    "name": "resource_runtime_trace_export",
                    "description": "Export a bounded runtime trace envelope for the current scope.",
                    "backed_by": "mei export runtime-trace --app <app> [--scene <scene>] [--target-file <file>] [--trace-limit N] --json"
                }
            ],
            "write_policy": {
                "default": "read_only",
                "note": "Access-side MCP is intentionally read-only and should not expose authoring rewrite/diff/revert flows."
            },
            "runtime_capabilities": host_runtime_capabilities_catalog(),
            "host_contract": host_runtime_contract_descriptor()
        }),
        _ => anyhow::bail!("unsupported MCP surface `{surface}`"),
    };
    print_json_output(&descriptor, args.json)
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
