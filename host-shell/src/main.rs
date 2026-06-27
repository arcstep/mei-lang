mod api_stubs;
mod assets;
mod build_info;
mod cli;
mod commands;
mod http;
mod pages;
mod request_logging;
mod state;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "mei-host-shell", about = "MeiLang v2 host shell (import / prebuild-data / warmup / serve)")]
struct Cli {
    #[command(subcommand)]
    command: cli::Command,
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
    commands::dispatch(cli.command).await
}
