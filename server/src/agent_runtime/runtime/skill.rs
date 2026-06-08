use std::{
    fs,
    path::{Path as FsPath, PathBuf},
    process::Command as ProcessCommand,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::Context;
use walkdir::WalkDir;

use super::super::{ManagedOpencodeSkillMeta, ManagedOpencodeSkillStatus};
use crate::AppState;

const MANAGED_SKILL_SOURCE_REL: &str = "guides/claude-skills";
const MANAGED_SKILL_INSTALL_REL: &str = ".mei/skills/meilang-author";
fn managed_skill_source_dir(package_root: &FsPath) -> PathBuf {
    package_root.join(MANAGED_SKILL_SOURCE_REL)
}

fn managed_skill_install_dir(source_root: &FsPath) -> PathBuf {
    source_root.join(MANAGED_SKILL_INSTALL_REL)
}

fn unix_timestamp_ms(value: SystemTime) -> Option<u128> {
    value
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|dur| dur.as_millis())
}

fn directory_latest_modified_ms(path: &FsPath) -> Option<u128> {
    if !path.exists() {
        return None;
    }
    let mut latest = fs::metadata(path)
        .ok()
        .and_then(|meta| meta.modified().ok())
        .and_then(unix_timestamp_ms);
    for entry in WalkDir::new(path).into_iter().flatten() {
        let modified = entry
            .metadata()
            .ok()
            .and_then(|meta| meta.modified().ok())
            .and_then(unix_timestamp_ms);
        if modified > latest {
            latest = modified;
        }
    }
    latest
}

fn markdown_file_count(path: &FsPath) -> usize {
    if !path.exists() {
        return 0;
    }
    WalkDir::new(path)
        .into_iter()
        .flatten()
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| entry.path().extension().and_then(|ext| ext.to_str()) == Some("md"))
        .count()
}

fn markdown_files(path: &FsPath) -> Vec<String> {
    if !path.exists() {
        return Vec::new();
    }
    let mut files = WalkDir::new(path)
        .into_iter()
        .flatten()
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| entry.path().extension().and_then(|ext| ext.to_str()) == Some("md"))
        .filter_map(|entry| {
            entry
                .path()
                .strip_prefix(path)
                .ok()
                .and_then(|value| value.to_str())
                .map(|value| value.replace('\\', "/"))
        })
        .collect::<Vec<_>>();
    files.sort();
    files
}

fn git_revision_short(package_root: &FsPath) -> Option<String> {
    let output = ProcessCommand::new("git")
        .arg("-C")
        .arg(package_root)
        .arg("rev-parse")
        .arg("--short")
        .arg("HEAD")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let revision = String::from_utf8(output.stdout).ok()?;
    let revision = revision.trim();
    if revision.is_empty() {
        None
    } else {
        Some(revision.to_string())
    }
}

fn copy_skill_tree(source_dir: &FsPath, install_dir: &FsPath) -> anyhow::Result<()> {
    if install_dir.exists() {
        fs::remove_dir_all(install_dir).with_context(|| {
            format!(
                "failed to reset installed skill directory {}",
                install_dir.display()
            )
        })?;
    }
    fs::create_dir_all(install_dir).with_context(|| {
        format!(
            "failed to create installed skill directory {}",
            install_dir.display()
        )
    })?;
    for entry in WalkDir::new(source_dir).into_iter().flatten() {
        let source_path = entry.path();
        let Some(relative) = source_path.strip_prefix(source_dir).ok() else {
            continue;
        };
        let target_path = install_dir.join(relative);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&target_path)
                .with_context(|| format!("failed to create {}", target_path.display()))?;
            continue;
        }
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        fs::copy(source_path, &target_path).with_context(|| {
            format!(
                "failed to copy skill file {} -> {}",
                source_path.display(),
                target_path.display()
            )
        })?;
    }
    Ok(())
}

