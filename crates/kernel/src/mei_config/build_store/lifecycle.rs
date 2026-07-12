use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use sha2::{Digest, Sha256};

use crate::mei_config::workspace_paths::{resolve_app_root, resolve_apps_root};

use super::build_generation::require_build_generation_tag;
use super::build_generation::resolve_build_generation_for_prebuild;
use super::env_paths::{
    app_env_build_dir, app_env_current_link, app_env_dir, app_env_var_dir,
    env_generation_from_env_dir, normalize_env_generation_id,
    resolve_app_build_generation_from_current, resolve_app_env_dir_following_current,
    resolve_env_generation_id_for_prebuild, resolve_workspace_default_app_id,
};
use super::paths::{civil_from_days, resolve_toolchain_version_with_hint, write_build_manifest};
use super::types::{read_links_state, write_links_state, BuildManifest, BUILD_MANIFEST_SCHEMA};

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
    merge_build_content_store(&app_env_build_dir(app_root, active_ver), target_build_dir)
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
    pub build_generation: String,
    pub toolchain_version: String,
    pub workspace_version: String,
    pub config_digest: Option<String>,
    pub store_dirs: BTreeMap<String, PathBuf>,
}

impl PrebuildGeneration {
    /// Script-compat alias (`MEI_BUILD_ID`, `--build-id`).
    pub fn build_id(&self) -> &str {
        self.env_version.as_str()
    }
}

pub fn begin_prebuild_generation(
    source_root: &Path,
    app_ids: &[String],
) -> Result<PrebuildGeneration> {
    begin_prebuild_generation_with_hint(source_root, app_ids, None)
}

pub fn begin_prebuild_generation_with_hint(
    source_root: &Path,
    app_ids: &[String],
    cli_toolchain_hint: Option<&str>,
) -> Result<PrebuildGeneration> {
    let toolchain_version = resolve_toolchain_version_with_hint(source_root, cli_toolchain_hint);
    let build_spec = resolve_build_generation_for_prebuild(source_root);
    let env_version = resolve_env_generation_id_for_prebuild(source_root);
    let workspace_version = build_spec.date.clone();
    let config_digest = workspace_config_digest(source_root);
    let mut store_dirs = BTreeMap::new();
    for app_id in app_ids {
        let app_root = resolve_app_root(source_root, app_id);
        let previous_active = resolve_app_env_dir_following_current(app_root.as_path())
            .and_then(|env_dir| env_generation_from_env_dir(env_dir.as_path()));
        let (build_dir, _var_dir) =
            replace_env_generation(app_root.as_path(), env_version.as_str())?;
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
        build_generation: build_spec.tag,
        toolchain_version,
        workspace_version,
        config_digest,
        store_dirs,
    })
}

fn workspace_config_digest(source_root: &Path) -> Option<String> {
    let path = std::env::var_os("MEI_WORKSPACE_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|| source_root.join("workspace.json"));
    let bytes = fs::read(path).ok()?;
    Some(format!("{:x}", Sha256::digest(bytes)))
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
                build_generation: Some(generation.build_generation.clone()),
                workspace_version: Some(generation.workspace_version.clone()),
                config_digest: generation.config_digest.clone(),
                source_revision: source_revision.map(str::to_string),
                stock_revision: stock_revision.map(str::to_string),
                finished_at: finished_at.clone(),
            },
        )?;
    }
    let mut links = read_links_state(source_root).unwrap_or_default();
    links.build.candidate = Some(generation.env_version.clone());
    links.toolchain.active = Some(generation.toolchain_version.clone());
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

fn snapshot_default_app_current_generation(source_root: &Path) -> Option<String> {
    let app_id = resolve_workspace_default_app_id(source_root)?;
    let app_root = resolve_app_root(source_root, app_id.as_str());
    resolve_app_build_generation_from_current(app_root.as_path()).ok()
}

fn all_apps_at_generation(source_root: &Path, env_version: &str) -> bool {
    let apps_root = resolve_apps_root(source_root);
    if !apps_root.is_dir() {
        return false;
    }
    let Ok(entries) = fs::read_dir(&apps_root) else {
        return false;
    };
    let mut found = false;
    for entry in entries.flatten() {
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        found = true;
        let current = resolve_app_env_dir_following_current(&entry.path())
            .and_then(|env_dir| env_generation_from_env_dir(env_dir.as_path()));
        if current.as_deref() != Some(env_version) {
            return false;
        }
    }
    found
}

