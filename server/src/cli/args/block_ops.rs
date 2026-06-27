use std::path::PathBuf;

use clap::{Args, Subcommand};

#[derive(Args, Clone)]
pub struct BlockArgs {
    #[command(subcommand)]
    pub command: BlockCommand,
}

#[derive(Subcommand, Clone)]
pub enum BlockCommand {
    /// 单块 compile → MCG
    Compile(BlockCompileArgs),
    /// 单 bundle / slot verify
    Verify(BlockVerifyArgs),
    /// 单 scope eval（快反馈）
    Eval(BlockEvalArgs),
    /// 单 MRG slot / MCG node inspect（含完整失败链）
    Inspect(BlockInspectArgs),
    /// 列出 dirty / failed blocks
    List(BlockListArgs),
}

#[derive(Args, Clone)]
pub struct BlockCompileArgs {
    #[arg(long, conflicts_with = "source_root")]
    pub workspace: Option<String>,
    #[arg(long, default_value = "../workspaces/ws-dev")]
    pub source_root: PathBuf,
    #[arg(long = "app")]
    pub app_id: String,
    #[arg(long)]
    pub node: String,
    #[arg(long, default_value_t = false)]
    pub json: bool,
    #[arg(long = "assemble-only", default_value_t = false)]
    pub assemble_only: bool,
}

#[derive(Args, Clone)]
pub struct BlockVerifyArgs {
    #[arg(long, conflicts_with = "source_root")]
    pub workspace: Option<String>,
    #[arg(long, default_value = "../workspaces/ws-dev")]
    pub source_root: PathBuf,
    #[arg(long = "app")]
    pub app_id: String,
    #[arg(long)]
    pub node: String,
    #[arg(long, default_value_t = false)]
    pub json: bool,
}

#[derive(Args, Clone)]
pub struct BlockEvalArgs {
    #[arg(long, conflicts_with = "source_root")]
    pub workspace: Option<String>,
    #[arg(long, default_value = "../workspaces/ws-dev")]
    pub source_root: PathBuf,
    #[arg(long = "app")]
    pub app_id: String,
    #[arg(long)]
    pub scope: Option<String>,
    #[arg(long)]
    pub target: Option<String>,
    #[arg(long)]
    pub owner: String,
    #[arg(long)]
    pub metrics: Vec<String>,
    #[arg(long, default_value_t = false)]
    pub verbose: bool,
    #[arg(long, default_value_t = false)]
    pub json: bool,
}

#[derive(Args, Clone)]
pub struct BlockInspectArgs {
    #[arg(long, conflicts_with = "source_root")]
    pub workspace: Option<String>,
    #[arg(long, default_value = "../workspaces/ws-dev")]
    pub source_root: PathBuf,
    #[arg(long = "app")]
    pub app_id: String,
    #[arg(long)]
    pub node: String,
    #[arg(long, default_value_t = false)]
    pub json: bool,
}

#[derive(Args, Clone)]
pub struct BlockListArgs {
    #[arg(long, conflicts_with = "source_root")]
    pub workspace: Option<String>,
    #[arg(long, default_value = "../workspaces/ws-dev")]
    pub source_root: PathBuf,
    #[arg(long = "app")]
    pub app_id: String,
    #[arg(long, default_value = "stale,missing,failed")]
    pub state: String,
    #[arg(long, default_value_t = false)]
    pub json: bool,
}
