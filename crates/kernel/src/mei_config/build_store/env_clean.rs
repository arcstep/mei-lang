use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use anyhow::Result;

use crate::mei_config::workspace_paths::{resolve_app_root, resolve_apps_root};
use crate::mei_config::types::{APP_BUILD_STORE_REL, APP_VAR_STORE_REL};

use super::build_generation::{
    format_version_footer_full, format_version_footer_short, is_build_generation_tag,
    resolve_version_display_identity_with_hint,
};
use super::env_paths::{
    app_env_root, env_generation_from_env_dir, normalize_env_generation_id,
    resolve_app_env_dir_following_current,
};
use super::types::{read_links_state, write_links_state};

#[derive(Debug, Clone, Default)]
pub struct MigrateEnvReport {
    pub migrated_build_dirs: usize,
    pub migrated_var_dirs: usize,
    pub env_versions: Vec<String>,
    pub removed_legacy_dirs: Vec<String>,
    pub upgraded_env_dirs: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct CleanEnvPolicy {
    pub dry_run: bool,
}

#[derive(Debug, Clone, Default)]
pub struct CleanEnvReport {
    pub removed: Vec<String>,
    pub retained: Vec<String>,
    pub dry_run: bool,
}

pub fn migrate_build_var_store_to_env(_source_root: &Path, app_root: &Path) -> Result<MigrateEnvReport> {
    let mut report = MigrateEnvReport::default();
    let build_store = app_root.join(APP_BUILD_STORE_REL);
    if build_store.is_dir() {
        let mut entries = fs::read_dir(&build_store)?.peekable();
        if entries.peek().is_some() {
            anyhow::bail!(
                "legacy build/store layout at {} — remove it and run build prepare",
                build_store.display()
            );
        }
    }

    let var_store = app_root.join(APP_VAR_STORE_REL);
    if var_store.is_dir() {
        let mut entries = fs::read_dir(&var_store)?.peekable();
        if entries.peek().is_some() {
            anyhow::bail!(
                "legacy var/store layout at {} — remove it and run build prepare",
                var_store.display()
            );
        }
    }

    report.removed_legacy_dirs = cleanup_legacy_app_store_dirs(app_root)?;
    report.upgraded_env_dirs = reject_non_build_generation_env_dirs(app_root)?;

    Ok(report)
}

fn reject_non_build_generation_env_dirs(app_root: &Path) -> Result<Vec<String>> {
    let env_root = app_env_root(app_root);
    if !env_root.is_dir() {
        return Ok(Vec::new());
    }
    let mut invalid = Vec::new();
    for entry in fs::read_dir(&env_root)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name == "current" || is_build_generation_tag(name.as_str()) {
            continue;
        }
        invalid.push(name);
    }
    if !invalid.is_empty() {
        anyhow::bail!(
            "legacy env directories at {}: {} — remove them and run build prepare",
            env_root.display(),
            invalid.join(", ")
        );
    }
    Ok(Vec::new())
}

fn cleanup_legacy_app_store_dirs(app_root: &Path) -> Result<Vec<String>> {
    let mut removed = Vec::new();
    for rel in [APP_BUILD_STORE_REL, APP_VAR_STORE_REL] {
        let path = app_root.join(rel);
        if !path.is_dir() {
            continue;
        }
        let mut entries = fs::read_dir(&path)?.peekable();
        if entries.peek().is_some() {
            continue;
        }
        fs::remove_dir_all(&path)?;
        removed.push(rel.to_string());
    }
    Ok(removed)
}

pub fn migrate_apps_to_env_layout(source_root: &Path, app_ids: &[String]) -> Result<Vec<(String, MigrateEnvReport)>> {
    let mut out = Vec::new();
    for app_id in app_ids {
        let app_root = resolve_app_root(source_root, app_id);
        let report = migrate_build_var_store_to_env(source_root, app_root.as_path())?;
        out.push((app_id.clone(), report));
    }
    let mut links = read_links_state(source_root).unwrap_or_default();
    normalize_links_build_fields(source_root, &mut links);
    write_links_state(source_root, &links)?;
    Ok(out)
}

fn normalize_links_build_fields(source_root: &Path, links: &mut super::types::LinksState) {
    if let Some(v) = links.build.candidate.take() {
        links.build.candidate = normalize_env_generation_id(source_root, v.as_str()).ok();
    }
    if let Some(v) = links.build.previous.take() {
        links.build.previous = normalize_env_generation_id(source_root, v.as_str()).ok();
    }
}

fn protected_env_versions(source_root: &Path) -> BTreeSet<String> {
    let mut keep = BTreeSet::new();
    let apps_root = resolve_apps_root(source_root);
    if apps_root.is_dir() {
        if let Ok(entries) = fs::read_dir(&apps_root) {
            for entry in entries.flatten() {
                if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    continue;
                }
                if let Some(env_dir) = resolve_app_env_dir_following_current(&entry.path()) {
                    if let Some(ver) = env_generation_from_env_dir(env_dir.as_path()) {
                        keep.insert(ver);
                    }
                }
            }
        }
    }
    if let Ok(links) = read_links_state(source_root) {
        for ver in [
            links.build.candidate.as_deref(),
            links.build.previous.as_deref(),
        ] {
            if let Some(v) = ver.map(str::trim).filter(|s| !s.is_empty()) {
                if let Ok(normalized) = normalize_env_generation_id(source_root, v) {
                    keep.insert(normalized);
                }
            }
        }
    }
    keep
}

pub fn clean_env_generations(
    source_root: &Path,
    app_ids: &[String],
    policy: &CleanEnvPolicy,
) -> Result<CleanEnvReport> {
    let keep = protected_env_versions(source_root);
    let mut report = CleanEnvReport {
        dry_run: policy.dry_run,
        ..CleanEnvReport::default()
    };
    for app_id in app_ids {
        let app_root = resolve_app_root(source_root, app_id);
        let env_root = app_env_root(app_root.as_path());
        if !env_root.is_dir() {
            continue;
        }
        for entry in fs::read_dir(&env_root)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let ver = entry.file_name().to_string_lossy().to_string();
            let label = format!("{app_id}/{ver}");
            if keep.contains(ver.as_str()) {
                report.retained.push(label);
                continue;
            }
            if policy.dry_run {
                report.removed.push(label);
            } else {
                fs::remove_dir_all(entry.path())?;
                report.removed.push(label);
            }
        }
    }
    Ok(report)
}

pub fn resolve_workspace_footer_label(source_root: &Path) -> String {
    format_version_footer_short(
        &resolve_version_display_identity_with_hint(source_root, None)
            .unwrap_or_else(|err| panic!("{err}")),
    )
}

pub fn resolve_build_footer_label(source_root: &Path) -> String {
    format_version_footer_full(
        &resolve_version_display_identity_with_hint(source_root, None)
            .unwrap_or_else(|err| panic!("{err}")),
    )
}

pub fn resolve_workspace_footer_label_with_hint(
    source_root: &Path,
    meilang_hint: Option<&str>,
) -> String {
    format_version_footer_short(
        &resolve_version_display_identity_with_hint(source_root, meilang_hint)
            .unwrap_or_else(|err| panic!("{err}")),
    )
}

pub fn resolve_build_footer_label_with_hint(
    source_root: &Path,
    meilang_hint: Option<&str>,
) -> String {
    format_version_footer_full(
        &resolve_version_display_identity_with_hint(source_root, meilang_hint)
            .unwrap_or_else(|err| panic!("{err}")),
    )
}
