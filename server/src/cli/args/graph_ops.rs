use std::path::PathBuf;

use clap::{Args, Subcommand, ValueEnum};

#[derive(Args, Clone)]
pub struct GraphArgs {
    #[command(subcommand)]
    pub command: GraphCommand,
}

#[derive(Subcommand, Clone)]
pub enum GraphCommand {
    /// 删除 1.3.0 前的 legacy registry / compiled_app / metric-response-index，为 clean rebuild 做准备。
    Migrate(GraphMigrateArgs),
    /// workspace 级磁盘 + registry revision 摘要
    Status(GraphStatusArgs),
    /// 按层列出 MCG/MRG/CAS 节点
    Inspect(GraphInspectArgs),
    /// 一致性校验（exit code 非 0 即失败）
    Doctor(GraphDoctorArgs),
}

#[derive(Args, Clone)]
pub struct GraphMigrateArgs {
    #[arg(long, conflicts_with = "source_root")]
    pub workspace: Option<String>,
    #[arg(long, default_value = "../workspaces/ws-dev")]
    pub source_root: PathBuf,
    #[arg(long = "app")]
    pub app_id: Option<String>,
    #[arg(long, default_value_t = false)]
    pub clean: bool,
    #[arg(long, default_value_t = false)]
    pub json: bool,
}

#[derive(Args, Clone)]
pub struct GraphStatusArgs {
    #[arg(long, conflicts_with = "source_root")]
    pub workspace: Option<String>,
    #[arg(long, default_value = "../workspaces/ws-dev")]
    pub source_root: PathBuf,
    #[arg(long = "app")]
    pub app_id: Option<String>,
    #[arg(long, default_value_t = false)]
    pub json: bool,
}

#[derive(Clone, ValueEnum)]
pub enum GraphInspectLayer {
    Mcg,
    Mrg,
    Cas,
    All,
}

#[derive(Args, Clone)]
pub struct GraphInspectArgs {
    #[arg(long, conflicts_with = "source_root")]
    pub workspace: Option<String>,
    #[arg(long, default_value = "../workspaces/ws-dev")]
    pub source_root: PathBuf,
    #[arg(long = "app")]
    pub app_id: String,
    #[arg(long, value_enum, default_value_t = GraphInspectLayer::All)]
    pub layer: GraphInspectLayer,
    #[arg(long)]
    pub hash: Option<String>,
    #[arg(long, default_value_t = false)]
    pub json: bool,
}

#[derive(Args, Clone)]
pub struct GraphDoctorArgs {
    #[arg(long, conflicts_with = "source_root")]
    pub workspace: Option<String>,
    #[arg(long, default_value = "../workspaces/ws-dev")]
    pub source_root: PathBuf,
    #[arg(long = "app")]
    pub app_id: String,
    #[arg(long, default_value_t = false)]
    pub json: bool,
}

#[derive(Args, Clone)]
pub struct ScopeArgs {
    #[command(subcommand)]
    pub command: ScopeCommand,
}

#[derive(Subcommand, Clone)]
pub enum ScopeCommand {
    Gate(ScopeGateArgs),
}

#[derive(Args, Clone)]
pub struct ScopeGateArgs {
    #[command(subcommand)]
    pub command: ScopeGateCommand,
}

#[derive(Subcommand, Clone)]
pub enum ScopeGateCommand {
    Check(ScopeGateCheckArgs),
}

#[derive(Args, Clone)]
pub struct ScopeGateCheckArgs {
    #[arg(long, conflicts_with = "source_root")]
    pub workspace: Option<String>,
    #[arg(long, default_value = "../workspaces/ws-dev")]
    pub source_root: PathBuf,
    #[arg(long = "app")]
    pub app_id: String,
    #[arg(long)]
    pub scene: Option<String>,
    #[arg(long, alias = "target")]
    pub target_file: Option<String>,
    #[arg(long, default_value_t = false)]
    pub json: bool,
}
