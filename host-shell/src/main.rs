mod access_page_cache;
mod api_error;
mod api_stubs;
mod app_launch_api;
mod app_runtime_proxy;
mod app_runtime_supervisor;
mod app_surface;
mod apply_profile;
mod artifact_observability;
mod assets;
mod build_api;
mod build_fragment_cache;
mod build_info;
mod build_ops;
mod build_worker;
mod cache_diagnostics;
mod cli;
mod client_trace;
mod commands;
mod dev_eval_scope;
mod draft_session;
mod generation_lifecycle;
mod gis_config;
mod gis_proxy;
mod host_events;
mod host_home;
mod host_mcg;
mod host_runtime_hub;
mod host_scoped;
mod hot_reload;
mod http;
mod instance_api;
mod landing;
mod launch_targets;
mod legacy_compat;
mod light_pages;
mod managed_plug;
mod ops_api;
mod ops_config_api;
mod ops_theme_layout_api;
mod page_observability;
mod pages;
mod plug_proxy;
mod presentation_compile;
mod presentation_scripts;
mod request_logging;
mod review_axes;
mod route_lifecycle;
mod runtime_api;
mod runtime_snapshot;
mod scene_bundle;
mod scene_manifest;
mod shell_chrome;
mod shell_redirects;
mod startup;
mod startup_banner;
mod state;
mod thin_shell_page_cache;
mod tool_exec;
mod upload_api;
mod upload_support;
mod view_revision;
mod workspace_page;
mod workspace_profile_api;

use clap::{CommandFactory, Parser};

#[derive(Parser, Debug)]
#[command(
    name = "mei-host-shell",
    about = "MeiLang v2 host (workspace init / prebuild / import / reload / serve)"
)]
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
