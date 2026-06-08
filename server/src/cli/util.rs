use std::path::PathBuf;

use anyhow::{Context, Result};
use mei_lang_kernel::{workspace_config_path, CompileOptions, CompileWatchedFile, Diagnostic, Severity};

use crate::http;
use mei_lang_toolchain as toolchain;
use serde::Serialize;
use serde_json::json;

use super::args::CliAppSelectorArgs;

pub fn resolve_package_root() -> Result<PathBuf> {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .context("server crate manifest has no parent directory")
        .map(std::path::Path::to_path_buf)
}

pub fn resolve_source_root_arg(
    package_root: &std::path::Path,
    workspace: Option<&str>,
    source_root: &PathBuf,
) -> Result<PathBuf> {
    if let Some(name) = workspace.map(str::trim).filter(|value| !value.is_empty()) {
        let root = package_root.join("../workspaces").join(name);
        let cfg = workspace_config_path(&root);
        if !cfg.is_file() {
            anyhow::bail!(
                "workspace profile `{name}` missing {}; expected under workspaces/{name}/",
                cfg.display()
            );
        }
        return Ok(root);
    }
    Ok(if source_root.is_absolute() {
        source_root.clone()
    } else {
        package_root.join(source_root)
    })
}

pub fn resolve_cli_source_root(package_root: &std::path::Path, raw: &PathBuf) -> Result<PathBuf> {
    let source_root = if raw.is_absolute() {
        raw.clone()
    } else {
        package_root.join(raw)
    };
    if !source_root.exists() {
        anyhow::bail!(
            "source root `{}` does not exist; create it first or pass a valid --source-root",
            source_root.display()
        );
    }
    if !source_root.is_dir() {
        anyhow::bail!(
            "source root `{}` is not a directory; pass a directory path to --source-root",
            source_root.display()
        );
    }
    source_root.canonicalize().with_context(|| {
        format!(
            "failed to canonicalize source root {}",
            source_root.display()
        )
    })
}

pub fn normalize_optional_arg(value: &Option<String>) -> Option<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

pub fn compile_options_from_selector(args: &CliAppSelectorArgs) -> CompileOptions {
    CompileOptions {
        scene: normalize_optional_arg(&args.scene),
        preview_target: normalize_optional_arg(&args.target_file),
    }
}

pub fn world_scope_from_selector(args: &CliAppSelectorArgs) -> Option<http::scene_api::WorldScope> {
    let scene_id = normalize_optional_arg(&args.scene);
    let target_file = normalize_optional_arg(&args.target_file);
    if scene_id.is_none() && target_file.is_none() {
        None
    } else {
        Some(http::scene_api::WorldScope {
            scene_id,
            target_file,
        })
    }
}

pub fn inspect_layout_for_app(source_root: &std::path::Path, app_id: &str) -> toolchain::SourceLayoutInspection {
    toolchain::inspect_source_layout(source_root, app_id)
}

pub fn ensure_cli_layout_ready(layout: &toolchain::SourceLayoutInspection) -> Result<()> {
    let errors: Vec<&toolchain::LayoutCheck> = layout
        .checks
        .iter()
        .filter(|item| item.level == "error")
        .collect();
    if errors.is_empty() {
        return Ok(());
    }
    let summary = errors
        .iter()
        .map(|item| {
            if let Some(hint) = item.hint.as_deref() {
                format!("- {} ({hint})", item.message)
            } else {
                format!("- {}", item.message)
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    anyhow::bail!(
        "layout checks failed for app `{}`:\n{}\nRun `mei inspect layout --app {} --source-root {}` for full report.",
        layout.app_id,
        summary,
        layout.app_id,
        layout.roots.source_root
    );
}

pub fn attach_layout_to_envelope(
    envelope: &mut toolchain::HeadlessArtifactEnvelope,
    layout: &toolchain::SourceLayoutInspection,
) -> Result<()> {
    let layout_value = serde_json::to_value(layout)?;
    if let Some(obj) = envelope.artifact.as_object_mut() {
        obj.insert("layout".to_string(), layout_value);
    } else {
        let current = envelope.artifact.clone();
        envelope.artifact = json!({
            "value": current,
            "layout": layout_value,
        });
    }
    Ok(())
}

pub fn parse_cli_filters(filters: &[String]) -> Result<std::collections::BTreeMap<String, String>> {
    let mut out = std::collections::BTreeMap::new();
    for item in filters {
        let raw = item.trim();
        let Some((key, value)) = raw.split_once('=') else {
            anyhow::bail!("invalid --filter `{raw}`; expected key=value");
        };
        let key = key.trim();
        let value = value.trim();
        if key.is_empty() || value.is_empty() {
            anyhow::bail!("invalid --filter `{raw}`; expected non-empty key=value");
        }
        out.insert(key.to_string(), value.to_string());
    }
    Ok(out)
}

pub fn diagnostics_summary(diagnostics: &[Diagnostic]) -> serde_json::Value {
    let mut errors = 0usize;
    let mut warnings = 0usize;
    let mut infos = 0usize;
    for item in diagnostics {
        match item.severity {
            Severity::Error => errors += 1,
            Severity::Warning => warnings += 1,
            Severity::Info => infos += 1,
        }
    }
    json!({
        "errors": errors,
        "warnings": warnings,
        "infos": infos,
    })
}

pub fn watched_files_json(files: &[CompileWatchedFile]) -> Vec<serde_json::Value> {
    files.iter()
        .map(|item| {
            json!({
                "rel_path": item.rel_path,
                "modified_ms": item.modified_ms,
                "size_bytes": item.size_bytes,
            })
        })
        .collect()
}

pub fn scope_json(scope: Option<&http::scene_api::WorldScope>) -> serde_json::Value {
    match scope {
        Some(scope) => json!({
            "scene_id": scope.scene_id,
            "target_file": scope.target_file,
        }),
        None => serde_json::Value::Null,
    }
}

pub fn print_json_output<T: Serialize>(value: &T, pretty: bool) -> Result<()> {
    if pretty {
        println!("{}", serde_json::to_string_pretty(value)?);
    } else {
        println!("{}", serde_json::to_string(value)?);
    }
    Ok(())
}

pub fn print_cli_version_if_requested() -> bool {
    matches!(
        std::env::args().nth(1).as_deref(),
        Some("-V") | Some("--version")
    )
}
