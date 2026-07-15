use std::path::PathBuf;

use clap::{Parser, Subcommand};
use mei_snapshot::{
    pack_portable_snapshot, pack_snapshot, unpack_snapshot, PackOptions, PortablePackOptions,
};

#[derive(Parser, Debug)]
#[command(name = "mei-snapshot", about = "Pack/unpack Mei Viewer snapshot archives")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Pack workspace app build artifacts into a .mei-snapshot.zip (v1 by default)
    Pack {
        #[arg(long)]
        workspace: PathBuf,
        /// Single app (v1) or repeatable with --portable
        #[arg(long)]
        app: Vec<String>,
        #[arg(long)]
        out: PathBuf,
        #[arg(long, default_value_t = false)]
        include_data: bool,
        #[arg(long, default_value_t = false)]
        include_cache: bool,
        /// Emit portable multi-app snapshot (formatVersion 2)
        #[arg(long, default_value_t = false)]
        portable: bool,
        #[arg(long, default_value_t = false)]
        include_media: bool,
        #[arg(long)]
        package_root: Option<PathBuf>,
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
            portable,
            include_media,
            package_root,
            default_scene,
            compiler_version,
            json,
        } => {
            if app.is_empty() {
                anyhow::bail!("at least one --app is required");
            }
            let manifest = if portable || app.len() > 1 {
                pack_portable_snapshot(&PortablePackOptions {
                    workspace,
                    app_ids: app,
                    out: out.clone(),
                    default_scene,
                    compiler_version,
                    workspace_label: None,
                    package_root,
                    include_media,
                })?
            } else {
                pack_snapshot(&PackOptions {
                    workspace,
                    app_id: app.into_iter().next().unwrap(),
                    out: out.clone(),
                    include_data,
                    include_cache,
                    default_scene,
                    compiler_version,
                })?
            };
            if json {
                println!("{}", serde_json::to_string_pretty(&manifest)?);
            } else {
                println!(
                    "packed {} v{} (app={}, apps={}, files={}, hint={}) -> {}",
                    manifest.format,
                    manifest.format_version,
                    manifest.app_id,
                    manifest.apps.len().max(1),
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
                        "appBundlePaths": result.app_bundle_paths.iter().map(|(id, p)| {
                            serde_json::json!({ "appId": id, "path": p })
                        }).collect::<Vec<_>>(),
                        "manifest": result.manifest,
                    })
                );
            } else {
                println!(
                    "unpacked v{} app={} bundles={} -> {}",
                    result.manifest.format_version,
                    result.manifest.app_id,
                    result.app_bundle_paths.len(),
                    result.dest.display()
                );
            }
        }
    }
    Ok(())
}
