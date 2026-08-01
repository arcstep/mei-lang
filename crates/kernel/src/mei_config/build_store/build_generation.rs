//! Build generation tags and version display helpers.
//!
//! Formats:
//! - Legacy: `WS-yyyymmdd.fixver` (no workspace git)
//! - Git: `WS-yyyymmdd-<short7>` or `WS-yyyymmdd-<short7>.N` (N≥1)

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;

use crate::mei_config::io::load_workspace_config;
use crate::mei_config::types::WorkspaceBuildGenerationConfig;
use crate::mei_config::workspace_paths::{resolve_app_root, resolve_apps_root};

use super::env_paths::{
    resolve_app_build_generation_from_current, resolve_workspace_default_app_id,
};
use super::paths::resolve_toolchain_version_with_hint;

const BUILD_GENERATION_PREFIX: &str = "WS-";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildGenerationSpec {
    /// Canonical tag, e.g. `WS-20260630.0` or `WS-20260801-abc1234`.
    pub tag: String,
    pub date: String,
    pub fixver: u32,
    /// Workspace git short hash when using git form.
    pub git_short: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionDisplayIdentity {
    /// MeiLang / toolchain semver shown to users (`x.y.z`).
    pub meilang_version: String,
    /// Canonical build generation tag.
    pub build_generation: String,
    /// Human label: `Build <tag>`.
    pub build_display_tag: String,
    /// Internal env directory id (active build pointer).
    pub env_generation_id: String,
}

pub fn format_build_generation_tag(date: &str, fixver: u32) -> String {
    format!("{BUILD_GENERATION_PREFIX}{date}.{fixver}")
}

pub fn format_build_generation_tag_git(date: &str, git_short: &str, fixver: u32) -> String {
    if fixver == 0 {
        format!("{BUILD_GENERATION_PREFIX}{date}-{git_short}")
    } else {
        format!("{BUILD_GENERATION_PREFIX}{date}-{git_short}.{fixver}")
    }
}

pub fn format_build_display_tag(tag: &str) -> String {
    format!("Build {tag}")
}

pub fn format_meilang_version_label(version: &str) -> String {
    format!("MeiLang {}", version.trim())
}

pub fn format_version_footer_short(identity: &VersionDisplayIdentity) -> String {
    format!(
        "{} · {}",
        format_meilang_version_label(identity.meilang_version.as_str()),
        identity.build_display_tag
    )
}

pub fn format_version_footer_full(identity: &VersionDisplayIdentity) -> String {
    format_version_footer_short(identity)
}

fn is_yyyymmdd(date: &str) -> bool {
    date.len() == 8 && date.chars().all(|ch| ch.is_ascii_digit())
}

fn is_git_short(hash: &str) -> bool {
    let len = hash.len();
    (4..=40).contains(&len) && hash.chars().all(|ch| ch.is_ascii_hexdigit())
}

pub fn parse_build_generation_tag(raw: &str) -> Option<BuildGenerationSpec> {
    let trimmed = raw.trim();
    let rest = trimmed.strip_prefix(BUILD_GENERATION_PREFIX)?;

    // Git form: YYYYMMDD-<hash> or YYYYMMDD-<hash>.N
    if let Some((date, rem)) = rest.split_once('-') {
        if is_yyyymmdd(date) {
            let (hash, fixver) = match rem.rsplit_once('.') {
                Some((h, n)) if n.chars().all(|c| c.is_ascii_digit()) && is_git_short(h) => {
                    (h, n.parse::<u32>().ok()?)
                }
                _ if is_git_short(rem) => (rem, 0u32),
                _ => return None,
            };
            return Some(BuildGenerationSpec {
                tag: format_build_generation_tag_git(date, hash, fixver),
                date: date.to_string(),
                fixver,
                git_short: Some(hash.to_string()),
            });
        }
    }

    // Legacy: YYYYMMDD.N
    let (date, fixver_raw) = rest.rsplit_once('.')?;
    if !is_yyyymmdd(date) {
        return None;
    }
    let fixver: u32 = fixver_raw.parse().ok()?;
    Some(BuildGenerationSpec {
        tag: format_build_generation_tag(date, fixver),
        date: date.to_string(),
        fixver,
        git_short: None,
    })
}

pub fn is_build_generation_tag(raw: &str) -> bool {
    parse_build_generation_tag(raw).is_some()
}

pub fn require_build_generation_tag(raw: &str) -> Result<BuildGenerationSpec> {
    parse_build_generation_tag(raw).ok_or_else(|| {
        anyhow::anyhow!(
            "invalid build generation `{raw}` (expected WS-yyyymmdd.fixver or WS-yyyymmdd-<git>[.N])"
        )
    })
}

pub fn resolve_build_generation_config(source_root: &Path) -> BuildGenerationSpec {
    let cfg = load_workspace_config(source_root);
    resolve_build_generation_from_config(source_root, &cfg.build.generation, false)
}

/// Prebuild: honour `dateSource=auto` by using today's date; prefer workspace git short hash.
pub fn resolve_build_generation_for_prebuild(source_root: &Path) -> BuildGenerationSpec {
    let cfg = load_workspace_config(source_root);
    resolve_build_generation_from_config(source_root, &cfg.build.generation, true)
}

fn resolve_build_generation_from_config(
    source_root: &Path,
    gen: &WorkspaceBuildGenerationConfig,
    allocate: bool,
) -> BuildGenerationSpec {
    let fixver = gen.fixver.unwrap_or(0);
    let date = match gen.date_source.as_deref().map(str::trim) {
        Some("manual") => gen
            .date
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(today_yyyymmdd),
        _ => today_yyyymmdd(),
    };

    if let Some(short) = workspace_git_short(source_root) {
        if allocate {
            return allocate_git_generation(source_root, date.as_str(), short.as_str());
        }
        return BuildGenerationSpec {
            tag: format_build_generation_tag_git(date.as_str(), short.as_str(), fixver),
            date,
            fixver,
            git_short: Some(short),
        };
    }

    BuildGenerationSpec {
        tag: format_build_generation_tag(date.as_str(), fixver),
        date,
        fixver,
        git_short: None,
    }
}

fn workspace_git_short(source_root: &Path) -> Option<String> {
    let output = Command::new("git")
        .args(["-C", source_root.to_str()?, "rev-parse", "--short=7", "HEAD"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let hash = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if is_git_short(&hash) {
        Some(hash)
    } else {
        None
    }
}

fn existing_generation_tags(source_root: &Path) -> BTreeSet<String> {
    let mut tags = BTreeSet::new();
    let apps_root = resolve_apps_root(source_root);
    let Ok(entries) = std::fs::read_dir(apps_root) else {
        return tags;
    };
    for entry in entries.flatten() {
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let env_root = entry.path().join("env");
        let Ok(env_entries) = std::fs::read_dir(env_root) else {
            continue;
        };
        for env_entry in env_entries.flatten() {
            if !env_entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                continue;
            }
            let name = env_entry.file_name().to_string_lossy().to_string();
            if is_build_generation_tag(name.as_str()) {
                tags.insert(name);
            }
        }
    }
    tags
}

fn allocate_git_generation(source_root: &Path, date: &str, short: &str) -> BuildGenerationSpec {
    let existing = existing_generation_tags(source_root);
    let base = format_build_generation_tag_git(date, short, 0);
    if !existing.contains(&base) {
        return BuildGenerationSpec {
            tag: base,
            date: date.to_string(),
            fixver: 0,
            git_short: Some(short.to_string()),
        };
    }
    let mut n = 1u32;
    loop {
        let tag = format_build_generation_tag_git(date, short, n);
        if !existing.contains(&tag) {
            return BuildGenerationSpec {
                tag,
                date: date.to_string(),
                fixver: n,
                git_short: Some(short.to_string()),
            };
        }
        n = n.saturating_add(1);
        if n > 10_000 {
            // safety valve
            return BuildGenerationSpec {
                tag: format_build_generation_tag_git(date, short, n),
                date: date.to_string(),
                fixver: n,
                git_short: Some(short.to_string()),
            };
        }
    }
}

fn version_identity_from_build_spec(
    spec: BuildGenerationSpec,
    meilang_version: String,
) -> VersionDisplayIdentity {
    VersionDisplayIdentity {
        build_generation: spec.tag.clone(),
        build_display_tag: format_build_display_tag(spec.tag.as_str()),
        env_generation_id: spec.tag,
        meilang_version,
    }
}

pub fn resolve_version_display_identity(source_root: &Path) -> VersionDisplayIdentity {
    resolve_version_display_identity_with_hint(source_root, None)
        .unwrap_or_else(|err| panic!("{err}"))
}

pub fn resolve_version_display_identity_with_hint(
    source_root: &Path,
    meilang_hint: Option<&str>,
) -> Result<VersionDisplayIdentity> {
    resolve_version_display_identity_for_app(source_root, None, meilang_hint)
}

/// Build generation for display: `env/current` of the given app, or workspace defaultApp.
pub fn resolve_version_display_identity_for_app(
    source_root: &Path,
    app_id: Option<&str>,
    meilang_hint: Option<&str>,
) -> Result<VersionDisplayIdentity> {
    let meilang_version = resolve_toolchain_version_with_hint(source_root, meilang_hint);

    if meilang_hint.is_some() {
        let spec = resolve_build_generation_for_prebuild(source_root);
        return Ok(version_identity_from_build_spec(spec, meilang_version));
    }

    let resolved_app = app_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| resolve_workspace_default_app_id(source_root));
    if let Some(app) = resolved_app.as_deref() {
        let app_root = resolve_app_root(source_root, app);
        let generation = resolve_app_build_generation_from_current(app_root.as_path())?;
        let spec = require_build_generation_tag(generation.as_str())?;
        return Ok(version_identity_from_build_spec(spec, meilang_version));
    }
    let spec = resolve_build_generation_config(source_root);
    Ok(version_identity_from_build_spec(spec, meilang_version))
}

pub fn today_yyyymmdd() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = (now / 86400) as i64;
    let (y, m, d) = super::paths::civil_from_days(days);
    format!("{y:04}{m:02}{d:02}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn parse_and_format_legacy_build_generation_tag() {
        let spec = parse_build_generation_tag("WS-20260630.1").expect("parse");
        assert_eq!(spec.date, "20260630");
        assert_eq!(spec.fixver, 1);
        assert_eq!(spec.git_short, None);
        assert_eq!(spec.tag, "WS-20260630.1");
    }

    #[test]
    fn parse_and_format_git_build_generation_tag() {
        let spec = parse_build_generation_tag("WS-20260801-abc1234").expect("parse");
        assert_eq!(spec.date, "20260801");
        assert_eq!(spec.fixver, 0);
        assert_eq!(spec.git_short.as_deref(), Some("abc1234"));
        assert_eq!(spec.tag, "WS-20260801-abc1234");

        let spec2 = parse_build_generation_tag("WS-20260801-abc1234.2").expect("parse");
        assert_eq!(spec2.fixver, 2);
        assert_eq!(spec2.tag, "WS-20260801-abc1234.2");
    }

    #[test]
    fn require_build_generation_tag_rejects_composite_legacy_id() {
        assert!(require_build_generation_tag("2.0.7-ws20260628").is_err());
    }

    #[test]
    fn auto_date_source_ignores_configured_date() {
        let tmp = tempdir().expect("tempdir");
        let ws = tmp.path();
        fs::write(
            ws.join("workspace.json"),
            r#"{"schemaVersion":2,"build":{"generation":{"dateSource":"auto","date":"19990101","fixver":2}}}"#,
        )
        .expect("write workspace.json");
        let spec = resolve_build_generation_for_prebuild(ws);
        assert_eq!(spec.date, today_yyyymmdd());
        assert_ne!(spec.date, "19990101");
        // no git → legacy form with fixver from config
        if spec.git_short.is_none() {
            assert_eq!(spec.fixver, 2);
            assert_eq!(spec.tag, format_build_generation_tag(spec.date.as_str(), 2));
        }
    }

    #[test]
    fn resolve_build_generation_for_prebuild_uses_auto_date() {
        let tmp = tempdir().expect("tempdir");
        let ws = tmp.path();
        fs::write(
            ws.join("workspace.json"),
            r#"{"schemaVersion":2,"build":{"generation":{"dateSource":"auto","fixver":0}}}"#,
        )
        .expect("write workspace.json");
        let spec = resolve_build_generation_for_prebuild(ws);
        assert!(spec.tag.starts_with("WS-"));
        assert!(is_build_generation_tag(&spec.tag));
    }

    #[test]
    fn allocate_git_generation_bumps_suffix() {
        let tmp = tempdir().expect("tempdir");
        let ws = tmp.path();
        fs::create_dir_all(ws.join("apps/demo/env/WS-20260801-deadbee")).expect("mkdir");
        let spec = allocate_git_generation(ws, "20260801", "deadbee");
        assert_eq!(spec.tag, "WS-20260801-deadbee.1");
        assert_eq!(spec.fixver, 1);
    }
}
