use std::path::PathBuf;

use clap::{Args, Subcommand};

use super::common_ops::CliAppSelectorArgs;

#[derive(Args, Clone)]
pub struct QueryArgs {
    #[command(subcommand)]
    pub command: QueryCommand,
}

#[derive(Subcommand, Clone)]
pub enum QueryCommand {
    Dataset(QueryDatasetArgs),
    Metric(QueryMetricArgs),
    Resource(QueryResourceArgs),
}

#[derive(Args, Clone)]
pub struct QueryDatasetArgs {
    #[command(flatten)]
    pub app: CliAppSelectorArgs,
    #[arg(long)]
    pub id: String,
    #[arg(long)]
    pub search: Option<String>,
    #[arg(long = "filter")]
    pub filters: Vec<String>,
    #[arg(long = "column")]
    pub columns: Vec<String>,
    #[arg(long)]
    pub limit: Option<usize>,
}

#[derive(Args, Clone)]
pub struct QueryMetricArgs {
    #[command(flatten)]
    pub app: CliAppSelectorArgs,
    #[arg(long)]
    pub id: String,
    #[arg(long = "metric-id")]
    pub metric_ids: Vec<String>,
    #[arg(long)]
    pub search: Option<String>,
    #[arg(long = "filter")]
    pub filters: Vec<String>,
}

#[derive(Args, Clone)]
pub struct QueryResourceArgs {
    #[command(flatten)]
    pub app: CliAppSelectorArgs,
    #[arg(long)]
    pub id: String,
}

#[derive(Args, Clone)]
pub struct RuntimeArgs {
    #[command(subcommand)]
    pub command: RuntimeCommand,
}

#[derive(Subcommand, Clone)]
pub enum RuntimeCommand {
    Peek(RuntimePeekArgs),
}

#[derive(Args, Clone)]
pub struct RuntimePeekArgs {
    #[command(flatten)]
    pub app: CliAppSelectorArgs,
    #[arg(long)]
    pub trace_limit: Option<usize>,
}

#[derive(Args, Clone)]
pub struct McpArgs {
    #[command(subcommand)]
    pub command: McpCommand,
}

#[derive(Subcommand, Clone)]
pub enum McpCommand {
    Describe(McpDescribeArgs),
    Catalog(McpCatalogArgs),
}

#[derive(Args, Clone)]
pub struct McpDescribeArgs {
    #[arg(long, value_parser = ["author", "access"])]
    pub surface: String,
    #[arg(long)]
    pub source_root: Option<PathBuf>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Clone)]
pub struct McpCatalogArgs {
    #[arg(long)]
    pub source_root: Option<PathBuf>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Clone)]
pub struct KnowledgeArgs {
    #[arg(long, default_value = "author", value_parser = ["author", "access"])]
    pub surface: String,
    #[arg(long)]
    pub source_root: Option<PathBuf>,
    #[arg(long)]
    pub topic: Option<String>,
    #[arg(long)]
    pub include_content: bool,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Clone)]
pub struct EditorRuntimeArgs {
    #[command(subcommand)]
    pub command: EditorRuntimeCommand,
}

#[derive(Subcommand, Clone)]
pub enum EditorRuntimeCommand {
    Describe(EditorRuntimeDescribeArgs),
    Doctor(EditorRuntimeDoctorArgs),
    Scaffold(EditorRuntimeScaffoldArgs),
}

#[derive(Args, Clone)]
pub struct EditorRuntimeDescribeArgs {
    #[arg(long)]
    pub source_root: Option<PathBuf>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Clone)]
pub struct EditorRuntimeDoctorArgs {
    #[arg(long)]
    pub source_root: Option<PathBuf>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Clone)]
pub struct EditorRuntimeScaffoldArgs {
    #[arg(long, default_value = ".")]
    pub target_root: PathBuf,
    #[arg(long = "tool")]
    pub tools: Vec<String>,
    #[arg(long)]
    pub force: bool,
    #[arg(long)]
    pub json: bool,
}

#[derive(clap::Args)]
pub struct ServeArgs {
    /// 工作区 profile（`workspaces/<name>/`，须含 `.mei-workspace.json`）；与 `--source-root` 二选一。
    #[arg(long, conflicts_with = "source_root")]
    pub workspace: Option<String>,
    #[arg(long, default_value = "../workspaces/ws-dev")]
    pub source_root: PathBuf,
    #[arg(long, default_value = "full", value_parser = ["full", "access-only"])]
    pub host_surface: String,
    /// 启用宿主登录鉴权（须已配置用户，否则启动失败）
    #[arg(long)]
    pub auth: bool,
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,
    #[arg(long, default_value_t = 9527)]
    pub port: u16,
    /// 启动策略：`background-build` 先 bind HTTP 再后台构建；`fail-fast-verify` 保持启动前全量校验。
    #[arg(long, default_value = "background-build", value_parser = ["background-build", "fail-fast-verify"])]
    pub startup_policy: String,
    /// 显式允许在 mei 启动时自动拉起托管的内置 Agent 运行时（默认关闭）
    #[arg(long)]
    pub auto_agent: bool,
    /// 启动时将 MeiLang skill 同步到工作区（默认关闭；与 `--auto-agent` 联用时自动开启）
    #[arg(long)]
    pub sync_agent_skill: bool,
    /// 工具链模式：`cargo` 使用 mei-lang 源码；`installed` 使用工作区 `toolchain/bin/`。
    #[arg(long, default_value = "installed", value_parser = ["cargo", "installed"])]
    pub toolchain_mode: String,
}

#[derive(clap::Args)]
pub struct AgentRuntimeArgs {
    #[command(subcommand)]
    pub command: AgentCommand,
}

#[derive(clap::Args)]
pub struct AgentSkillArgs {
    #[arg(long, default_value = "../workspaces/ws-dev")]
    pub source_root: PathBuf,
    #[command(subcommand)]
    pub command: AgentSkillCommand,
}

#[derive(Subcommand)]
pub enum AgentSkillCommand {
    /// 查看当前 MeiLang skill 安装与同步状态
    Status,
    /// 手动同步 MeiLang skill 到运行时目录
    Sync,
}

#[derive(Subcommand)]
pub enum AgentCommand {
    Skill(AgentSkillArgs),
}