fn build_skill_status(package_root: &FsPath, source_root: &FsPath) -> ManagedOpencodeSkillStatus {
    let source_dir = managed_skill_source_dir(package_root);
    let install_dir = managed_skill_install_dir(source_root);
    let entry_file = install_dir.join("SKILL.md");
    let source_present = source_dir.join("SKILL.md").exists();
    let installed = entry_file.exists();
    let source_updated_at_ms = directory_latest_modified_ms(&source_dir);
    let install_updated_at_ms = directory_latest_modified_ms(&install_dir);
    let stale = source_present
        && installed
        && source_updated_at_ms
            .zip(install_updated_at_ms)
            .is_some_and(|(source_ms, install_ms)| source_ms > install_ms);
    ManagedOpencodeSkillStatus {
        source_dir: source_dir.display().to_string(),
        install_dir: install_dir.display().to_string(),
        entry_file: entry_file.display().to_string(),
        source_present,
        installed,
        stale,
        source_updated_at_ms,
        install_updated_at_ms,
        file_count: markdown_file_count(if installed { &install_dir } else { &source_dir }),
        revision: git_revision_short(package_root),
    }
}

pub(crate) fn managed_agent_skill_status_for_root(
    package_root: &FsPath,
    source_root: &FsPath,
) -> ManagedOpencodeSkillStatus {
    build_skill_status(package_root, source_root)
}

pub(crate) fn managed_agent_skill_status(
    state: &AppState,
) -> anyhow::Result<ManagedOpencodeSkillStatus> {
    Ok(build_skill_status(&state.package_root, &state.source_root))
}

pub(crate) fn sync_managed_agent_skill_for_root(
    package_root: &FsPath,
    source_root: &FsPath,
) -> anyhow::Result<ManagedOpencodeSkillStatus> {
    let source_dir = managed_skill_source_dir(package_root);
    let source_entry = source_dir.join("SKILL.md");
    if !source_entry.exists() {
        anyhow::bail!(
            "MeiLang skill source is missing: {}",
            source_entry.display()
        );
    }
    let install_dir = managed_skill_install_dir(source_root);
    copy_skill_tree(&source_dir, &install_dir)?;
    Ok(build_skill_status(package_root, source_root))
}

pub(crate) fn sync_managed_agent_skill(
    state: &AppState,
) -> anyhow::Result<ManagedOpencodeSkillStatus> {
    sync_managed_agent_skill_for_root(&state.package_root, &state.source_root)
}

pub(crate) fn ensure_managed_agent_skill_synced(
    state: &AppState,
) -> anyhow::Result<ManagedOpencodeSkillStatus> {
    let status = build_skill_status(&state.package_root, &state.source_root);
    if !status.source_present {
        return Ok(status);
    }
    if status.installed && !status.stale {
        return Ok(status);
    }
    sync_managed_agent_skill_for_root(&state.package_root, &state.source_root)
}

/// 解析 meilang-author skill 根目录（已安装优先，否则源码目录）。
/// 默认安装路径为 `{source_root}/.mei/skills/meilang-author`。
pub(crate) fn resolve_meilang_skill_home_for_source_root(
    package_root: &FsPath,
    source_root: &FsPath,
) -> Option<PathBuf> {
    let status = build_skill_status(package_root, source_root);
    if status.installed {
        Some(PathBuf::from(&status.install_dir))
    } else if status.source_present {
        Some(PathBuf::from(&status.source_dir))
    } else {
        None
    }
}

pub(crate) fn load_managed_agent_skill_meta(
    state: &AppState,
) -> anyhow::Result<Option<ManagedOpencodeSkillMeta>> {
    let Some(home) =
        resolve_meilang_skill_home_for_source_root(&state.package_root, &state.source_root)
    else {
        return Ok(None);
    };
    let status = build_skill_status(&state.package_root, &state.source_root);
    let source_kind = if status.installed {
        "installed"
    } else {
        "source"
    };
    let companion_files = markdown_files(&home)
        .into_iter()
        .filter(|file| file != "SKILL.md")
        .collect::<Vec<_>>();
    Ok(Some(ManagedOpencodeSkillMeta {
        skill_home: home.display().to_string(),
        source_kind: source_kind.to_string(),
        companion_files,
    }))
}
