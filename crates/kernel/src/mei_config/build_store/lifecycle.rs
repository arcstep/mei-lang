use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;

use crate::mei_config::workspace_paths::{resolve_app_root, resolve_apps_root};

use super::paths::{
    app_build_active_link, app_build_store_dir, app_var_active_link, app_var_store_dir,
    civil_from_days, generate_build_id, resolve_toolchain_version, write_build_manifest,
};
use super::types::{
    read_links_state, write_links_state, BuildManifest, BUILD_MANIFEST_SCHEMA,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ContentStoreMergeStats {
    pub copied_files: usize,
    pub skipped_existing: usize,
}

fn build_content_store_dir(build_store_dir: &Path) -> PathBuf {
    build_store_dir.join("store").join("content")
}

/// Copy missing CAS blobs from `from_build_store` into `to_build_store` (content-addressed, skip existing).
pub fn merge_build_content_store(
    from_build_store: &Path,
    to_build_store: &Path,
) -> Result<ContentStoreMergeStats> {
    let from = build_content_store_dir(from_build_store);
    if !from.is_dir() {
        return Ok(ContentStoreMergeStats::default());
    }
    let to = build_content_store_dir(to_build_store);
    let mut stats = ContentStoreMergeStats::default();
    for kind_entry in fs::read_dir(&from)? {
        let kind_entry = kind_entry?;
        if !kind_entry.file_type()?.is_dir() {
            continue;
        }
        let kind_name = kind_entry.file_name();
        let to_kind = to.join(&kind_name);
        fs::create_dir_all(&to_kind)?;
        for blob_entry in fs::read_dir(kind_entry.path())? {
            let blob_entry = blob_entry?;
            if !blob_entry.file_type()?.is_file() {
                continue;
            }
            let dest = to_kind.join(blob_entry.file_name());
            if dest.exists() {
                stats.skipped_existing += 1;
                continue;
            }
            fs::copy(blob_entry.path(), &dest)?;
            stats.copied_files += 1;
        }
    }
    Ok(stats)
}

fn seed_build_content_store_from_active(
    app_root: &Path,
    active_build_id: &str,
    target_store_dir: &Path,
) -> Result<ContentStoreMergeStats> {
    merge_build_content_store(
        &app_build_store_dir(app_root, active_build_id),
        target_store_dir,
    )
}

fn union_historical_build_content_into_target(
    app_root: &Path,
    target_build_id: &str,
) -> Result<ContentStoreMergeStats> {
    let store_parent = app_root.join("build").join("store");
    if !store_parent.is_dir() {
        return Ok(ContentStoreMergeStats::default());
    }
    let target_dir = app_build_store_dir(app_root, target_build_id);
    let mut total = ContentStoreMergeStats::default();
    for entry in fs::read_dir(&store_parent)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name == target_build_id {
            continue;
        }
        let stats = merge_build_content_store(&entry.path(), &target_dir)?;
        total.copied_files += stats.copied_files;
        total.skipped_existing += stats.skipped_existing;
    }
    Ok(total)
}

pub struct PrebuildGeneration {
    pub build_id: String,
    pub toolchain_version: String,
    pub store_dirs: BTreeMap<String, PathBuf>,
}

pub fn begin_prebuild_generation(source_root: &Path, app_ids: &[String]) -> Result<PrebuildGeneration> {
    let toolchain_version = resolve_toolchain_version(source_root);
    let build_id = generate_build_id(&toolchain_version);
    let previous_active = read_links_state(source_root)
        .ok()
        .and_then(|links| links.build.active);
    let mut store_dirs = BTreeMap::new();
    for app_id in app_ids {
        let app_root = resolve_app_root(source_root, app_id);
        let store_dir = app_build_store_dir(&app_root, &build_id);
        fs::create_dir_all(&store_dir)?;
        if let Some(ref active_id) = previous_active {
            if active_id != &build_id {
                let _ = seed_build_content_store_from_active(
                    app_root.as_path(),
                    active_id.as_str(),
                    store_dir.as_path(),
                );
            }
        }
        let var_store = app_var_store_dir(&app_root, &build_id);
        fs::create_dir_all(var_store.join("cache"))?;
        fs::create_dir_all(var_store.join("eval-results"))?;
        store_dirs.insert(app_id.clone(), store_dir);
    }
    Ok(PrebuildGeneration {
        build_id,
        toolchain_version,
        store_dirs,
    })
}

