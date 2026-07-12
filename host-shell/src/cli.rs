use clap::{Args, Subcommand};
use std::path::PathBuf;

#[derive(Subcommand, Debug)]
pub enum Command {
    Version(VersionArgs),
    Import(ImportArgs),
    Reload(ReloadArgs),
    Prebuild(PrebuildArgs),
    PrebuildData(PrebuildDataArgs),
    #[command(subcommand)]
    Mrg(MrgCommand),
    #[command(subcommand, name = "auth")]
    Auth(mei_host_auth::cli_args::AuthCommand),
    Serve(ServeArgs),
    #[command(subcommand)]
    Build(BuildCommand),
    #[command(subcommand, name = "build-worker")]
    BuildWorker(BuildWorkerCommand),
    #[command(subcommand)]
    Workspace(WorkspaceCommand),
    #[command(subcommand, name = "apps")]
    Apps(AppsCommand),
    #[command(subcommand, name = "eval-cache")]
    EvalCache(EvalCacheCommand),
}

#[derive(Subcommand, Debug)]
pub enum AppsCommand {
    List(AppsListArgs),
}

#[derive(Args, Debug)]
pub struct AppsListArgs {
    #[arg(long)]
    pub workspace: PathBuf,
    #[arg(long)]
    pub json: bool,
}

#[derive(Subcommand, Debug)]
pub enum WorkspaceCommand {
    Init(WorkspaceInitArgs),
}

#[derive(Subcommand, Debug)]
pub enum BuildCommand {
    Prepare(BuildPrepareArgs),
    Finalize(BuildFinalizeArgs),
    Promote(BuildPromoteArgs),
    Rollback(BuildRollbackArgs),
    Clean(BuildCleanArgs),
    MigrateEnv(BuildMigrateEnvArgs),
    Status(BuildStatusArgs),
}

#[derive(Subcommand, Debug)]
pub enum BuildWorkerCommand {
    /// Run one-shot compile/import/snapshot/seal pipeline from a BuildRequest JSON.
    Run(BuildWorkerRunArgs),
}

#[derive(Args, Debug)]
pub struct BuildWorkerRunArgs {
    #[arg(long)]
    pub workspace: PathBuf,
    /// Path to BuildRequest JSON (`mei-build-request-v1`).
    #[arg(long)]
    pub request: PathBuf,
    /// Optional path for BuildResult JSON; defaults to stdout.
    #[arg(long)]
    pub output: Option<PathBuf>,
}

#[derive(Args, Debug)]
pub struct BuildPrepareArgs {
    #[arg(long)]
    pub workspace: PathBuf,
    #[arg(long)]
    pub app: Vec<String>,
}

#[derive(Args, Debug)]
pub struct BuildFinalizeArgs {
    #[arg(long)]
    pub workspace: PathBuf,
    #[arg(long)]
    pub app: Vec<String>,
    #[arg(long)]
    pub build_id: String,
}

#[derive(Args, Debug)]
pub struct BuildPromoteArgs {
    #[arg(long)]
    pub workspace: PathBuf,
    #[arg(long)]
    pub build_id: Option<String>,
}

#[derive(Args, Debug)]
pub struct BuildRollbackArgs {
    #[arg(long)]
    pub workspace: PathBuf,
}

#[derive(Args, Debug)]
pub struct BuildCleanArgs {
    #[arg(long)]
    pub workspace: PathBuf,
    #[arg(long)]
    pub app: Vec<String>,
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Args, Debug)]
pub struct BuildMigrateEnvArgs {
    #[arg(long)]
    pub workspace: PathBuf,
    #[arg(long)]
    pub app: Vec<String>,
}

#[derive(Args, Debug)]
pub struct BuildStatusArgs {
    #[arg(long)]
    pub workspace: PathBuf,
}

#[derive(Args, Debug)]
pub struct PrebuildArgs {
    #[arg(long)]
    pub workspace: PathBuf,
    #[arg(long)]
    pub app: String,
    #[arg(long, default_value = "home")]
    pub policy: String,
}

#[derive(Args, Debug)]
pub struct PrebuildDataArgs {
    #[arg(long)]
    pub workspace: PathBuf,
    #[arg(long)]
    pub app: String,
}

#[derive(Args, Debug)]
pub struct VersionArgs {
    #[arg(long)]
    pub workspace: Option<PathBuf>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct ImportArgs {
    #[arg(long)]
    pub workspace: PathBuf,
    #[arg(long)]
    pub app: String,
    #[arg(long)]
    pub bundle: Option<PathBuf>,
}

#[derive(Args, Debug)]
pub struct ReloadArgs {
    #[arg(long)]
    pub workspace: PathBuf,
    #[arg(long)]
    pub app: String,
    #[arg(long)]
    pub bundle: Option<PathBuf>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct ServeArgs {
    #[arg(long)]
    pub workspace: PathBuf,
    #[arg(long)]
    pub app: Option<String>,
    /// 声明式 workspace profile 路径；相对路径按 workspace 根解析
    #[arg(long = "workspace-config", value_name = "PATH")]
    pub workspace_config: Option<PathBuf>,
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,
    #[arg(long, default_value = "9527")]
    pub port: u16,
    /// 启用宿主登录鉴权（须已配置用户，否则启动失败）
    #[arg(long)]
    pub auth: bool,
    /// 进程级数据能力上限：eval（默认）| fixture | static
    #[arg(
        long = "data-mode-ceiling",
        value_name = "MODE",
        default_value = "eval"
    )]
    pub data_mode_ceiling: String,
    /// 开发态求值配置：full（默认）| static | scoped（见 0535；亦读 MEI_DEV_EVAL_PROFILE）
    #[arg(long = "dev-eval-profile", value_name = "PROFILE")]
    pub dev_eval_profile: Option<String>,
    /// scoped 时的动态求值 scope 前缀（逗号分隔；亦读 MEI_EVAL_SCOPE）
    #[arg(long = "eval-scope", value_name = "PREFIXES")]
    pub eval_scope: Option<String>,
    /// scoped 时的预热 scope 前缀（逗号分隔；亦读 MEI_WARMUP_SCOPE）
    #[arg(long = "warmup-scope", value_name = "PREFIXES")]
    pub warmup_scope: Option<String>,
}

#[derive(Subcommand, Debug)]
pub enum EvalCacheCommand {
    Invalidate(EvalCacheInvalidateArgs),
}

#[derive(Args, Debug)]
pub struct EvalCacheInvalidateArgs {
    #[arg(long)]
    pub workspace: PathBuf,
    #[arg(long)]
    pub app: String,
    #[arg(long)]
    pub force: bool,
}

#[derive(Subcommand, Debug)]
pub enum MrgCommand {
    Status(MrgStatusArgs),
}

#[derive(Args, Debug)]
pub struct MrgStatusArgs {
    #[arg(long)]
    pub workspace: PathBuf,
    #[arg(long)]
    pub app: String,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct WorkspaceInitArgs {
    #[arg(long)]
    pub dir: PathBuf,
    #[arg(long)]
    pub id: Option<String>,
    #[arg(long)]
    pub label: Option<String>,
    #[arg(long)]
    pub app: Option<String>,
}
