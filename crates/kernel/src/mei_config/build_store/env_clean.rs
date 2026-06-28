use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use anyhow::Result;

use crate::mei_config::workspace_paths::resolve_app_root;
use crate::mei_config::types::{APP_BUILD_STORE_REL, APP_VAR_STORE_REL};

use super::env_paths::{
    app_env_root, format_env_generation_id, format_build_identity_display,
    is_composite_env_generation_id, normalize_env_generation_id, parse_composite_env_generation_id,
    parse_ver_from_legacy_build_id, resolve_active_build_identity, resolve_env_generation_id,
    resolve_workspace_version,
};
use super::lifecycle::apply_build_symlinks_for_all_apps;
use super::migrate::merge_dir_recursive;
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

pub fn migrate_build_var_store_to_env(source_root: &Path, app_root: &Path) -> Result<MigrateEnvReport> {
    let mut report = MigrateEnvReport::default();
    let ws_ver = resolve_workspace_version(source_root);
    let build_store = app_root.join(APP_BUILD_STORE_REL);
    if build_store.is_dir() {
        for entry in fs::read_dir(&build_store)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let legacy_id = entry.file_name().to_string_lossy().to_string();
            let Some(tc) = parse_ver_from_legacy_build_id(legacy_id.as_str()) else {
                continue;
            };
            let composite = format_env_generation_id(tc.as_str(), ws_ver.as_str());
            let target_build = app_env_root(app_root).join(&composite).join("build");
            fs::create_dir_all(app_env_root(app_root))?;
            if target_build.exists() {
                merge_dir_recursive(&entry.path(), &target_build)?;
                fs::remove_dir_all(entry.path())?;
            } else {
                fs::create_dir_all(target_build.parent().unwrap_or(app_root))?;
                fs::rename(entry.path(), &target_build)?;
            }
            report.migrated_build_dirs += 1;
            push_env_version(&mut report.env_versions, composite);
        }
        if fs::read_dir(&build_store)?.next().is_none() {
            fs::remove_dir_all(&build_store).ok();
        }
    }

    let var_store = app_root.join(APP_VAR_STORE_REL);
    if var_store.is_dir() {
        for entry in fs::read_dir(&var_store)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let legacy_id = entry.file_name().to_string_lossy().to_string();
            let Some(tc) = parse_ver_from_legacy_build_id(legacy_id.as_str()) else {
                continue;
            };
            let composite = format_env_generation_id(tc.as_str(), ws_ver.as_str());
            let target_var = app_env_root(app_root).join(&composite).join("var");
            fs::create_dir_all(app_env_root(app_root))?;
            if target_var.exists() {
                merge_dir_recursive(&entry.path(), &target_var)?;
                fs::remove_dir_all(entry.path())?;
            } else {
                fs::create_dir_all(target_var.parent().unwrap_or(app_root))?;
                fs::rename(entry.path(), &target_var)?;
            }
            report.migrated_var_dirs += 1;
            push_env_version(&mut report.env_versions, composite);
        }
        if fs::read_dir(&var_store)?.next().is_none() {
            fs::remove_dir_all(&var_store).ok();
        }
    }

    report.removed_legacy_dirs = cleanup_legacy_app_store_dirs(app_root)?;
    report.upgraded_env_dirs = upgrade_non_composite_env_dirs(source_root, app_root)?;

    Ok(report)
}

fn push_env_version(out: &mut Vec<String>, ver: String) {
    if !out.contains(&ver) {
        out.push(ver);
    }
}

fn upgrade_non_composite_env_dirs(source_root: &Path, app_root: &Path) -> Result<Vec<String>> {
    let env_root = app_env_root(app_root);
    if !env_root.is_dir() {
        return Ok(Vec::new());
    }
    let ws_ver = resolve_workspace_version(source_root);
    let mut upgraded = Vec::new();
    for entry in fs::read_dir(&env_root)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name.contains("-wsws") {
            let fixed = name.replace("-wsws", "-ws");
            let target = env_root.join(&fixed);
            if target.exists() {
                merge_dir_recursive(&entry.path(), target.as_path())?;
                fs::remove_dir_all(entry.path())?;
            } else {
                fs::rename(entry.path(), &target)?;
            }
            upgraded.push(format!("{name} -> {fixed}"));
            continue;
        }
        if is_composite_env_generation_id(name.as_str()) {
            continue;
        }
        let composite = format_env_generation_id(name.as_str(), ws_ver.as_str());
        let target = env_root.join(&composite);
        if target.as_path() == entry.path() {
            continue;
        }
        if target.exists() {
            merge_dir_recursive(&entry.path(), target.as_path())?;
            fs::remove_dir_all(entry.path())?;
        } else {
            fs::rename(entry.path(), &target)?;
        }
        upgraded.push(format!("{name} -> {composite}"));
    }
    Ok(upgraded)
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
    let active = read_links_state(source_root)
        .ok()
        .and_then(|links| links.build.active)
        .map(|v| normalize_env_generation_id(source_root, v.as_str()))
        .unwrap_or_else(|| resolve_env_generation_id(source_root));
    apply_build_symlinks_for_all_apps(source_root, active.as_str())?;
    let mut links = read_links_state(source_root).unwrap_or_default();
    normalize_links_build_fields(source_root, &mut links);
    write_links_state(source_root, &links)?;
    Ok(out)
}

fn normalize_links_build_fields(source_root: &Path, links: &mut super::types::LinksState) {
    if let Some(v) = links.build.active.take() {
        links.build.active = Some(normalize_env_generation_id(source_root, v.as_str()));
    }
    if let Some(v) = links.build.candidate.take() {
        links.build.candidate = Some(normalize_env_generation_id(source_root, v.as_str()));
    }
    if let Some(v) = links.build.previous.take() {
        links.build.previous = Some(normalize_env_generation_id(source_root, v.as_str()));
    }
}

fn protected_env_versions(source_root: &Path) -> BTreeSet<String> {
    let mut keep = BTreeSet::new();
    if let Ok(links) = read_links_state(source_root) {
        for ver in [
            links.build.active.as_deref(),
            links.build.candidate.as_deref(),
            links.build.previous.as_deref(),
        ] {
            if let Some(v) = ver.map(str::trim).filter(|s| !s.is_empty()) {
                keep.insert(normalize_env_generation_id(source_root, v));
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

pub fn resolve_build_footer_label(source_root: &Path) -> String {
    if let Ok(links) = read_links_state(source_root) {
        if let Some(active) = links
            .build
            .active
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            if let Some((tc, ws)) = parse_composite_env_generation_id(active) {
                return format!("MeiLang {tc} · WS {ws} · build {active}");
            }
        }
    }
    format_build_identity_display(&resolve_active_build_identity(source_root))
}
