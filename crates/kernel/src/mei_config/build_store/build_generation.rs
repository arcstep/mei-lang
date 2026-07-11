//! Build generation tags (`WS-yyyymmdd.fixver`) and version display helpers.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;

use crate::mei_config::io::load_workspace_config;
use crate::mei_config::types::WorkspaceBuildGenerationConfig;
use crate::mei_config::workspace_paths::resolve_app_root;

use super::env_paths::{
    resolve_app_build_generation_from_current, resolve_workspace_default_app_id,
};
use super::paths::resolve_toolchain_version_with_hint;

const BUILD_GENERATION_PREFIX: &str = "WS-";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildGenerationSpec {
    /// Canonical tag, e.g. `WS-20260630.0`.
    pub tag: String,
    pub date: String,
    pub fixver: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionDisplayIdentity {
    /// MeiLang / toolchain semver shown to users (`x.y.z`).
    pub meilang_version: String,
    /// Canonical build generation tag (`WS-yyyymmdd.fixver`).
    pub build_generation: String,
    /// Human label: `Build WS-yyyymmdd.fixver`.
    pub build_display_tag: String,
    /// Internal env directory id (active build pointer).
    pub env_generation_id: String,
}

pub fn format_build_generation_tag(date: &str, fixver: u32) -> String {
    format!("{BUILD_GENERATION_PREFIX}{date}.{fixver}")
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

pub fn parse_build_generation_tag(raw: &str) -> Option<BuildGenerationSpec> {
    let trimmed = raw.trim();
    let rest = trimmed.strip_prefix(BUILD_GENERATION_PREFIX)?;
    let (date, fixver_raw) = rest.rsplit_once('.')?;
    let date = date.trim();
    if date.len() != 8 || !date.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    let fixver: u32 = fixver_raw.parse().ok()?;
    Some(BuildGenerationSpec {
        tag: format_build_generation_tag(date, fixver),
        date: date.to_string(),
        fixver,
    })
}

pub fn is_build_generation_tag(raw: &str) -> bool {
    parse_build_generation_tag(raw).is_some()
}

pub fn require_build_generation_tag(raw: &str) -> Result<BuildGenerationSpec> {
    parse_build_generation_tag(raw).ok_or_else(|| {
        anyhow::anyhow!("invalid build generation `{raw}` (expected WS-yyyymmdd.fixver)")
    })
}

pub fn resolve_build_generation_config(source_root: &Path) -> BuildGenerationSpec {
    let cfg = load_workspace_config(source_root);
    resolve_build_generation_from_config(&cfg.build.generation)
}

/// Prebuild: honour `dateSource=auto` by using today's date.
pub fn resolve_build_generation_for_prebuild(source_root: &Path) -> BuildGenerationSpec {
    let cfg = load_workspace_config(source_root);
    resolve_build_generation_from_config(&cfg.build.generation)
}

fn resolve_build_generation_from_config(
    gen: &WorkspaceBuildGenerationConfig,
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
        // `auto` (default): compile-day date; ignore any configured `date`.
        _ => today_yyyymmdd(),
    };
    BuildGenerationSpec {
        tag: format_build_generation_tag(date.as_str(), fixver),
        date,
        fixver,
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

    // Host-shell SSOT: running binary version + compile-day build generation (fixver from config).
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
    fn parse_and_format_build_generation_tag() {
        let spec = parse_build_generation_tag("WS-20260630.1").expect("parse");
        assert_eq!(spec.date, "20260630");
        assert_eq!(spec.fixver, 1);
        assert_eq!(spec.tag, "WS-20260630.1");
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
        assert_eq!(spec.fixver, 2);
        assert_eq!(spec.date, today_yyyymmdd());
        assert_ne!(spec.date, "19990101");
        assert_eq!(spec.tag, format_build_generation_tag(spec.date.as_str(), 2));
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
        assert!(spec.tag.ends_with(".0"));
    }
}
