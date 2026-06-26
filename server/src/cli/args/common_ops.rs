use std::path::PathBuf;

use clap::{Args, Subcommand};

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

