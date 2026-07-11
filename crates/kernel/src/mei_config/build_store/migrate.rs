use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::mei_config::types::{
    LEGACY_WORKSPACE_RUNTIME_DIR_REL, TOOLCHAIN_ACTIVE_REL, TOOLCHAIN_STORE_REL,
    WORKSPACE_AGENT_LOCAL_DIR_REL, WORKSPACE_HOSTS_DIR_REL, WORKSPACE_PLATFORM_DIR_REL,
};
use crate::mei_config::workspace_paths::resolve_toolchain_root;

use super::lifecycle::set_active_symlink;
use super::paths::resolve_app_build_root_following_active;
use super::types::{read_links_state, write_links_state};

pub fn migrate_legacy_app_mei(app_root: &Path) -> Result<()> {
    let legacy = app_root.join(".mei");
    if !legacy.is_dir() {
        return Ok(());
    }
    let build_root = resolve_app_build_root_following_active(app_root);
    for sub in ["prebuild", "graph"] {
        let from = legacy.join(sub);
        if !from.is_dir() {
            continue;
        }
        let to = build_root.join(sub);
        merge_dir_recursive(&from, &to)?;
    }
    fs::remove_dir_all(&legacy).ok();
    Ok(())
}

/// 一次性迁移工作区根级 legacy `.mei/` → `runtime/hosts`、`runtime/agent`、`runtime/platform/`。
pub fn migrate_legacy_workspace_mei(source_root: &Path) -> Result<()> {
    let legacy = source_root.join(".mei");
    if !legacy.is_dir() {
        return Ok(());
    }
    let mappings = [
        ("local/hosts", WORKSPACE_HOSTS_DIR_REL),
        ("local/agent", WORKSPACE_AGENT_LOCAL_DIR_REL),
    ];
    for (from_suffix, to_rel) in mappings {
        let from = legacy.join(from_suffix);
        if !from.is_dir() {
            continue;
        }
        merge_dir_recursive(&from, &source_root.join(to_rel))?;
    }
    let legacy_runtime = legacy.join("runtime");
    if legacy_runtime.is_dir() {
        merge_dir_recursive(
            &legacy_runtime,
            &source_root.join(WORKSPACE_PLATFORM_DIR_REL),
        )?;
    }
    let remaining: Vec<_> = fs::read_dir(&legacy)
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .collect();
    if remaining.is_empty() {
        fs::remove_dir_all(&legacy).ok();
    }
    Ok(())
}

/// Move workspace root `runtime/` → `deploy/runtime/` when the new path is absent.
pub fn migrate_legacy_workspace_runtime_dir(source_root: &Path) -> Result<()> {
    let legacy = source_root.join(LEGACY_WORKSPACE_RUNTIME_DIR_REL);
    if !legacy.is_dir() {
        return Ok(());
    }
    let target = source_root
        .join(WORKSPACE_PLATFORM_DIR_REL)
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| source_root.join("deploy/runtime"));
    if target.is_dir() {
        merge_dir_recursive(&legacy, &target)?;
        fs::remove_dir_all(&legacy).ok();
    } else {
        fs::rename(&legacy, &target)
            .with_context(|| format!("move {} → {}", legacy.display(), target.display()))?;
    }
    Ok(())
}

pub fn toolchain_store_dir(source_root: &Path, toolchain_version: &str) -> PathBuf {
    resolve_toolchain_root(source_root)
        .join(TOOLCHAIN_STORE_REL)
        .join(toolchain_version.trim())
}

pub fn apply_toolchain_store_symlinks(source_root: &Path, toolchain_version: &str) -> Result<()> {
    let toolchain_root = resolve_toolchain_root(source_root);
    let store_dir = toolchain_store_dir(source_root, toolchain_version);
    let store_bin = store_dir.join("bin");
    fs::create_dir_all(&store_bin)?;
    migrate_flat_toolchain_bin_to_store(&toolchain_root, &store_bin)?;
    set_active_symlink(&toolchain_root.join(TOOLCHAIN_ACTIVE_REL), &store_dir)?;
    set_active_symlink(&toolchain_root.join("bin"), &store_bin)?;
    Ok(())
}

pub fn record_toolchain_install_links(source_root: &Path, toolchain_version: &str) -> Result<()> {
    let version = toolchain_version.trim();
    if version.is_empty() {
        return Ok(());
    }
    let mut links = read_links_state(source_root).unwrap_or_default();
    if links.toolchain.active.as_deref() == Some(version) {
        return Ok(());
    }
    links.toolchain.previous = links.toolchain.active.take();
    links.toolchain.active = Some(version.to_string());
    write_links_state(source_root, &links)
}

fn migrate_flat_toolchain_bin_to_store(toolchain_root: &Path, store_bin: &Path) -> Result<()> {
    let flat_bin = toolchain_root.join("bin");
    if flat_bin.is_symlink() {
        return Ok(());
    }
    if !flat_bin.is_dir() {
        return Ok(());
    }
    merge_dir_recursive(&flat_bin, store_bin)?;
    fs::remove_dir_all(&flat_bin)
        .with_context(|| format!("remove flat toolchain bin {}", flat_bin.display()))?;
    Ok(())
}

pub(crate) fn merge_dir_recursive(from: &Path, to: &Path) -> Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let dest = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            merge_dir_recursive(&entry.path(), &dest)?;
        } else if !dest.exists() {
            fs::copy(entry.path(), &dest)?;
        }
    }
    Ok(())
}
