use super::prelude::*;

use super::types::BinaryFlavor;

pub(crate) fn ensure_command_allowed(flavor: BinaryFlavor, command: &Command) -> Result<()> {
    if flavor == BinaryFlavor::Compat {
        let command_name = match command {
            Command::Serve(_) => "serve",
            Command::Agent(_) => "agent",
            Command::Host(args) => match &args.command {
                HostCommand::Describe(_) => "host describe",
                HostCommand::Auth(_) => "host auth",
            },
            Command::Workspace(_) => "workspace",
            Command::Knowledge(_) => "knowledge",
            Command::EditorRuntime(_) => "editor-runtime",
            Command::Graph(_) => "graph",
            Command::Layer(_) => "layer",
            Command::Block(_) => "block",
            Command::Scope(_) => "scope",
            Command::Prebuild(_) => "prebuild",
            Command::Readiness(_) => "readiness",
            Command::Diagnostics(_) => "diagnostics",
            Command::Warmup(_) => "warmup",
            Command::Compile(_) => "compile",
            Command::Check(_) => "check",
            Command::Inspect(_) => "inspect",
            Command::Export(_) => "export",
            Command::Query(_) => "query",
            Command::Runtime(_) => "runtime",
            Command::Mcp(_) => "mcp",
        };
        anyhow::bail!(
            "the `mei` compatibility entrypoint is retired; use `mei-toolchain` for `{}` or `mei-host-web` for host commands",
            command_name
        );
    }
    let allowed = match flavor {
        BinaryFlavor::Compat => false,
        BinaryFlavor::Toolchain => matches!(
            command,
            Command::Workspace(_)
                | Command::Knowledge(_)
                | Command::EditorRuntime(_)
                | Command::Graph(_)
                | Command::Layer(_)
                | Command::Block(_)
                | Command::Scope(_)
                | Command::Prebuild(_)
                | Command::Readiness(_)
                | Command::Diagnostics(_)
                | Command::Warmup(_)
                | Command::Compile(_)
                | Command::Check(_)
                | Command::Inspect(_)
                | Command::Export(_)
                | Command::Query(_)
                | Command::Runtime(_)
                | Command::Mcp(_)
        ),
        BinaryFlavor::HostWeb => matches!(
            command,
            Command::Serve(_) | Command::Agent(_) | Command::Host(_)
        ),
    };
    if allowed {
        return Ok(());
    }
    let hint = match flavor {
        BinaryFlavor::Compat => "mei-toolchain",
        BinaryFlavor::Toolchain => "mei-host-web",
        BinaryFlavor::HostWeb => "mei-toolchain",
    };
    let role = match flavor {
        BinaryFlavor::Compat => "retired compatibility entrypoint",
        BinaryFlavor::Toolchain => "toolchain-only entrypoint",
        BinaryFlavor::HostWeb => "host-web-only entrypoint",
    };
    let command_name = match command {
        Command::Serve(_) => "serve",
        Command::Agent(_) => "agent",
        Command::Host(args) => match &args.command {
            HostCommand::Describe(_) => "host describe",
            HostCommand::Auth(_) => "host auth",
        },
        Command::Workspace(_) => "workspace",
        Command::Knowledge(_) => "knowledge",
        Command::EditorRuntime(_) => "editor-runtime",
        Command::Graph(_) => "graph",
        Command::Layer(_) => "layer",
        Command::Block(_) => "block",
        Command::Scope(_) => "scope",
        Command::Prebuild(_) => "prebuild",
        Command::Readiness(_) => "readiness",
        Command::Diagnostics(_) => "diagnostics",
        Command::Warmup(_) => "warmup",
        Command::Compile(_) => "compile",
        Command::Check(_) => "check",
        Command::Inspect(_) => "inspect",
        Command::Export(_) => "export",
        Command::Query(_) => "query",
        Command::Runtime(_) => "runtime",
        Command::Mcp(_) => "mcp",
    };
    anyhow::bail!(
        "`{}` does not expose `{}` under the current split; use `{}`",
        role,
        command_name,
        hint
    )
}

pub async fn run_cli_for_flavor(flavor: BinaryFlavor) -> Result<()> {
    if print_cli_version_if_requested() {
        println!(
            "{} {} ({})",
            flavor.display_name(),
            crate::build_info::BUILD_VERSION,
            crate::build_info::BUILD_TARGET_TAG
        );
        return Ok(());
    }
    let cli = Cli::parse();
    ensure_command_allowed(flavor, &cli.command)?;
    let package_root = resolve_package_root()?;
    set_mei_package_root(package_root.clone());
    let default_filter = match cli.command {
        Command::Serve(_) => "warn,mei_lang_server=info",
        _ => "error",
    };
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(default_filter));
    if matches!(cli.command, Command::Serve(_)) {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(tracing_subscriber::fmt::layer().with_target(false).compact())
            .with(crate::http::host_log::HostLogLayer)
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(env_filter)
            .with_target(false)
            .compact()
            .init();
    }
    match cli.command {
        Command::Serve(args) => super::startup::serve(args).await,
        Command::Agent(args) => agent_command(AgentRuntimeArgs {
            command: args.command,
        }),
        Command::Host(args) => host_command(args),
        Command::Workspace(args) => workspace_command(args),
        Command::Knowledge(args) => knowledge_command(args),
        Command::EditorRuntime(args) => editor_runtime_command(args),
        Command::Graph(args) => graph_command(args),
        Command::Layer(args) => layer_command(args),
        Command::Block(args) => block_command(args),
        Command::Scope(args) => scope_command(args),
        Command::Prebuild(args) => prebuild_command(args),
        Command::Readiness(args) => readiness_command(args),
        Command::Diagnostics(args) => diagnostics_command(args),
        Command::Warmup(args) => warmup_command(args),
        Command::Compile(args) => compile_or_check_command("compile", args),
        Command::Check(args) => compile_or_check_command("check", args),
        Command::Inspect(args) => inspect_command(args),
        Command::Export(args) => export_command(args),
        Command::Query(args) => query_command(args),
        Command::Runtime(args) => runtime_command(args),
        Command::Mcp(args) => mcp_command(args),
    }
}
