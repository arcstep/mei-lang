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
    #[command(subcommand)]
    Workspace(WorkspaceCommand),
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
    pub app: String,
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,
    #[arg(long, default_value = "9527")]
    pub port: u16,
    /// 启用宿主登录鉴权（须已配置用户，否则启动失败）
    #[arg(long)]
    pub auth: bool,
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