pub fn finish_prebuild_generation(
    source_root: &Path,
    generation: &PrebuildGeneration,
    app_ids: &[String],
    source_revision: Option<&str>,
    stock_revision: Option<&str>,
) -> Result<()> {
    let finished_at = chrono_like_rfc3339();
    for app_id in app_ids {
        let store_dir = generation
            .store_dirs
            .get(app_id)
            .cloned()
            .unwrap_or_else(|| app_build_store_dir(&resolve_app_root(source_root, app_id), &generation.build_id));
        write_build_manifest(
            &store_dir,
            &BuildManifest {
                schema_version: BUILD_MANIFEST_SCHEMA.to_string(),
                build_id: generation.build_id.clone(),
                app_id: app_id.clone(),
                toolchain_version: generation.toolchain_version.clone(),
                source_revision: source_revision.map(str::to_string),
                stock_revision: stock_revision.map(str::to_string),
                finished_at: finished_at.clone(),
            },
        )?;
    }
    let mut links = read_links_state(source_root).unwrap_or_default();
    links.build.candidate = Some(generation.build_id.clone());
    if links
        .toolchain
        .active
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_none()
    {
        links.toolchain.active = Some(generation.toolchain_version.clone());
    }
    if source_revision.is_some() {
        links.source_revision = source_revision.map(str::to_string);
    }
    if stock_revision.is_some() {
        links.stock_revision = stock_revision.map(str::to_string);
    }
    write_links_state(source_root, &links)?;
    Ok(())
}

fn chrono_like_rfc3339() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (y, m, d) = civil_from_days((now / 86400) as i64);
    let tod = now % 86400;
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        tod / 3600,
        (tod % 3600) / 60,
        tod % 60
    )
}

pub fn promote_build(source_root: &Path, build_id: Option<&str>) -> Result<String> {
    let mut links = read_links_state(source_root)?;
    let target = build_id
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or_else(|| links.build.candidate.clone())
        .ok_or_else(|| anyhow::anyhow!("no build candidate to promote"))?;
    let apps_root = resolve_apps_root(source_root);
    if apps_root.is_dir() {
        for entry in fs::read_dir(&apps_root)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            union_historical_build_content_into_target(&entry.path(), target.as_str())?;
        }
    }
    if links.build.active.as_deref() == Some(target.as_str()) {
        return Ok(target);
    }
    links.build.previous = links.build.active.take();
    links.build.active = Some(target.clone());
    links.build.candidate = None;
    apply_build_symlinks_for_all_apps(source_root, &target)?;
    write_links_state(source_root, &links)?;
    Ok(target)
}

pub fn rollback_build(source_root: &Path) -> Result<String> {
    let mut links = read_links_state(source_root)?;
    let target = links
        .build
        .previous
        .clone()
        .ok_or_else(|| anyhow::anyhow!("no previous build to rollback"))?;
    links.build.active = Some(target.clone());
    apply_build_symlinks_for_all_apps(source_root, &target)?;
    write_links_state(source_root, &links)?;
    Ok(target)
}

fn apply_build_symlinks_for_all_apps(source_root: &Path, build_id: &str) -> Result<()> {
    let apps_root = resolve_apps_root(source_root);
    if !apps_root.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(&apps_root)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let app_root = entry.path();
        set_active_symlink(&app_build_active_link(&app_root), &app_build_store_dir(&app_root, build_id))?;
        set_active_symlink(&app_var_active_link(&app_root), &app_var_store_dir(&app_root, build_id))?;
    }
    Ok(())
}

pub(super) fn set_active_symlink(link: &Path, target: &Path) -> Result<()> {
    fs::create_dir_all(target)?;
    if link.is_symlink() || link.exists() {
        fs::remove_file(link).or_else(|_| fs::remove_dir_all(link))?;
    }
    if let Some(parent) = link.parent() {
        fs::create_dir_all(parent)?;
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link)?;
    }
    #[cfg(not(unix))]
    {
        fs::create_dir_all(link)?;
        let marker = link.join(".mei-build-target");
        fs::write(marker, target.to_string_lossy().as_bytes())?;
    }
    Ok(())
}
