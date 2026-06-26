use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(about = "MeiLang skeleton server", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    Serve(ServeArgs),
    Agent(AgentArgs),
    Host(HostArgs),
    Workspace(WorkspaceArgs),
    Knowledge(KnowledgeArgs),
    EditorRuntime(EditorRuntimeArgs),
    Prebuild(PrebuildArgs),
    Readiness(ReadinessArgs),
    Diagnostics(DiagnosticsArgs),
    Warmup(WarmupArgs),
    Compile(CheckArgs),
    Check(CheckArgs),
    Inspect(InspectArgs),
    Export(ExportArgs),
    Query(QueryArgs),
    Runtime(RuntimeArgs),
    Mcp(McpArgs),
}

#[derive(clap::Args)]
pub struct AgentArgs {
    #[command(subcommand)]
    pub command: AgentCommand,
}

#[derive(Args, Clone)]
pub struct HostArgs {
    #[command(subcommand)]
    pub command: HostCommand,
}

#[derive(Subcommand, Clone)]
pub enum HostCommand {
    Describe(HostDescribeArgs),
    Auth(HostAuthArgs),
}

#[derive(Args, Clone)]
pub struct HostDescribeArgs {
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct WorkspaceArgs {
    #[command(subcommand)]
    pub command: WorkspaceCommand,
}

#[derive(Subcommand)]
pub enum WorkspaceCommand {
    /// 一条命令创建源码工作区，并安装本地 runtime（init + runtime install + scaffold + optional create-app）
    Bootstrap(WorkspaceBootstrapArgs),
    /// 创建 workspace profile 目录与 `workspace.json`（自动补齐 stock/components/templates/authoring）
    Init(WorkspaceInitArgs),
    #[command(hide = true)]
    /// 已弃用：stock 在 init / runtime install / prebuild / host 启动时自动补齐；仅保留 `--force` 供运维脚本覆盖
    Materialize(WorkspaceMaterializeArgs),
    /// 管理 workspace-local `.mei/` runtime 元数据与资产
    Runtime(WorkspaceRuntimeArgs),
    /// 在工作区内创建最小 mei 应用骨架
    CreateApp(WorkspaceCreateAppArgs),
    /// 输出 workspace 级别的 headless 摘要，便于 AI / 外部工具快速理解 app 列表与发现配置
    Summary(WorkspaceSummaryArgs),
    /// v2 build store：promote / rollback / status
    Build(WorkspaceBuildArgs),
    /// 一次性迁移 legacy `.mei/`：工作区根 → runtime/；app 级 → build/active/
    MigrateLegacyAppMei(WorkspaceMigrateLegacyArgs),
    /// 工作区 stock SSOT：同步、诊断、路径迁移
    Stock(WorkspaceStockArgs),
}

#[derive(Args)]
pub struct WorkspaceBootstrapArgs {
    #[arg(long)]
    pub source_root: PathBuf,
    #[arg(long)]
    pub label: Option<String>,
    #[arg(long = "app", alias = "app-id")]
    pub app_id: Option<String>,
    #[arg(long = "tool")]
    pub tools: Vec<String>,
    #[arg(long)]
    pub force: bool,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct WorkspaceInitArgs {
    /// profile 目录名，如 `ws-dev`（创建在 workspaces/ 下）
    pub profile_id: Option<String>,
    #[arg(long)]
    pub label: Option<String>,
    #[arg(long, default_value = "../workspaces")]
    pub workspaces_root: PathBuf,
    #[arg(long)]
    pub source_root: Option<PathBuf>,
    #[arg(long)]
    pub standalone: bool,
    #[arg(long = "tool")]
    pub tools: Vec<String>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct WorkspaceStockArgs {
    #[command(subcommand)]
    pub command: WorkspaceStockCommand,
}

#[derive(Subcommand)]
pub enum WorkspaceStockCommand {
    /// 从平台包强制同步 stock 树到工作区
    Sync(WorkspaceStockSyncArgs),
    /// 检查 stock 树、孤儿路径与 STOCK.json 漂移
    Doctor(WorkspaceStockDoctorArgs),
    /// 迁移 legacy `.stock/` 路径与 authoring 示例引用
    MigratePaths(WorkspaceStockMigratePathsArgs),
    /// 生成/更新隐藏 stock catalog 应用与 warmup manifest
    CatalogAppSync(WorkspaceStockCatalogAppSyncArgs),
}

#[derive(Args)]
pub struct WorkspaceStockSyncArgs {
    #[arg(long, default_value = "../workspaces/ws-dev")]
    pub source_root: PathBuf,
    #[arg(long)]
    pub force: bool,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct WorkspaceStockDoctorArgs {
    #[arg(long, default_value = "../workspaces/ws-dev")]
    pub source_root: PathBuf,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct WorkspaceStockMigratePathsArgs {
    #[arg(long, default_value = "../workspaces/ws-dev")]
    pub source_root: PathBuf,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct WorkspaceStockCatalogAppSyncArgs {
    #[arg(long, default_value = "../workspaces/ws-dev")]
    pub source_root: PathBuf,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct WorkspaceMaterializeArgs {
    #[arg(long, default_value = "../workspaces/ws-dev")]
    pub source_root: PathBuf,
    #[arg(long)]
    pub force: bool,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct WorkspaceRuntimeArgs {
    #[command(subcommand)]
    pub command: WorkspaceRuntimeCommand,
}

#[derive(Subcommand)]
pub enum WorkspaceRuntimeCommand {
    /// 查看当前 workspace-local runtime 状态与 doctor 结果
    Status(WorkspaceRuntimeStatusArgs),
    /// 安装或补齐 workspace-local runtime 元数据与文本资产
    Install(WorkspaceRuntimeInstallArgs),
    /// 更新 workspace-local runtime，但保留 `.mei/local/**` 宿主状态
    Update(WorkspaceRuntimeUpdateArgs),
}

#[derive(Args)]
pub struct WorkspaceRuntimeStatusArgs {
    #[arg(long, default_value = "../workspaces/ws-dev")]
    pub source_root: PathBuf,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct WorkspaceRuntimeInstallArgs {
    #[arg(long, default_value = "../workspaces/ws-dev")]
    pub source_root: PathBuf,
    #[arg(long)]
    pub force: bool,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct WorkspaceRuntimeUpdateArgs {
    #[arg(long, default_value = "../workspaces/ws-dev")]
    pub source_root: PathBuf,
    #[arg(long)]
    pub force: bool,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct WorkspaceCreateAppArgs {
    pub app_id: String,
    #[arg(long, default_value = "../workspaces/ws-dev")]
    pub source_root: PathBuf,
    #[arg(long = "tool")]
    pub tools: Vec<String>,
    #[arg(long)]
    pub scaffold: bool,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct WorkspaceSummaryArgs {
    #[arg(long, default_value = "../workspaces/ws-dev")]
    pub source_root: PathBuf,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct WorkspaceBuildArgs {
    #[command(subcommand)]
    pub command: WorkspaceBuildCommand,
}

#[derive(Subcommand)]
pub enum WorkspaceBuildCommand {
    /// candidate → active；同步各 app build/var active symlink
    Promote(WorkspaceBuildPromoteArgs),
    /// active ← previous
    Rollback(WorkspaceBuildRollbackArgs),
    /// 打印 deploy/state/links.json 与各 app BUILD.json
    Status(WorkspaceBuildStatusArgs),
}

#[derive(Args)]
pub struct WorkspaceBuildPromoteArgs {
    #[arg(long, default_value = "../workspaces/ws-dev")]
    pub source_root: PathBuf,
    #[arg(long = "build-id")]
    pub build_id: Option<String>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct WorkspaceBuildRollbackArgs {
    #[arg(long, default_value = "../workspaces/ws-dev")]
    pub source_root: PathBuf,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct WorkspaceBuildStatusArgs {
    #[arg(long, default_value = "../workspaces/ws-dev")]
    pub source_root: PathBuf,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct WorkspaceMigrateLegacyArgs {
    #[arg(long, default_value = "../workspaces/ws-dev")]
    pub source_root: PathBuf,
    #[arg(long)]
    pub app_id: Option<String>,
    /// 迁移工作区根级 legacy `.mei/`（hosts/agent/runtime → runtime/）
    #[arg(long, default_value_t = true)]
    pub migrate_workspace: bool,
    /// 未指定 --app 时迁移全部 app 的 `.mei/`
    #[arg(long, default_value_t = true)]
    pub all_apps: bool,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Clone)]
pub struct HostAuthArgs {
    #[command(subcommand)]
    pub command: HostAuthCommand,
}

#[derive(Subcommand, Clone)]
pub enum HostAuthCommand {
    /// 生成 JWT 密钥与登录 RSA 密钥对（写入 `.mei/local/hosts/*.state.json`，不涉及用户密码）
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
pub struct HostAuthEnsureKeysArgs {
    #[arg(long, default_value = "../workspaces/ws-dev")]
    pub source_root: PathBuf,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Clone)]
pub struct HostAuthRotateKeysArgs {
    #[arg(long, default_value = "../workspaces/ws-dev")]
    pub source_root: PathBuf,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Clone)]
pub struct HostAuthBootstrapUsersArgs {
    #[arg(long, default_value = "../workspaces/ws-dev")]
    pub source_root: PathBuf,
    #[arg(long, default_value = "super")]
    pub super_username: String,
    #[arg(long, default_value = "超级管理员")]
    pub super_profile: String,
    #[arg(long, default_value = "admin")]
    pub admin_username: String,
    #[arg(long, default_value = "管理员")]
    pub admin_profile: String,
    #[arg(long, default_value = "guest")]
    pub guest_username: String,
    #[arg(long, default_value = "访客")]
    pub guest_profile: String,
    #[arg(long = "guest-app-allow")]
    pub guest_app_allow: Vec<String>,
    #[arg(long = "guest-scene-allow", help = "格式: app_id:scene_id")]
    pub guest_scene_allow: Vec<String>,
    /// 从 stdin 读取统一初始密码（super/admin/guest 共用）；未指定时为各账号随机生成。
    #[arg(long)]
    pub default_password_stdin: bool,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Clone)]
pub struct HostAuthAddUserArgs {
    #[arg(long, default_value = "../workspaces/ws-dev")]
    pub source_root: PathBuf,
    #[arg(long)]
    pub username: String,
    #[arg(long, default_value = "guest", value_parser = ["super", "admin", "guest"])]
    pub role: String,
    #[arg(long, default_value = "")]
    pub profile: String,
    #[arg(long = "app-allow")]
    pub app_allow: Vec<String>,
    #[arg(long = "scene-allow", help = "格式: app_id:scene_id")]
    pub scene_allow: Vec<String>,
    /// 必须显式声明从 stdin 读取密码，避免误将明文放进命令行参数。
    #[arg(long)]
    pub password_stdin: bool,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Clone)]
pub struct HostAuthSetUserEnabledArgs {
    #[arg(long, default_value = "../workspaces/ws-dev")]
    pub source_root: PathBuf,
    #[arg(long)]
    pub username: String,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Clone)]
pub struct HostAuthHashPasswordArgs {
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Clone)]
pub struct HostAuthDescribeArgs {
    #[arg(long, default_value = "../workspaces/ws-dev")]
    pub source_root: PathBuf,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Clone)]
pub struct CliAppSelectorArgs {
    #[arg(long, default_value = "../workspaces/ws-dev")]
    pub source_root: PathBuf,
    #[arg(long)]
    pub app: String,
    #[arg(long)]
    pub scene: Option<String>,
    #[arg(long, alias = "target")]
    pub target_file: Option<String>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Clone)]
pub struct WarmupArgs {
    #[command(subcommand)]
    pub command: WarmupCommand,
}

#[derive(Subcommand, Clone)]
pub enum WarmupCommand {
    /// 从 board.mei 推导 deferred warmup 条目 diff（不写盘）
    Suggest(WarmupSuggestArgs),
}

#[derive(Args, Clone)]
pub struct WarmupSuggestArgs {
    #[arg(long, conflicts_with = "source_root")]
    pub workspace: Option<String>,
    #[arg(long, default_value = "../workspaces/ws-dev")]
    pub source_root: PathBuf,
    #[arg(long = "app")]
    pub app_id: Option<String>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Clone)]
pub struct CheckArgs {
    #[command(flatten)]
    pub app: CliAppSelectorArgs,
}

#[derive(Args, Clone)]
pub struct PrebuildArgs {
    /// 工作区 profile（`workspaces/<name>/`，须含 `.mei-workspace.json`）；与 `--source-root` 二选一。
    #[arg(long, conflicts_with = "source_root")]
    pub workspace: Option<String>,
    #[arg(long, default_value = "../workspaces/ws-dev")]
    pub source_root: PathBuf,
    #[arg(long = "app")]
    pub app_id: Option<String>,
    #[arg(long)]
    pub verify: bool,
    #[arg(long)]
    pub clean: bool,
    /// 忽略 prebuild 输入指纹，强制执行完整 prebuild（不删盘）。
    #[arg(long)]
    pub force_rebuild: bool,
    /// 仅构建 defaultScene + hotScenes 对应的首条热路径，不等待完整 warmup。
    #[arg(long, default_value_t = false)]
    pub hot_only: bool,
    /// 输出可读的摘要 JSON（不含 compile_revision 等冗长字段）。
    #[arg(long)]
    pub json: bool,
    /// 输出完整 prebuild 报告 JSON（体积极大，建议重定向到文件）。
    #[arg(long)]
    pub json_full: bool,
}

#[derive(Args, Clone)]
pub struct ReadinessArgs {
    #[command(subcommand)]
    pub command: ReadinessCommand,
}

#[derive(Subcommand, Clone)]
pub enum ReadinessCommand {
    Check(ReadinessCheckArgs),
}

#[derive(Args, Clone)]
pub struct ReadinessCheckArgs {
    #[arg(long, conflicts_with = "source_root")]
    pub workspace: Option<String>,
    #[arg(long, default_value = "../workspaces/ws-dev")]
    pub source_root: PathBuf,
    #[arg(long)]
    pub json: bool,
    #[arg(long)]
    pub bundle_snapshot_root: Option<PathBuf>,
}

#[derive(Args, Clone)]
pub struct DiagnosticsArgs {
    #[command(subcommand)]
    pub command: DiagnosticsCommand,
}

#[derive(Subcommand, Clone)]
pub enum DiagnosticsCommand {
    Summary(DiagnosticsSummaryArgs),
}

#[derive(Args, Clone)]
pub struct DiagnosticsSummaryArgs {
    #[arg(long, conflicts_with = "source_root")]
    pub workspace: Option<String>,
    #[arg(long, default_value = "../workspaces/ws-dev")]
    pub source_root: PathBuf,
    #[arg(long = "app")]
    pub app_id: String,
    #[arg(long)]
    pub sections: Vec<String>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Clone)]
pub struct InspectArgs {
    #[command(subcommand)]
    pub command: InspectCommand,
}

#[derive(Subcommand, Clone)]
pub enum InspectCommand {
    World(InspectWorldArgs),
    Inventory(InspectInventoryArgs),
    Summary(InspectSummaryArgs),
    Layout(InspectLayoutArgs),
}

#[derive(Args, Clone)]
pub struct InspectWorldArgs {
    #[command(flatten)]
    pub app: CliAppSelectorArgs,
}

#[derive(Args, Clone)]
pub struct InspectInventoryArgs {
    #[command(flatten)]
    pub app: CliAppSelectorArgs,
}

#[derive(Args, Clone)]
pub struct InspectSummaryArgs {
    #[command(flatten)]
    pub app: CliAppSelectorArgs,
}

#[derive(Args, Clone)]
pub struct InspectLayoutArgs {
    #[command(flatten)]
    pub app: CliAppSelectorArgs,
}

#[derive(Args, Clone)]
pub struct ExportArgs {
    #[command(subcommand)]
    pub command: ExportCommand,
}

#[derive(Subcommand, Clone)]
pub enum ExportCommand {
    Inventory(ExportInventoryArgs),
    SemanticDag(ExportSemanticDagArgs),
    Contracts(ExportContractsArgs),
    EvalPlan(ExportEvalPlanArgs),
    RuntimeTrace(ExportRuntimeTraceArgs),
    DataSnapshots(ExportDataSnapshotsArgs),
}

#[derive(Args, Clone)]
pub struct ExportDataSnapshotsArgs {
    #[command(flatten)]
    pub app: CliAppSelectorArgs,
    /// xlsx 相对路径（可重复）；省略时使用 zhifa 默认热表
    #[arg(long = "xlsx")]
    pub xlsx_paths: Vec<String>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Clone)]
pub struct ExportInventoryArgs {
    #[command(flatten)]
    pub app: CliAppSelectorArgs,
    #[arg(long)]
    pub write_store: bool,
}

#[derive(Args, Clone)]
pub struct ExportSemanticDagArgs {
    #[command(flatten)]
    pub app: CliAppSelectorArgs,
    #[arg(long = "dataset-id")]
    pub dataset_id: String,
    #[arg(long = "metric-id")]
    pub metric_ids: Vec<String>,
    #[arg(long)]
    pub write_store: bool,
}

#[derive(Args, Clone)]
pub struct ExportContractsArgs {
    #[command(flatten)]
    pub app: CliAppSelectorArgs,
    #[arg(long = "dataset-id")]
    pub dataset_id: String,
    #[arg(long = "metric-id")]
    pub metric_ids: Vec<String>,
    #[arg(long)]
    pub write_store: bool,
}

#[derive(Args, Clone)]
pub struct ExportEvalPlanArgs {
    #[command(flatten)]
    pub app: CliAppSelectorArgs,
    #[arg(long = "dataset-id")]
    pub dataset_id: String,
    #[arg(long = "metric-id")]
    pub metric_ids: Vec<String>,
    #[arg(long)]
    pub search: Option<String>,
    #[arg(long = "filter")]
    pub filters: Vec<String>,
    #[arg(long)]
    pub write_store: bool,
}

#[derive(Args, Clone)]
pub struct ExportRuntimeTraceArgs {
    #[command(flatten)]
    pub app: CliAppSelectorArgs,
    #[arg(long)]
    pub trace_limit: Option<usize>,
    #[arg(long)]
    pub write_store: bool,
}

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
