use std::path::PathBuf;

use clap::{Args, Subcommand, ValueEnum};

#[derive(Args, Clone)]
pub struct LayerArgs {
    #[command(subcommand)]
    pub command: LayerCommand,
}

#[derive(Subcommand, Clone)]
pub enum LayerCommand {
    /// 编译指定层（L3 MCG / L4 MRG eval pass）
    Compile(LayerCompileArgs),
    /// 校验 registry ↔ CAS 一致性（exit 1 即失败）
    Verify(LayerVerifyArgs),
    /// 单/多层节点摘要（只读）
    Inspect(LayerInspectArgs),
    /// 增强版 graph status + dirty 计数
    Status(LayerStatusArgs),
}

#[derive(Clone, ValueEnum)]
pub enum LayerTarget {
    L2,
    L3,
    L4,
}

#[derive(Args, Clone)]
pub struct LayerCompileArgs {
    #[arg(long, conflicts_with = "source_root")]
    pub workspace: Option<String>,
    #[arg(long, default_value = "../workspaces/ws-dev")]
    pub source_root: PathBuf,
    #[arg(long = "app")]
    pub app_id: String,
    #[arg(long, value_enum)]
    pub layer: LayerTarget,
    #[arg(long)]
    pub target: Option<String>,
    #[arg(long, default_value_t = false)]
    pub continue_on_error: bool,
    #[arg(long, default_value_t = false)]
    pub json: bool,
}

#[derive(Args, Clone)]
pub struct LayerVerifyArgs {
    #[arg(long, conflicts_with = "source_root")]
    pub workspace: Option<String>,
    #[arg(long, default_value = "../workspaces/ws-dev")]
    pub source_root: PathBuf,
    #[arg(long = "app")]
    pub app_id: String,
    #[arg(long, default_value = "all")]
    pub layer: String,
    #[arg(long, default_value_t = false)]
    pub json: bool,
}

#[derive(Args, Clone)]
pub struct LayerInspectArgs {
    #[arg(long, conflicts_with = "source_root")]
    pub workspace: Option<String>,
    #[arg(long, default_value = "../workspaces/ws-dev")]
    pub source_root: PathBuf,
    #[arg(long = "app")]
    pub app_id: String,
    #[arg(long, value_enum)]
    pub layer: LayerTarget,
    #[arg(long)]
    pub node: Option<String>,
    #[arg(long, default_value_t = false)]
    pub json: bool,
}

#[derive(Args, Clone)]
pub struct LayerStatusArgs {
    #[arg(long, conflicts_with = "source_root")]
    pub workspace: Option<String>,
    #[arg(long, default_value = "../workspaces/ws-dev")]
    pub source_root: PathBuf,
    #[arg(long = "app")]
    pub app_id: String,
    #[arg(long, default_value_t = false)]
    pub json: bool,
}
