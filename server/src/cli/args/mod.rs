
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(about = "MeiLang skeleton server", long_about = None)]
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
    Prebuild(PrebuildArgs),
    Readiness(ReadinessArgs),
    Diagnostics(DiagnosticsArgs),
    Warmup(WarmupArgs),
    Compile(CheckArgs),
    Check(CheckArgs),
    Inspect(InspectArgs),
    Export(ExportArgs),
    Query(QueryArgs),
    Runtime(RuntimeArgs),
    Mcp(McpArgs),
}

mod common_ops; mod host_workspace; mod inspect_export; mod query_agent;
pub use common_ops::*; pub use host_workspace::*; pub use inspect_export::*; pub use query_agent::*;
