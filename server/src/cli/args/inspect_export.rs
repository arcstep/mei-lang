
use clap::{Args, Subcommand};

use super::common_ops::CliAppSelectorArgs;

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
