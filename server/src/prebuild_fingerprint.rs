use std::fs;
use std::hash::{Hash, Hasher};
use std::path::Path;

use anyhow::{Context, Result};
use mei_lang_kernel::{
    compile_revision_token_from_root_with_options, read_data_snapshot_import_manifest,
    resolve_app_root, resolve_runtime_warmup_manifest, CompileOptions, RuntimeWarmupManifest,
};
use serde::{Deserialize, Serialize};

pub const PREBUILD_INPUTS_SCHEMA_VERSION: &str = "v2";
pub const PREBUILD_STATE_SCHEMA_VERSION: &str = "mei-prebuild-state-v1";
pub const PREBUILD_STATE_REL: &str = "runtime/prebuild-state.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrebuildArtifactCoverageSummary {
    pub total_missing_artifacts: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedPrebuildState {
    pub schema_version: String,
    #[serde(rename = "inputsFingerprint")]
    pub inputs_fingerprint: String,
    #[serde(rename = "lastOkAtMs")]
    pub last_ok_at_ms: u64,
    #[serde(rename = "lastMode")]
    pub last_mode: String,
    #[serde(rename = "lastScopeProfile")]
    pub last_scope_profile: String,
    #[serde(rename = "succeededApps")]
    pub succeeded_apps: Vec<String>,
    #[serde(rename = "artifactCoverageSummary")]
    pub artifact_coverage_summary: PrebuildArtifactCoverageSummary,
}

#[derive(Debug, Clone)]
pub struct PrebuildFingerprintMatch {
    pub stored: PersistedPrebuildState,
}

pub fn prebuild_state_path(source_root: &Path) -> std::path::PathBuf {
    source_root.join(PREBUILD_STATE_REL)
}

pub fn load_prebuild_state(source_root: &Path) -> Result<Option<PersistedPrebuildState>> {
    let path = prebuild_state_path(source_root);
    if !path.is_file() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path)
        .with_context(|| format!("read prebuild state {}", path.display()))?;
    let state = serde_json::from_str::<PersistedPrebuildState>(&raw)
        .with_context(|| format!("parse prebuild state {}", path.display()))?;
    if state.schema_version != PREBUILD_STATE_SCHEMA_VERSION {
        return Ok(None);
    }
    Ok(Some(state))
}

pub fn persist_prebuild_state(source_root: &Path, state: &PersistedPrebuildState) -> Result<()> {
    let path = prebuild_state_path(source_root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_string_pretty(state)?)?;
    fs::rename(tmp, path)?;
    Ok(())
}

pub fn compute_prebuild_inputs_fingerprint(source_root: &Path) -> Result<String> {
    let manifest = resolve_runtime_warmup_manifest(source_root)?
        .ok_or_else(|| anyhow::anyhow!("warmup manifest unavailable"))?;
    let mut parts = vec![PREBUILD_INPUTS_SCHEMA_VERSION.to_string()];
    parts.push(platform_compile_revision_token());
    parts.push(canonical_manifest_token(&manifest)?);
    parts.push(workspace_warmup_token(source_root)?);
    for app in &manifest.apps {
        let app_root = resolve_app_root(source_root, app.app_id.as_str());
        parts.push(format!("app={}", app.app_id.trim()));
        parts.push(import_manifest_token(app_root.as_path())?);
        let token = compile_revision_token_from_root_with_options(
            source_root,
            app_root.as_path(),
            &CompileOptions::default(),
        )?;
        parts.push(format!("compile_revision={token}"));
        if crate::graph::feature::graph_registry_enabled() {
            parts.push(crate::graph::app_graph_fingerprint(source_root, app.app_id.as_str()));
        }
    }
    Ok(stable_hash(parts.join("\n").as_str()))
}

pub fn try_match_prebuild_fingerprint(source_root: &Path) -> Result<Option<PrebuildFingerprintMatch>> {
    let Some(stored) = load_prebuild_state(source_root)? else {
        return Ok(None);
    };
    let current = compute_prebuild_inputs_fingerprint(source_root)?;
    if stored.inputs_fingerprint != current {
        return Ok(None);
    }
    if stored.artifact_coverage_summary.total_missing_artifacts != 0 {
        return Ok(None);
    }
    Ok(Some(PrebuildFingerprintMatch { stored }))
}

fn platform_compile_revision_token() -> String {
    format!(
        "host={}|kernel={}",
        crate::build_info::BUILD_VERSION,
        mei_lang_kernel::platform_source_revision(),
    )
}

fn canonical_manifest_token(manifest: &RuntimeWarmupManifest) -> Result<String> {
    Ok(stable_hash(
        &serde_json::to_string(manifest).context("serialize warmup manifest")?,
    ))
}

fn workspace_warmup_token(source_root: &Path) -> Result<String> {
    let path = mei_lang_kernel::workspace_config_path(source_root);
    let raw = fs::read_to_string(&path)
        .with_context(|| format!("read workspace config {}", path.display()))?;
    let value: serde_json::Value =
        serde_json::from_str(&raw).context("parse workspace config for fingerprint")?;
    let warmup = value
        .get("warmup")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    Ok(stable_hash(
        &serde_json::to_string(&warmup).context("serialize warmup section")?,
    ))
}

fn import_manifest_token(app_root: &Path) -> Result<String> {
    let Some(manifest) = read_data_snapshot_import_manifest(app_root)? else {
        return Ok("import=missing".to_string());
    };
    let mut sigs = manifest
        .entries
        .iter()
        .map(|entry| entry.content_signature.clone())
        .collect::<Vec<_>>();
    sigs.sort();
    Ok(stable_hash(&sigs.join("|")))
}

fn stable_hash(text: &str) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_hash_is_deterministic() {
        assert_eq!(stable_hash("abc"), stable_hash("abc"));
        assert_ne!(stable_hash("abc"), stable_hash("abd"));
    }
}
