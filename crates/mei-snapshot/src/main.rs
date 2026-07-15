use std::path::PathBuf;

use clap::{Parser, Subcommand};
use mei_snapshot::{pack_snapshot, unpack_snapshot, PackOptions};

#[derive(Parser, Debug)]
#[command(name = "mei-snapshot", about = "Pack/unpack Mei Viewer snapshot archives")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Pack workspace app build artifacts into a .mei-snapshot.zip
    Pack {
        #[arg(long)]
        workspace: PathBuf,
        #[arg(long)]
        app: String,
        #[arg(long)]
        out: PathBuf,
        #[arg(long, default_value_t = false)]
        include_data: bool,
        #[arg(long, default_value_t = false)]
        include_cache: bool,
        #[arg(long)]
        default_scene: Option<String>,
        #[arg(long)]
        compiler_version: Option<String>,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    /// Unpack a .mei-snapshot.zip into an empty directory
    Unpack {
        #[arg(long)]
        archive: PathBuf,
        #[arg(long)]
        into: PathBuf,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Pack {
            workspace,
            app,
            out,
            include_data,
            include_cache,
            default_scene,
            compiler_version,
            json,
        } => {
            let manifest = pack_snapshot(&PackOptions {
                workspace,
                app_id: app,
                out: out.clone(),
                include_data,
                include_cache,
                default_scene,
                compiler_version,
            })?;
            if json {
                println!("{}", serde_json::to_string_pretty(&manifest)?);
            } else {
                println!(
                    "packed {} (app={}, files={}, hint={}) -> {}",
                    manifest.format,
                    manifest.app_id,
                    manifest.files.len(),
                    manifest.data_mode_hint.as_str(),
                    out.display()
                );
            }
        }
        Commands::Unpack {
            archive,
            into,
            json,
        } => {
            let result = unpack_snapshot(&archive, &into)?;
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "dest": result.dest,
                        "bundlePath": result.bundle_path,
                        "manifest": result.manifest,
                    })
                );
            } else {
                println!(
                    "unpacked app={} bundle={} -> {}",
                    result.manifest.app_id,
                    result.bundle_path.display(),
                    result.dest.display()
                );
            }
        }
    }
    Ok(())
}
