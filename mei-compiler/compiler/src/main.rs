use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use mei_lower::lower_path;
use mei_lower::compile_v2_app;
use mei_surface::surface_catalog;
use mei_syntax::v2::parse_v2_source_file;
use serde_json::Value as JsonValue;

#[derive(Parser)]
#[command(name = "mei-compiler", about = "Mei surface compiler (Phase 0)")]
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
    /// Compile MeiLang 2.0 app to graph blocks JSON
    CompileV2 {
        #[arg(long)]
        workspace: PathBuf,
        #[arg(long)]
        app: String,
    },
    /// Parse a v2 .mei file and print AST JSON (debug)
    ParseV2 {
        #[arg(long)]
        file: PathBuf,
        #[arg(long)]
        expand: bool,
    },
    /// Compare native lower output with Starlark evaluate_mei_file for an app
    Check {
        #[arg(long)]
        workspace: PathBuf,
        #[arg(long)]
        app: String,
    },
    /// List v0 built-in surface constructors
    DescribeSurface {
        #[arg(long)]
        json: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::EmitDecl { file } => emit_decl(&file),
        Command::CompileV2 { workspace, app } => compile_v2(&workspace, &app),
        Command::ParseV2 { file, expand } => parse_v2(&file, expand),
        Command::Check { workspace, app } => check_app(&workspace, &app),
        Command::DescribeSurface { json } => describe_surface(json),
    }
}

fn compile_v2(workspace: &Path, app: &str) -> Result<()> {
    let outcome = compile_v2_app(workspace, app)
        .map_err(|error| anyhow::anyhow!("{error}"))?;
    println!(
        "{}",
        serde_json::to_string_pretty(&outcome).context("serialize graph outcome")?
    );
    Ok(())
}

fn parse_v2(file: &Path, expand: bool) -> Result<()> {
    let parsed = parse_v2_source_file(file).with_context(|| format!("parse {}", file.display()))?;
    let output: JsonValue = if expand {
        let workspace = file
            .ancestors()
            .find(|p| p.join("workspace.json").is_file())
            .context("could not locate workspace.json for macro expand")?;
        let ws_raw = std::fs::read_to_string(workspace.join("workspace.json"))?;
        let templates_rel = serde_json::from_str::<JsonValue>(&ws_raw)
            .ok()
            .and_then(|v| {
                v.get("paths")
                    .and_then(|p| p.get("templates"))
                    .and_then(|t| t.as_str())
                    .map(str::to_string)
            })
            .unwrap_or_else(|| "stock/templates".to_string());
        let templates_root = workspace.join(&templates_rel);
        let registry = mei_graph::MacroRegistry::load_dir(&templates_root)?;
        let expanded = mei_graph::expand_v2_file(&parsed, &registry, &templates_root)
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

fn check_app(workspace: &Path, app: &str) -> Result<()> {
    #[cfg(not(feature = "check"))]
    {
        let _ = (workspace, app);
        bail!("`check` requires building mei-compiler with `--features check` (links Starlark for golden diff)");
    }
    #[cfg(feature = "check")]
    check_app_with_starlark(workspace, app)
}

#[cfg(feature = "check")]
fn check_app_with_starlark(workspace: &Path, app: &str) -> Result<()> {
    let app_dir = workspace.join("apps").join(app);
    let main_mei = app_dir.join("src/main.mei");
    if !main_mei.is_file() {
        bail!("missing app entry: {}", main_mei.display());
    }

    let mut files = vec![main_mei.clone()];
    let scenes_dir = app_dir.join("src/scenes");
    if scenes_dir.is_dir() {
        for entry in std::fs::read_dir(&scenes_dir).context("read scenes dir")? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "mei") {
                files.push(path);
            }
        }
        files.sort();
    }

    for path in files {
        let starlark = evaluate_starlark(&path)?;
        let native = JsonValue::Array(
            lower_path(&path)
                .with_context(|| format!("native lower {}", path.display()))?
                .exports,
        );
        if !decl_ir_equal(&starlark, &native) {
            eprintln!("decl IR mismatch: {}", path.display());
            eprintln!(
                "starlark:\n{}",
                serde_json::to_string_pretty(&normalize_decl_ir(&starlark))?
            );
            eprintln!(
                "native:\n{}",
                serde_json::to_string_pretty(&normalize_decl_ir(&native))?
            );
            std::process::exit(1);
        }
        println!("ok {}", path.display());
    }
    Ok(())
}

#[cfg(feature = "check")]
fn evaluate_starlark(path: &Path) -> Result<JsonValue> {
    mei_lang_kernel::evaluate_mei_file(path)
        .with_context(|| format!("starlark evaluate {}", path.display()))
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

#[cfg_attr(not(feature = "check"), allow(dead_code))]
fn normalize_decl_ir(value: &JsonValue) -> JsonValue {
    match value {
        JsonValue::Object(map) => {
            let mut keys: Vec<_> = map.keys().cloned().collect();
            keys.sort();
            let mut out = serde_json::Map::new();
            for key in keys {
                if let Some(entry) = map.get(&key) {
                    out.insert(key, normalize_decl_ir(entry));
                }
            }
            JsonValue::Object(out)
        }
        JsonValue::Array(items) => JsonValue::Array(items.iter().map(normalize_decl_ir).collect()),
        other => other.clone(),
    }
}

#[cfg_attr(not(feature = "check"), allow(dead_code))]
fn decl_ir_equal(left: &JsonValue, right: &JsonValue) -> bool {
    normalize_decl_ir(left) == normalize_decl_ir(right)
}
