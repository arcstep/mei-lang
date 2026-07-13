use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use mei_bundle::{
    bundle_output_path, bundle_stats, compute_workspace_digest, exchange_from_outcome, read_bundle,
    write_bundle_from_outcome,
};
use mei_lower::compile_app;
use mei_lower::lower_path;
use mei_surface::surface_catalog;
use mei_syntax::v2::parse_v2_source_file;
use serde_json::Value as JsonValue;

#[derive(Parser)]
#[command(
    name = "mei-compiler",
    about = "MeiLang 2.0 compiler (.meibundle exchange output)"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Parse + lower a .mei file and print Decl IR JSON (v0)
    EmitDecl {
        #[arg(long)]
        file: PathBuf,
    },
    /// Compile app to .meibundle (default) or exchange JSON
    Compile {
        #[arg(long)]
        workspace: PathBuf,
        #[arg(long)]
        app: String,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long, value_enum, default_value_t = CompileFormat::Bundle)]
        format: CompileFormat,
        #[arg(long, default_value_t = false)]
        pretty: bool,
        /// Write `{app}.blocks.pretty.json` beside the bundle for debugging
        #[arg(long, default_value_t = false)]
        emit_debug: bool,
    },
    /// Inspect or summarize a .meibundle
    Bundle {
        #[command(subcommand)]
        command: BundleCommand,
    },
    /// Parse a .mei file and print AST JSON (debug)
    Parse {
        #[arg(long)]
        file: PathBuf,
        #[arg(long)]
        expand: bool,
    },
    /// List v0 built-in surface constructors
    DescribeSurface {
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum BundleCommand {
    /// Print blocks from a bundle (optionally filtered)
    Inspect {
        path: PathBuf,
        #[arg(long, default_value_t = false)]
        pretty: bool,
        #[arg(long)]
        kind: Option<String>,
    },
    /// Print manifest and compression sizes
    Stats { path: PathBuf },
}

#[derive(Clone, Copy, Debug, Default, ValueEnum)]
enum CompileFormat {
    #[default]
    Bundle,
    Json,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::EmitDecl { file } => emit_decl(&file),
        Command::Compile {
            workspace,
            app,
            out,
            format,
            pretty,
            emit_debug,
        } => compile_app_cmd(&workspace, &app, out.as_deref(), format, pretty, emit_debug),
        Command::Bundle { command } => match command {
            BundleCommand::Inspect { path, pretty, kind } => {
                bundle_inspect(&path, pretty, kind.as_deref())
            }
            BundleCommand::Stats { path } => bundle_stats_cmd(&path),
        },
        Command::Parse { file, expand } => parse_mei(&file, expand),
        Command::DescribeSurface { json } => describe_surface(json),
    }
}

fn compile_app_cmd(
    workspace: &Path,
    app: &str,
    out: Option<&Path>,
    format: CompileFormat,
    pretty: bool,
    emit_debug: bool,
) -> Result<()> {
    let outcome = compile_app(workspace, app).map_err(|error| anyhow::anyhow!("{error}"))?;
    let exchange = exchange_from_outcome(&outcome);

    match format {
        CompileFormat::Json => {
            let payload = serde_json::to_value(&exchange).context("serialize exchange")?;
            let text = if pretty {
                serde_json::to_string_pretty(&payload)?
            } else {
                serde_json::to_string(&payload)?
            };
            println!("{text}");
            Ok(())
        }
        CompileFormat::Bundle => {
            let templates_rel = read_templates_rel(workspace);
            let digest = compute_workspace_digest(workspace, app, templates_rel.as_str());
            let out_path = out
                .map(Path::to_path_buf)
                .unwrap_or_else(|| bundle_output_path(workspace, app));
            let stats = write_bundle_from_outcome(
                &outcome,
                digest.as_str(),
                env!("CARGO_PKG_VERSION"),
                out_path.as_path(),
                emit_debug,
            )
            .map_err(|e| anyhow::anyhow!("{e}"))?;
            println!(
                "wrote {} ({} blocks, bundle {} bytes, blocks json {} -> zstd {} bytes)",
                path_for_log(workspace, out_path.as_path()),
                stats.manifest.block_count,
                stats.bundle_bytes,
                stats.blocks_json_bytes,
                stats.blocks_zstd_bytes,
            );
            if emit_debug {
                let sidecar = out_path
                    .parent()
                    .unwrap_or_else(|| out_path.as_path())
                    .join(format!(
                        "{}.blocks.pretty.json",
                        out_path
                            .file_stem()
                            .and_then(|stem| stem.to_str())
                            .unwrap_or("bundle")
                    ));
                println!(
                    "debug sidecar: {}",
                    path_for_log(workspace, sidecar.as_path())
                );
            }
            Ok(())
        }
    }
}

