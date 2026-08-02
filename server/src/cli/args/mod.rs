use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    about = "MeiLang skeleton server",
    long_about = None,
    version
)]
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
    Graph(GraphArgs),
    Layer(LayerArgs),
    Block(BlockArgs),
    Scope(ScopeArgs),
    Prebuild(PrebuildArgs),
    Readiness(ReadinessArgs),
    Diagnostics(DiagnosticsArgs),
    Warmup(WarmupArgs),
    Compile(CheckArgs),
    Check(CheckArgs),
    Inspect(InspectArgs),
    Export(ExportArgs),
    Query(QueryArgs),
    QueryAudit(QueryAuditArgs),
    Runtime(RuntimeArgs),
    Mcp(McpArgs),
}

mod block_ops;
mod common_ops;
mod graph_ops;
mod host_workspace;
mod inspect_export;
mod layer_ops;
mod query_agent;
mod query_audit;
pub use block_ops::*;
pub use common_ops::*;
pub use graph_ops::*;
pub use host_workspace::*;
pub use inspect_export::*;
pub use layer_ops::*;
pub use query_agent::*;
pub use query_audit::*;
