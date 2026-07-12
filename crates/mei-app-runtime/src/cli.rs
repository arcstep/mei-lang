use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "mei-app-runtime",
    about = "MeiLang per-app App Runtime (embedded DS + Access + view/eval)"
)]
pub struct Cli {
    #[arg(short = 'V', long = "version", global = true)]
    pub print_version: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    Version(VersionArgs),
    Serve(ServeArgs),
}

#[derive(Args, Debug)]
pub struct VersionArgs {
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug, Clone)]
pub struct ServeArgs {
    #[arg(long)]
    pub workspace: PathBuf,

    #[arg(long)]
    pub app: String,

    /// Pinned generation. When omitted, follows `apps/{app}/env/current`.
    #[arg(long)]
    pub generation: Option<String>,

    #[arg(long)]
    pub instance_id: String,

    /// Optional JSON [`mei_host_core::InstanceSpec`] path.
    #[arg(long)]
    pub instance_spec: Option<PathBuf>,

    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,

    /// `0` = OS-assigned port; actual listen address printed as `MEI_APP_RUNTIME_LISTEN=host:port`.
    #[arg(long, default_value_t = 0)]
    pub port: u16,

    /// Internal instance token; requests must carry `x-mei-instance-token` (health excluded).
    #[arg(long)]
    pub token: String,
}