fn bundle_inspect(path: &Path, pretty: bool, kind: Option<&str>) -> Result<()> {
    let (_manifest, blocks) = read_bundle(path).map_err(|e| anyhow::anyhow!("{e}"))?;
    let filtered: Vec<_> = match kind {
        Some(k) => blocks.into_iter().filter(|b| b.kind == k).collect(),
        None => blocks,
    };
    let payload = serde_json::to_value(&filtered).context("serialize blocks")?;
    let text = if pretty {
        serde_json::to_string_pretty(&payload)?
    } else {
        serde_json::to_string(&payload)?
    };
    println!("{text}");
    Ok(())
}

fn bundle_stats_cmd(path: &Path) -> Result<()> {
    let stats = bundle_stats(path).map_err(|e| anyhow::anyhow!("{e}"))?;
    let m = &stats.manifest;
    println!("bundle_schema_version: {}", m.bundle_schema_version);
    println!("compiler_version: {}", m.compiler_version);
    println!("app_id: {}", m.app_id);
    println!("syntax_version: {}", m.syntax_version);
    println!("block_count: {}", m.block_count);
    println!("workspace_digest: {}", m.workspace_digest);
    println!(
        "index_by_kind: {}",
        serde_json::to_string(&m.index_by_kind)?
    );
    println!("bundle_bytes: {}", stats.bundle_bytes);
    println!("blocks_json_bytes: {}", stats.blocks_json_bytes);
    println!("blocks_zstd_bytes: {}", stats.blocks_zstd_bytes);
    if stats.blocks_json_bytes > 0 {
        let ratio = (stats.blocks_zstd_bytes as f64) / (stats.blocks_json_bytes as f64) * 100.0;
        println!("zstd_ratio: {ratio:.1}%");
    }
    Ok(())
}

fn read_templates_rel(workspace: &Path) -> String {
    let workspace_json = mei_graph::resolve_workspace_config_path(workspace);
    let raw = std::fs::read_to_string(&workspace_json).unwrap_or_default();
    serde_json::from_str::<JsonValue>(&raw)
        .ok()
        .and_then(|v| {
            v.get("paths")
                .and_then(|p| p.get("templates"))
                .and_then(|t| t.as_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| "stock/templates".to_string())
}

fn parse_mei(file: &Path, expand: bool) -> Result<()> {
    let parsed = parse_v2_source_file(file).with_context(|| format!("parse {}", file.display()))?;
    let output: JsonValue = if expand {
        let workspace = file
            .ancestors()
            .find(|p| p.join("workspace.json").is_file())
            .context("could not locate workspace.json for macro expand")?;
        let templates_rel = read_templates_rel(workspace);
        let templates_root = workspace.join(&templates_rel);
        let app_root = file
            .ancestors()
            .find(|p| p.join("mei.lang.json").is_file() || p.join("app.config.json").is_file())
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| workspace.join("apps").join("_unknown"));
        let roots = mei_graph::TemplateRoots::from_app_and_stock(&app_root, templates_root);
        let registry = mei_graph::MacroRegistry::load_layered(&roots)?;
        let expanded = mei_graph::expand_v2_file(&parsed, &registry, &roots)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        serde_json::to_value(&expanded).context("serialize expanded v2")?
    } else {
        serde_json::to_value(&parsed).context("serialize v2 ast")?
    };
    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}

fn emit_decl(file: &Path) -> Result<()> {
    let outcome = lower_path(file).with_context(|| format!("lower {}", file.display()))?;
    let payload = JsonValue::Array(outcome.exports);
    println!(
        "{}",
        serde_json::to_string_pretty(&payload).context("serialize decl IR")?
    );
    Ok(())
}

fn describe_surface(json: bool) -> Result<()> {
    let items: Vec<_> = surface_catalog()
        .into_iter()
        .map(|entry| {
            serde_json::json!({
                "name": entry.name,
                "detail": entry.detail,
            })
        })
        .collect();
    if json {
        println!("{}", serde_json::to_string_pretty(&items)?);
    } else {
        for entry in items {
            println!("{} — {}", entry["name"], entry["detail"]);
        }
    }
    Ok(())
}

fn path_for_log(workspace: &Path, path: &Path) -> String {
    if let Ok(relative) = path.strip_prefix(workspace) {
        return relative.display().to_string();
    }
    if let (Ok(workspace), Ok(path)) = (workspace.canonicalize(), path.canonicalize()) {
        if let Ok(relative) = path.strip_prefix(&workspace) {
            return relative.display().to_string();
        }
    }
    path.display().to_string()
}
