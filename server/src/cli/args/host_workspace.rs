use std::path::PathBuf;

use clap::{Args, Subcommand};

use super::query_agent::AgentCommand;

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
    /// 清理 build/var/graph/compile 缓存与 prebuild 状态（冷启动基准）
    Clean(WorkspaceBuildCleanArgs),
    /// candidate → active；同步各 app build/var active symlink
    Promote(WorkspaceBuildPromoteArgs),
    /// active ← previous
    Rollback(WorkspaceBuildRollbackArgs),
    /// 打印 deploy/state/links.json 与各 app BUILD.json
    Status(WorkspaceBuildStatusArgs),
}

#[derive(Args)]
pub struct WorkspaceBuildCleanArgs {
    #[arg(long, default_value = "../workspaces/ws-dev")]
    pub source_root: PathBuf,
    #[arg(long = "app")]
    pub app_id: Option<String>,
    #[arg(long)]
    pub json: bool,
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
