use std::path::PathBuf;

use anyhow::{Context, Result};
use mei_lang_kernel::{
    workspace_config_path, CompileOptions, CompileWatchedFile, Diagnostic, Severity,
};

use mei_lang_toolchain as toolchain;
use serde::Serialize;
use serde_json::json;

use super::args::CliAppSelectorArgs;

fn package_root_from_env() -> Option<PathBuf> {
    std::env::var("MEI_PACKAGE_ROOT")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn package_root_from_current_exe() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let bin_dir = exe.parent()?;
    let prefix = bin_dir.parent()?.canonicalize().ok()?;
    let share_root = prefix.join("share/mei");
    if share_root.join("stock").is_dir() {
        return Some(share_root);
    }
    let candidate = prefix.parent()?.canonicalize().ok()?;
    let looks_like_package_root = candidate.join("stock").is_dir()
        || candidate.join("app").is_dir()
        || candidate.join("guides").is_dir();
    looks_like_package_root.then_some(candidate)
}

fn source_root_from_env() -> Option<PathBuf> {
    std::env::var("MEI_SOURCE_ROOT")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

pub fn resolve_package_root() -> Result<PathBuf> {
    if let Some(path) = package_root_from_env().filter(|path| path.exists()) {
        return path.canonicalize().with_context(|| {
            format!("failed to canonicalize MEI_PACKAGE_ROOT {}", path.display())
        });
    }
    if let Some(path) = package_root_from_current_exe() {
        return Ok(path);
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .context("server crate manifest has no parent directory")
        .map(std::path::Path::to_path_buf)
}

pub fn resolve_cargo_package_root(source_root: &std::path::Path) -> Result<PathBuf> {
    if let Some(path) = package_root_from_env().filter(|path| path.exists()) {
        return path.canonicalize().with_context(|| {
            format!("failed to canonicalize MEI_PACKAGE_ROOT {}", path.display())
        });
    }
    if let Ok(raw) = std::env::var("MEI_LANG_ROOT") {
        let candidate = PathBuf::from(raw.trim());
        if candidate.join("Cargo.toml").is_file() {
            return candidate.canonicalize().with_context(|| {
                format!("failed to canonicalize MEI_LANG_ROOT {}", candidate.display())
            });
        }
    }
    let sibling = source_root
        .parent()
        .and_then(|parent| parent.parent())
        .map(|grand| grand.join("mei-lang"));
    if let Some(candidate) = sibling.filter(|path| path.join("Cargo.toml").is_file()) {
        return candidate.canonicalize().with_context(|| {
            format!("failed to canonicalize mei-lang root {}", candidate.display())
        });
    }
    resolve_package_root()
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

pub fn resolve_optional_cli_source_root(
    package_root: &std::path::Path,
    raw: Option<&PathBuf>,
) -> Result<Option<PathBuf>> {
    if let Some(raw) = raw {
        return resolve_cli_source_root(package_root, raw).map(Some);
    }
    if let Some(raw) = source_root_from_env() {
        return resolve_cli_source_root(package_root, &raw).map(Some);
    }
    Ok(None)
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
        ..Default::default()
    }
}

pub fn world_scope_from_selector(args: &CliAppSelectorArgs) -> Option<toolchain::WorldScope> {
    let scene_id = normalize_optional_arg(&args.scene);
    let target_file = normalize_optional_arg(&args.target_file);
    if scene_id.is_none() && target_file.is_none() {
        None
    } else {
        Some(toolchain::WorldScope {
            scene_id,
            target_file,
        })
    }
}

pub fn inspect_layout_for_app(
    source_root: &std::path::Path,
    app_id: &str,
) -> toolchain::SourceLayoutInspection {
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
    files
        .iter()
        .map(|item| {
            json!({
                "rel_path": item.rel_path,
                "modified_ms": item.modified_ms,
                "size_bytes": item.size_bytes,
            })
        })
        .collect()
}

pub fn scope_json(scope: Option<&toolchain::WorldScope>) -> serde_json::Value {
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
