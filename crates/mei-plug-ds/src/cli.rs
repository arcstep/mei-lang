use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "mei-plug-ds",
    about = "MeiLang data-source plugin (warmup + datasets API)"
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
    Warmup(WarmupArgs),
    Serve(ServeArgs),
}

#[derive(Args, Debug)]
pub struct VersionArgs {
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct WarmupArgs {
    #[arg(long)]
    pub workspace: PathBuf,
    #[arg(long)]
    pub app: String,
    #[arg(long, default_value = "home")]
    pub policy: String,
    #[arg(long, default_value = "disk")]
    pub tier: String,
    #[arg(long)]
    pub board: Option<String>,
    #[arg(long)]
    pub frontier: Option<String>,
    #[arg(long, default_value_t = 0)]
    pub hops: usize,
}

#[derive(Args, Debug)]
pub struct ServeArgs {
    #[arg(long)]
    pub workspace: PathBuf,
    #[arg(long)]
    pub app: String,
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,
    #[arg(long, default_value = "9528")]
    pub port: u16,
}
