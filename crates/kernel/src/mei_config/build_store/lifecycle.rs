use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;

use crate::mei_config::workspace_paths::{resolve_app_root, resolve_apps_root};

use super::env_paths::{
    app_build_active_link, app_env_build_dir, app_env_dir, app_env_var_dir, app_var_active_link,
    normalize_env_generation_id, resolve_env_generation_id, resolve_workspace_version,
};
use super::paths::{civil_from_days, resolve_toolchain_version, write_build_manifest};
use super::types::{
    read_links_state, write_links_state, BuildManifest, BUILD_MANIFEST_SCHEMA,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ContentStoreMergeStats {
    pub copied_files: usize,
    pub skipped_existing: usize,
}

fn build_content_store_dir(build_root: &Path) -> PathBuf {
    build_root.join("store").join("content")
}

/// Copy missing CAS blobs from `from_build_root` into `to_build_root` (content-addressed, skip existing).
pub fn merge_build_content_store(
    from_build_root: &Path,
    to_build_root: &Path,
) -> Result<ContentStoreMergeStats> {
    let from = build_content_store_dir(from_build_root);
    if !from.is_dir() {
        return Ok(ContentStoreMergeStats::default());
    }
    let to = build_content_store_dir(to_build_root);
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
    active_ver: &str,
    target_build_dir: &Path,
) -> Result<ContentStoreMergeStats> {
    merge_build_content_store(
        &app_env_build_dir(app_root, active_ver),
        target_build_dir,
    )
}

fn union_historical_build_content_into_target(
    app_root: &Path,
    target_ver: &str,
) -> Result<ContentStoreMergeStats> {
    let env_root = app_root.join("env");
    if !env_root.is_dir() {
        return Ok(ContentStoreMergeStats::default());
    }
    let target_dir = app_env_build_dir(app_root, target_ver);
    let mut total = ContentStoreMergeStats::default();
    for entry in fs::read_dir(&env_root)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name == target_ver {
            continue;
        }
        let other_build = entry.path().join("build");
        if !other_build.is_dir() {
            continue;
        }
        let stats = merge_build_content_store(other_build.as_path(), target_dir.as_path())?;
        total.copied_files += stats.copied_files;
        total.skipped_existing += stats.skipped_existing;
    }
    Ok(total)
}

/// Wipe and recreate `env/{ver}/build` and `env/{ver}/var` (replace semantics for same ver).
pub fn replace_env_generation(app_root: &Path, env_version: &str) -> Result<(PathBuf, PathBuf)> {
    let build_dir = app_env_build_dir(app_root, env_version);
    let var_dir = app_env_var_dir(app_root, env_version);
    fs::create_dir_all(app_env_dir(app_root, env_version))?;
    if build_dir.exists() {
        fs::remove_dir_all(&build_dir)?;
    }
    if var_dir.exists() {
        fs::remove_dir_all(&var_dir)?;
    }
    fs::create_dir_all(&build_dir)?;
    fs::create_dir_all(var_dir.join("cache"))?;
    fs::create_dir_all(var_dir.join("eval-cache"))?;
    fs::create_dir_all(var_dir.join("data-snapshots"))?;
    Ok((build_dir, var_dir))
}

pub struct PrebuildGeneration {
    pub env_version: String,
    pub toolchain_version: String,
    pub workspace_version: String,
    pub store_dirs: BTreeMap<String, PathBuf>,
}

impl PrebuildGeneration {
    /// Script-compat alias (`MEI_BUILD_ID`, `--build-id`).
    pub fn build_id(&self) -> &str {
        self.env_version.as_str()
    }
}

