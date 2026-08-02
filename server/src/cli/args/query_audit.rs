use std::path::PathBuf;

use clap::{Args, Subcommand};

#[derive(Args, Clone)]
pub struct QueryAuditArgs {
    #[command(subcommand)]
    pub command: QueryAuditCommand,
}

#[derive(Subcommand, Clone)]
pub enum QueryAuditCommand {
    /// List recent audit rows (table or `--json`)
    Tail(QueryAuditTailArgs),
    /// Show one audit row by `audit_id`
    Explain(QueryAuditExplainArgs),
    /// Exit 1 if any controlled=false or shape exceeds 0549 budgets
    Gate(QueryAuditGateArgs),
    /// Re-run saved SQL file N times; print p50/p95 exec_ms
    Replay(QueryAuditReplayArgs),
    /// Write a markdown summary from the day's JSONL
    Report(QueryAuditReportArgs),
}

#[derive(Args, Clone)]
pub struct QueryAuditCommonArgs {
    /// Workspace profile under `workspaces/<name>/` (conflicts with `--source-root`)
    #[arg(long, conflicts_with = "source_root")]
    pub workspace: Option<String>,
    #[arg(long, default_value = "../workspaces/ws-dev")]
    pub source_root: PathBuf,
    #[arg(long)]
    pub app: String,
    /// Override audit root (`…/query-audit`). Default: `resolve_app_var_root(app)/query-audit`
    #[arg(long)]
    pub var_root: Option<PathBuf>,
    /// Calendar day `YYYYMMDD` (default: today)
    #[arg(long)]
    pub day: Option<String>,
}

#[derive(Args, Clone)]
pub struct QueryAuditTailArgs {
    #[command(flatten)]
    pub common: QueryAuditCommonArgs,
    #[arg(long)]
    pub metric: Option<String>,
    #[arg(long, default_value_t = 50)]
    pub limit: usize,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Clone)]
pub struct QueryAuditExplainArgs {
    #[command(flatten)]
    pub common: QueryAuditCommonArgs,
    #[arg(long)]
    pub id: String,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Clone)]
pub struct QueryAuditGateArgs {
    #[command(flatten)]
    pub common: QueryAuditCommonArgs,
    #[arg(long)]
    pub metric: Option<String>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Clone)]
pub struct QueryAuditReplayArgs {
    #[command(flatten)]
    pub common: QueryAuditCommonArgs,
    #[arg(long)]
    pub id: String,
    #[arg(long = "bench", default_value_t = 5)]
    pub bench: usize,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Clone)]
pub struct QueryAuditReportArgs {
    #[command(flatten)]
    pub common: QueryAuditCommonArgs,
    #[arg(long)]
    pub metric: Option<String>,
    #[arg(
        long,
        default_value = "../docs/draft/mei-lang/2026-08-02-zhifa-df-sql-audit-report.md"
    )]
    pub out: PathBuf,
}
