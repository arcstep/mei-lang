use std::{
    fs,
    path::{Path as FsPath, PathBuf},
};

use mei_lang_toolchain::meilang_author_skill_package;
use walkdir::WalkDir;

use super::super::ManagedOpencodeSkillStatus;
use crate::AppState;

fn managed_skill_install_dir(source_root: &FsPath) -> PathBuf {
    let descriptor = meilang_author_skill_package();
    source_root.join(descriptor.install_dir_rel)
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

fn directory_latest_modified_ms(path: &FsPath) -> Option<u128> {
    if !path.exists() {
        return None;
    }
    let mut latest = fs::metadata(path)
        .ok()
        .and_then(|meta| meta.modified().ok())
        .and_then(|value| {
            value
                .duration_since(std::time::UNIX_EPOCH)
                .ok()
                .map(|dur| dur.as_millis())
        });
    for entry in WalkDir::new(path).into_iter().flatten() {
        let modified = entry
            .metadata()
            .ok()
            .and_then(|meta| meta.modified().ok())
            .and_then(|value| {
                value
                    .duration_since(std::time::UNIX_EPOCH)
                    .ok()
                    .map(|dur| dur.as_millis())
            });
        if modified > latest {
            latest = modified;
        }
    }
    latest
}

fn build_skill_status(source_root: &FsPath) -> ManagedOpencodeSkillStatus {
    let descriptor = meilang_author_skill_package();
    let install_dir = managed_skill_install_dir(source_root);
    let entry_file = install_dir.join(&descriptor.entry_file);
    let installed = entry_file.exists();
    let install_updated_at_ms = directory_latest_modified_ms(&install_dir);
    ManagedOpencodeSkillStatus {
        source_dir: install_dir.display().to_string(),
        install_dir: install_dir.display().to_string(),
        entry_file: entry_file.display().to_string(),
        source_present: installed,
        installed,
        stale: false,
        source_updated_at_ms: install_updated_at_ms,
        install_updated_at_ms,
        file_count: markdown_file_count(&install_dir),
        revision: None,
    }
}

pub(crate) fn managed_agent_skill_status_for_root(
    _package_root: &FsPath,
    source_root: &FsPath,
) -> ManagedOpencodeSkillStatus {
    build_skill_status(source_root)
}

pub(crate) fn managed_agent_skill_status(
    state: &AppState,
) -> anyhow::Result<ManagedOpencodeSkillStatus> {
    Ok(build_skill_status(&state.source_root))
}

pub(crate) fn sync_managed_agent_skill_for_root(
    _package_root: &FsPath,
    source_root: &FsPath,
) -> anyhow::Result<ManagedOpencodeSkillStatus> {
    let status = build_skill_status(source_root);
    if !status.installed {
        anyhow::bail!(
            "workspace-local author skill is not installed at {}; run `mei-toolchain workspace runtime install --source-root {}`",
            status.install_dir,
            source_root.display()
        );
    }
    Ok(status)
}

pub(crate) fn sync_managed_agent_skill(
    state: &AppState,
) -> anyhow::Result<ManagedOpencodeSkillStatus> {
    sync_managed_agent_skill_for_root(&state.package_root, &state.source_root)
}