pub fn begin_prebuild_generation(source_root: &Path, app_ids: &[String]) -> Result<PrebuildGeneration> {
    let toolchain_version = resolve_toolchain_version(source_root);
    let workspace_version = resolve_workspace_version(source_root);
    let env_version = resolve_env_generation_id(source_root);
    let previous_active = read_links_state(source_root)
        .ok()
        .and_then(|links| links.build.active)
        .map(|v| normalize_env_generation_id(source_root, v.as_str()));
    let mut store_dirs = BTreeMap::new();
    for app_id in app_ids {
        let app_root = resolve_app_root(source_root, app_id);
        let (build_dir, _var_dir) = replace_env_generation(app_root.as_path(), env_version.as_str())?;
        if let Some(ref active_ver) = previous_active {
            if active_ver != &env_version {
                let _ = seed_build_content_store_from_active(
                    app_root.as_path(),
                    active_ver.as_str(),
                    build_dir.as_path(),
                );
            }
        }
        store_dirs.insert(app_id.clone(), build_dir);
    }
    Ok(PrebuildGeneration {
        env_version,
        toolchain_version,
        workspace_version,
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
        let app_root = resolve_app_root(source_root, app_id);
        let env_dir = app_env_dir(app_root.as_path(), generation.env_version.as_str());
        write_build_manifest(
            env_dir.as_path(),
            &BuildManifest {
                schema_version: BUILD_MANIFEST_SCHEMA.to_string(),
                env_version: generation.env_version.clone(),
                app_id: app_id.clone(),
                toolchain_version: generation.toolchain_version.clone(),
                workspace_version: Some(generation.workspace_version.clone()),
                source_revision: source_revision.map(str::to_string),
                stock_revision: stock_revision.map(str::to_string),
                finished_at: finished_at.clone(),
            },
        )?;
    }
    let mut links = read_links_state(source_root).unwrap_or_default();
    links.build.candidate = Some(generation.env_version.clone());
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
        .map(|id| normalize_env_generation_id(source_root, id))
        .or_else(|| {
            links
                .build
                .candidate
                .as_deref()
                .map(|id| normalize_env_generation_id(source_root, id))
        })
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
    links.build.previous = links
        .build
        .active
        .take()
        .map(|v| normalize_env_generation_id(source_root, v.as_str()));
    links.build.active = Some(target.clone());
    links.build.candidate = None;
    apply_build_symlinks_for_all_apps(source_root, &target)?;
    write_links_state(source_root, &links)?;
    Ok(target)
}

/// Point `build/active` and `var/active` at `env/{ver}/build|var` for listed apps.
pub fn attach_build_generation(
    source_root: &Path,
    app_ids: &[String],
    env_version: &str,
) -> Result<()> {
    let env_version = env_version.trim();
    if env_version.is_empty() {
        anyhow::bail!("attach_build_generation: empty env_version");
    }
    for app_id in app_ids {
        let app_root = resolve_app_root(source_root, app_id);
        let build_dir = app_env_build_dir(&app_root, env_version);
        fs::create_dir_all(build_dir.join("exchange"))?;
        let var_dir = app_env_var_dir(&app_root, env_version);
        fs::create_dir_all(var_dir.join("cache"))?;
        fs::create_dir_all(var_dir.join("eval-cache"))?;
        fs::create_dir_all(var_dir.join("data-snapshots"))?;
        set_active_symlink(&app_build_active_link(&app_root), &build_dir)?;
        set_active_symlink(&app_var_active_link(&app_root), &var_dir)?;
    }
    Ok(())
}

pub fn prepare_dev_build_generation(
    source_root: &Path,
    app_ids: &[String],
) -> Result<PrebuildGeneration> {
    let generation = begin_prebuild_generation(source_root, app_ids)?;
    attach_build_generation(source_root, app_ids, generation.env_version.as_str())?;
    Ok(generation)
}

pub fn finalize_and_promote_build(
    source_root: &Path,
    generation: &PrebuildGeneration,
    app_ids: &[String],
    source_revision: Option<&str>,
    stock_revision: Option<&str>,
    auto_promote: bool,
) -> Result<Option<String>> {
    finish_prebuild_generation(
        source_root,
        generation,
        app_ids,
        source_revision,
        stock_revision,
    )?;
    if auto_promote {
        Ok(Some(promote_build(
            source_root,
            Some(generation.env_version.as_str()),
        )?))
    } else {
        Ok(None)
    }
}

/// Move flat `build/active` (directory) into `env/{ver}/build` and symlink active.
pub fn migrate_flat_build_to_store(app_root: &Path, env_version: &str) -> Result<bool> {
    use super::migrate::merge_dir_recursive;
    let env_version = env_version.trim();
    if env_version.is_empty() {
        return Ok(false);
    }
    let active = app_build_active_link(app_root);
    if active.is_symlink() || !active.is_dir() {
        return Ok(false);
    }
    let build_dir = app_env_build_dir(app_root, env_version);
    fs::create_dir_all(app_env_dir(app_root, env_version))?;
    if build_dir.exists() {
        merge_dir_recursive(&active, &build_dir)?;
        fs::remove_dir_all(&active)?;
    } else {
        fs::create_dir_all(build_dir.parent().unwrap_or(app_root))?;
        fs::rename(&active, &build_dir)?;
    }
    set_active_symlink(&active, &build_dir)?;

    let var_active = app_var_active_link(app_root);
    if var_active.is_dir() && !var_active.is_symlink() {
        let var_dir = app_env_var_dir(app_root, env_version);
        fs::create_dir_all(app_env_dir(app_root, env_version))?;
        if var_dir.exists() {
            merge_dir_recursive(&var_active, &var_dir)?;
            fs::remove_dir_all(&var_active)?;
        } else {
            fs::rename(&var_active, &var_dir)?;
        }
        set_active_symlink(&var_active, &var_dir)?;
    }
    Ok(true)
}

pub fn rollback_build(source_root: &Path) -> Result<String> {
    let mut links = read_links_state(source_root)?;
    let target = links
        .build
        .previous
        .clone()
        .ok_or_else(|| anyhow::anyhow!("no previous build to rollback"))?;
    let target = normalize_env_generation_id(source_root, target.as_str());
    links.build.active = Some(target.clone());
    apply_build_symlinks_for_all_apps(source_root, target.as_str())?;
    write_links_state(source_root, &links)?;
    Ok(target)
}

pub(crate) fn apply_build_symlinks_for_all_apps(source_root: &Path, env_version: &str) -> Result<()> {
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
        set_active_symlink(
            &app_build_active_link(&app_root),
            &app_env_build_dir(&app_root, env_version),
        )?;
        set_active_symlink(
            &app_var_active_link(&app_root),
            &app_env_var_dir(&app_root, env_version),
        )?;
    }
    Ok(())
}

pub(crate) fn set_active_symlink(link: &Path, target: &Path) -> Result<()> {
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