pub fn promote_build(source_root: &Path, build_id: Option<&str>) -> Result<String> {
    let mut links = read_links_state(source_root)?;
    let target = if let Some(id) = build_id.map(str::trim).filter(|s| !s.is_empty()) {
        normalize_env_generation_id(source_root, id)?
    } else if let Some(candidate) = links
        .build
        .candidate
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        normalize_env_generation_id(source_root, candidate)?
    } else {
        anyhow::bail!("no build candidate to promote");
    };
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
    if all_apps_at_generation(source_root, target.as_str()) {
        return Ok(target);
    }
    if let Some(current) = snapshot_default_app_current_generation(source_root) {
        if current != target {
            links.build.previous = Some(current);
        }
    }
    links.build.candidate = None;
    apply_build_symlinks_for_all_apps(source_root, &target)?;
    write_links_state(source_root, &links)?;
    Ok(target)
}

/// Point `env/current` at `env/{ver}` for listed apps.
pub fn attach_build_generation(
    source_root: &Path,
    app_ids: &[String],
    env_version: &str,
) -> Result<()> {
    let env_version = require_build_generation_tag(env_version)?.tag;
    for app_id in app_ids {
        let app_root = resolve_app_root(source_root, app_id);
        let build_dir = app_env_build_dir(&app_root, env_version.as_str());
        fs::create_dir_all(build_dir.join("exchange"))?;
        let var_dir = app_env_var_dir(&app_root, env_version.as_str());
        fs::create_dir_all(var_dir.join("cache"))?;
        fs::create_dir_all(var_dir.join("eval-cache"))?;
        fs::create_dir_all(var_dir.join("data-snapshots"))?;
        set_active_symlink(
            &app_env_current_link(&app_root),
            &app_env_dir(&app_root, env_version.as_str()),
        )?;
    }
    Ok(())
}

pub fn prepare_dev_build_generation(
    source_root: &Path,
    app_ids: &[String],
) -> Result<PrebuildGeneration> {
    prepare_dev_build_generation_with_hint(source_root, app_ids, None)
}

pub fn prepare_dev_build_generation_with_hint(
    source_root: &Path,
    app_ids: &[String],
    cli_toolchain_hint: Option<&str>,
) -> Result<PrebuildGeneration> {
    let generation = begin_prebuild_generation_with_hint(source_root, app_ids, cli_toolchain_hint)?;
    attach_build_generation(source_root, app_ids, generation.env_version.as_str())?;
    sync_dev_links_for_generation(source_root, &generation)?;
    Ok(generation)
}

fn sync_dev_links_for_generation(
    source_root: &Path,
    generation: &PrebuildGeneration,
) -> Result<()> {
    let mut links = read_links_state(source_root).unwrap_or_default();
    links.toolchain.active = Some(generation.toolchain_version.clone());
    links.build.candidate = Some(generation.env_version.clone());
    write_links_state(source_root, &links)?;
    Ok(())
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

/// Legacy flat build/active migration is no longer supported.
pub fn migrate_flat_build_to_store(app_root: &Path, _env_version: &str) -> Result<bool> {
    let flat_build = app_root.join("build").join("active");
    if flat_build.is_dir() && !flat_build.is_symlink() {
        anyhow::bail!(
            "legacy flat build/active at {} — remove it and run build prepare",
            flat_build.display()
        );
    }
    Ok(false)
}

pub fn rollback_build(source_root: &Path) -> Result<String> {
    let links = read_links_state(source_root)?;
    let target = links
        .build
        .previous
        .clone()
        .ok_or_else(|| anyhow::anyhow!("no previous build to rollback"))?;
    let target = normalize_env_generation_id(source_root, target.as_str())?;
    apply_build_symlinks_for_all_apps(source_root, target.as_str())?;
    write_links_state(source_root, &links)?;
    Ok(target)
}

pub(crate) fn apply_build_symlinks_for_all_apps(
    source_root: &Path,
    env_version: &str,
) -> Result<()> {
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
        let env_dir = app_env_dir(&app_root, env_version);
        set_active_symlink(&app_env_current_link(&app_root), &env_dir)?;
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
