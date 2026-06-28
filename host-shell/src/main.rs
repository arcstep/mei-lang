mod api_stubs;
mod assets;
mod build_info;
mod cli;
mod commands;
mod http;
mod pages;
mod request_logging;
mod state;

use clap::{CommandFactory, Parser};

#[derive(Parser, Debug)]
#[command(name = "mei-host-shell", about = "MeiLang v2 host shell (import / prebuild-data / warmup / serve)")]
struct Cli {
    /// Print shell binary build identity and exit
    #[arg(short = 'V', long = "version", global = true)]
    print_version: bool,

    #[command(subcommand)]
    command: Option<cli::Command>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(false)
        .compact()
        .init();
    if cli.print_version {
        return crate::build_info::print_cli_version(None, false);
    }
    let Some(command) = cli.command else {
        Cli::command().print_help()?;
        return Ok(());
    };
    commands::dispatch(command).await
}
