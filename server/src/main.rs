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
use clap::{Args, Parser, Subcommand};
use mei_lang_kernel::{
    compile_app_with_options, compile_revision_plan_from_root_with_options, CompileOptions,
    CompileWatchedFile, CompiledApp, Diagnostic, Severity,
};
use serde::Serialize;
use serde_json::json;
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
    Compile(CheckArgs),
    Check(CheckArgs),
    Inspect(InspectArgs),
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
    #[arg(long, default_value = "127.0.0.1")]
    host: String,
    #[arg(long, default_value_t = 9527)]
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
    #[allow(dead_code)] // 测试夹具保留；页面渲染走 GisTilesConfig::resolve_for_app。
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
        Command::Compile(args) => compile_or_check_command("compile", args),
        Command::Check(args) => compile_or_check_command("check", args),
        Command::Inspect(args) => inspect_command(args),
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
    fs::create_dir_all(&source_root).with_context(|| {
        format!(
            "failed to create or access source root {}",
            source_root.display()
        )
    })?;
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
    let options = compile_options_from_selector(&args.app);
    let app_root = source_root.join(app_id);
    let revision_plan =
        compile_revision_plan_from_root_with_options(&source_root, &app_root, &options)?;
    let compiled = compile_app_with_options(&source_root, app_id, options.clone())?;
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
            "token": revision_plan.token,
            "components_revision": revision_plan.components_revision,
            "watched_files": watched_files_json(&revision_plan.watched_files),
        }
    });
    print_json_output(&output, args.app.json)
}

fn inspect_command(args: InspectArgs) -> Result<()> {
    match args.command {
        InspectCommand::World(args) => inspect_world_command(args),
        InspectCommand::Inventory(args) => inspect_inventory_command(args),
    }
}

fn inspect_world_command(args: InspectWorldArgs) -> Result<()> {
    let package_root = resolve_package_root()?;
    let source_root = resolve_cli_source_root(&package_root, &args.app.source_root)?;
    let app_id = args.app.app.trim();
    if app_id.is_empty() {
        anyhow::bail!("--app is required");
    }
    let scope = world_scope_from_selector(&args.app);
    let snapshot = http::scene_api::build_world_context_snapshot(&source_root, app_id, scope.as_ref())?;
    let output = json!({
        "schema_version": "mei-cli-v1",
        "command": "inspect.world",
        "app_id": app_id,
        "scope": scope_json(scope.as_ref()),
        "world_context": snapshot,
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
    let scope = world_scope_from_selector(&args.app);
    let snapshot = http::scene_api::build_world_context_snapshot(&source_root, app_id, scope.as_ref())?;
    let output = json!({
        "schema_version": "mei-cli-v1",
        "command": "inspect.inventory",
        "app_id": app_id,
        "scope": scope_json(scope.as_ref()),
        "active_target_file": snapshot.active_target_file,
        "inventory": snapshot.resource_inventory,
    });
    print_json_output(&output, args.app.json)
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
    let scope = world_scope_from_selector(&args.app);
    let result = http::scene_api::query_resource_get(
        &source_root,
        app_id,
        scope.as_ref(),
        args.id.trim(),
    )?;
    let output = json!({
        "schema_version": "mei-cli-v1",
        "command": "query.resource",
        "app_id": app_id,
        "scope": scope_json(scope.as_ref()),
        "resource_id": args.id.trim(),
        "result": result,
    });
    print_json_output(&output, args.app.json)
}

fn runtime_command(args: RuntimeArgs) -> Result<()> {
    match args.command {
        RuntimeCommand::Peek(args) => runtime_peek_command(args),
    }
}

fn runtime_peek_command(args: RuntimePeekArgs) -> Result<()> {
    let package_root = resolve_package_root()?;
    let source_root = resolve_cli_source_root(&package_root, &args.app.source_root)?;
    let app_id = args.app.app.trim();
    if app_id.is_empty() {
        anyhow::bail!("--app is required");
    }
    let scope = world_scope_from_selector(&args.app);
    let result = http::scene_api::query_resource_runtime_peek(
        &source_root,
        app_id,
        scope.as_ref(),
        args.trace_limit,
    )?;
    let output = json!({
        "schema_version": "mei-cli-v1",
        "command": "runtime.peek",
        "app_id": app_id,
        "scope": scope_json(scope.as_ref()),
        "result": result,
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
            "transport": {
                "status": "descriptor_ready",
                "recommended": "wrap these commands in stdio MCP host or editor-native adapter"
            },
            "tools": [
                {
                    "name": "mei_check",
                    "description": "Compile an app and return diagnostics plus revision metadata.",
                    "backed_by": "mei check --app <app> [--scene <scene>] [--target-file <file>] --json"
                },
                {
                    "name": "mei_compile",
                    "description": "Compile an app and return the same JSON contract as check for scripted consumers.",
                    "backed_by": "mei compile --app <app> [--scene <scene>] [--target-file <file>] --json"
                },
                {
                    "name": "mei_inspect_world",
                    "description": "Return the structured world/runtime snapshot for the selected app scope.",
                    "backed_by": "mei inspect world --app <app> [--scene <scene>] [--target-file <file>] --json"
                },
                {
                    "name": "mei_inspect_inventory",
                    "description": "Return the app inventory/resource index for the selected scope.",
                    "backed_by": "mei inspect inventory --app <app> [--scene <scene>] [--target-file <file>] --json"
                },
                {
                    "name": "mei_query_dataset",
                    "description": "Run bounded dataset row/schema queries.",
                    "backed_by": "mei query dataset --app <app> --id <dataset_id> [--scene <scene>] [--filter key=value]... [--column name]... [--limit N] --json"
                },
                {
                    "name": "mei_query_metric",
                    "description": "Run bounded runtime metric queries for a dataset.",
                    "backed_by": "mei query metric --app <app> --id <dataset_id> [--metric-id <metric>]... [--scene <scene>] [--filter key=value]... --json"
                },
                {
                    "name": "mei_runtime_peek",
                    "description": "Peek current runtime phase/result/actions for the selected scope.",
                    "backed_by": "mei runtime peek --app <app> [--scene <scene>] [--target-file <file>] --json"
                },
                {
                    "name": "mei_query_resource",
                    "description": "Fetch a single world resource/entity payload.",
                    "backed_by": "mei query resource --app <app> --id <resource_id> [--scene <scene>] [--target-file <file>] --json"
                }
            ],
            "write_policy": {
                "default": "read_only",
                "note": "Editor-side MCP currently wraps semantic read/check/query surfaces only; file writes stay in the external dev tool."
            }
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
                }
            ],
            "write_policy": {
                "default": "read_only",
                "note": "Access-side MCP is intentionally read-only and should not expose authoring rewrite/diff/revert flows."
            }
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
    let preferred_mode = if args.auto_agent {
        "managed".to_string()
    } else {
        agent_runtime::runtime::preferred_agent_mode()
    };
    let preferred_server_url = agent_runtime::runtime::preferred_agent_server_url();
    let auto_agent = args.auto_agent;
    let native_agent = Arc::new(mei_agent::NativeAgent::open_with_resource_tools(
        source_root.clone(),
        std::sync::Arc::new(resource_tool_bridge::SceneResourceToolExecutor::default()),
    )?);
    let gis_tiles = Arc::new(gis_config::GisTilesConfig::resolve());
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
    tracing::debug!(
        cwd = ?std::env::current_dir(),
        manifest_dir = env!("CARGO_MANIFEST_DIR"),
        package_root = %package_root.display(),
        source_root = %source_root.display(),
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
