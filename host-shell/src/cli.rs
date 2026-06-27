use clap::{Args, Subcommand};
use std::path::PathBuf;

#[derive(Subcommand, Debug)]
pub enum Command {
    Import(ImportArgs),
    PrebuildData(PrebuildDataArgs),
    Warmup(WarmupArgs),
    Serve(ServeArgs),
}

#[derive(Args, Debug)]
pub struct PrebuildDataArgs {
    #[arg(long)]
    pub workspace: PathBuf,
    #[arg(long)]
    pub app: String,
}

#[derive(Args, Debug)]
pub struct ImportArgs {
    #[arg(long)]
    pub workspace: PathBuf,
    #[arg(long)]
    pub app: String,
    #[arg(long)]
    pub bundle: Option<PathBuf>,
}

#[derive(Args, Debug)]
pub struct WarmupArgs {
    #[arg(long)]
    pub workspace: PathBuf,
    #[arg(long)]
    pub app: String,
    #[arg(long, default_value = "home")]
    pub policy: String,
}

#[derive(Args, Debug)]
pub struct ServeArgs {
    #[arg(long)]
    pub workspace: PathBuf,
    #[arg(long)]
    pub app: String,
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,
    #[arg(long, default_value = "9527")]
    pub port: u16,
}
