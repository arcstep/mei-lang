mod api_error;
mod api_stubs;
mod assets;
mod build_api;
mod build_info;
mod build_fragment_cache;
mod build_layout_tuning;
mod access_page_cache;
mod artifact_observability;
mod scene_bundle;
mod scene_manifest;
mod startup;
mod startup_banner;
mod page_observability;
mod build_ops;
mod cache_diagnostics;
mod cli;
mod commands;
mod gis_config;
mod gis_proxy;
mod landing;
mod http;
mod draft_session;
mod layout_tuning_draft;
mod layout_tuning_draft_store;
mod light_pages;
mod managed_plug;
mod ops_api;
mod ops_config_api;
mod ops_layout_tuning_api;
mod presentation_compile;
mod presentation_scripts;
mod review_axes;
mod runtime_api;
mod runtime_snapshot;
mod pages;
mod plug_proxy;
mod client_trace;
mod request_logging;
mod state;
mod tool_exec;
mod upload_api;
mod upload_support;

use clap::{CommandFactory, Parser};

#[derive(Parser, Debug)]
#[command(name = "mei-host-shell", about = "MeiLang v2 host (workspace init / prebuild / import / reload / serve)")]
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
        .with_ansi(true)
        .with_target(false)
        .with_writer(std::io::stderr)
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
