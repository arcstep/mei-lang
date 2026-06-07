use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use mei_lang_kernel::CompileWatchedFile;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::types::WorldScope;

pub const TOOLCHAIN_ARTIFACT_STORE_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactWriteContext {
    pub app_id: String,
    pub artifact_kind: String,
    pub artifact_name: String,
    pub scope: WorldScope,
    pub active_scene_id: Option<String>,
    pub active_target_file: String,
    pub revision_token: String,
    pub components_revision: u128,
    pub watched_files: Vec<ArtifactWatchedFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactStoreManifest {
    pub schema_version: String,
    pub store_version: u32,
    pub app_id: String,
    pub artifact_kind: String,
    pub artifact_name: String,
    pub scope: WorldScope,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_scene_id: Option<String>,
    pub active_target_file: String,
    pub revision_token: String,
    pub components_revision: u128,
    pub watched_files: Vec<ArtifactWatchedFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactStoreWriteResult {
    pub store_root: String,
    pub metadata_path: String,
    pub manifest_path: String,
    pub artifact_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ArtifactStoreMetadata {
    schema_version: String,
    store_version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactWatchedFile {
    pub rel_path: String,
    pub modified_ms: u128,
    pub size_bytes: u64,
}

impl From<&CompileWatchedFile> for ArtifactWatchedFile {
    fn from(value: &CompileWatchedFile) -> Self {
        Self {
            rel_path: value.rel_path.clone(),
            modified_ms: value.modified_ms,
            size_bytes: value.size_bytes,
        }
    }
}

pub fn toolchain_artifact_store_root(app_root: &Path) -> PathBuf {
    app_root.join(".mei")
}

fn sanitize_segment(value: &str) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if matches!(ch, '-' | '_' | '.') {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    let out = out.trim_matches('_');
    if out.is_empty() {
        "default".to_string()
    } else {
        out.to_string()
    }
}

fn scope_slug(scope: &WorldScope) -> String {
    let scene = sanitize_segment(scope.scene_id.as_deref().unwrap_or("default-scene"));
    let target = sanitize_segment(scope.target_file.as_deref().unwrap_or("default-target"));
    format!("{scene}__{target}")
}

fn ensure_metadata(root: &Path) -> Result<PathBuf> {
    fs::create_dir_all(root)
        .with_context(|| format!("failed to create toolchain artifact store {}", root.display()))?;
    let metadata_path = root.join("store.json");
    let metadata = ArtifactStoreMetadata {
        schema_version: "mei-toolchain-store-v1".to_string(),
        store_version: TOOLCHAIN_ARTIFACT_STORE_VERSION,
    };
    let raw = serde_json::to_string_pretty(&metadata)?;
    fs::write(&metadata_path, raw)
        .with_context(|| format!("failed to write store metadata {}", metadata_path.display()))?;
    Ok(metadata_path)
}

pub fn write_json_artifact(
    app_root: &Path,
    context: &ArtifactWriteContext,
    artifact: &Value,
) -> Result<ArtifactStoreWriteResult> {
    let root = toolchain_artifact_store_root(app_root);
    let metadata_path = ensure_metadata(&root)?;
    let name = format!(
        "{}__{}",
        sanitize_segment(&context.artifact_name),
        scope_slug(&context.scope)
    );
    let manifests_dir = root.join("manifests").join(&context.artifact_kind);
    let artifacts_dir = root.join("artifacts").join(&context.artifact_kind);
    fs::create_dir_all(&manifests_dir).with_context(|| {
        format!(
            "failed to create artifact manifest dir {}",
            manifests_dir.display()
        )
    })?;
    fs::create_dir_all(&artifacts_dir).with_context(|| {
        format!(
            "failed to create artifact payload dir {}",
            artifacts_dir.display()
        )
    })?;

    let manifest = ArtifactStoreManifest {
        schema_version: "mei-toolchain-artifact-manifest-v1".to_string(),
        store_version: TOOLCHAIN_ARTIFACT_STORE_VERSION,
        app_id: context.app_id.clone(),
        artifact_kind: context.artifact_kind.clone(),
        artifact_name: context.artifact_name.clone(),
        scope: context.scope.clone(),
        active_scene_id: context.active_scene_id.clone(),
        active_target_file: context.active_target_file.clone(),
        revision_token: context.revision_token.clone(),
        components_revision: context.components_revision,
        watched_files: context.watched_files.clone(),
    };

    let artifact_path = artifacts_dir.join(format!("{name}.json"));
    let manifest_path = manifests_dir.join(format!("{name}.json"));
    fs::write(&artifact_path, serde_json::to_string_pretty(artifact)?)
        .with_context(|| format!("failed to write artifact {}", artifact_path.display()))?;
    fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)
        .with_context(|| format!("failed to write artifact manifest {}", manifest_path.display()))?;

    Ok(ArtifactStoreWriteResult {
        store_root: root.display().to_string(),
        metadata_path: metadata_path.display().to_string(),
        manifest_path: manifest_path.display().to_string(),
        artifact_path: artifact_path.display().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::{scope_slug, write_json_artifact, ArtifactWriteContext, WorldScope};
    use serde_json::json;

    #[test]
    fn scope_slug_is_stable() {
        let slug = scope_slug(&WorldScope {
            scene_id: Some("home".to_string()),
            target_file: Some("scenes/home.mei".to_string()),
        });
        assert_eq!(slug, "home__scenes_home.mei");
    }

    #[test]
    fn write_json_artifact_creates_store_layout() {
        let root = std::env::temp_dir().join("mei-toolchain-artifact-store");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create temp root");
        let result = write_json_artifact(
            &root,
            &ArtifactWriteContext {
                app_id: "demo".to_string(),
                artifact_kind: "inventory_snapshot".to_string(),
                artifact_name: "inventory".to_string(),
                scope: WorldScope::default(),
                active_scene_id: Some("home".to_string()),
                active_target_file: "main.mei".to_string(),
                revision_token: "rev".to_string(),
                components_revision: 1,
                watched_files: Vec::new(),
            },
            &json!({"ok": true}),
        )
        .expect("write artifact");
        assert!(std::path::Path::new(&result.metadata_path).is_file());
        assert!(std::path::Path::new(&result.manifest_path).is_file());
        assert!(std::path::Path::new(&result.artifact_path).is_file());
    }
}
