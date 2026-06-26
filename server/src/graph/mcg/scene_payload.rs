use std::path::Path;

use mei_lang_kernel::CompiledApp;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::graph::content_store::{self, SCENE_PAYLOAD};
use crate::graph::io::read_json_registry;
use crate::graph::types::stable_hash;

pub const SCENE_PAYLOAD_ARTIFACT_SCHEMA: &str = "mei-scene-payload-artifact-v2";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenePayloadArtifact {
    #[serde(rename = "schemaVersion")]
    pub schema_version: String,
    #[serde(rename = "targetFile")]
    pub target_file: String,
    pub revision: String,
    pub payload: Value,
}

#[derive(Debug, Clone)]
pub struct PersistedScenePayload {
    pub content_hash: String,
}

pub fn scene_payload_revision(target_file: &str, dependency_fingerprint: &str) -> String {
    format!(
        "nr:{}:{}",
        mei_lang_kernel::scene_payload_cache_epoch(),
        stable_hash(&format!("{target_file}\n{dependency_fingerprint}"))
    )
}

pub fn scene_payload_value_from_compiled(compiled: &CompiledApp) -> Value {
    serde_json::to_value(compiled).unwrap_or_else(|_| {
        serde_json::json!({
            "activeTargetFile": compiled.active_target_file,
            "activeScene": compiled.active_scene,
        })
    })
}

/// Reject legacy stub payloads that cannot back metric/world artifact planning.
pub fn scene_payload_is_assemblable(compiled: &CompiledApp) -> bool {
    if compiled.active_target_file.trim().is_empty() {
        return false;
    }
    if compiled.scene_contract.is_some() {
        return true;
    }
    if !compiled.scene_bindings_by_id.is_empty() || !compiled.scene_projection_assembly_by_id.is_empty()
    {
        return true;
    }
    compiled.resources.iter().any(|resource| {
        resource
            .dataset
            .as_ref()
            .is_some_and(|dataset| dataset.has_runtime_metric_defs())
    }) || !compiled.world_metrics.is_empty()
}

pub fn persist_scene_payload_artifact(
    app_root: &Path,
    target_file: &str,
    revision: &str,
    payload: &Value,
) -> anyhow::Result<PersistedScenePayload> {
    let artifact = ScenePayloadArtifact {
        schema_version: SCENE_PAYLOAD_ARTIFACT_SCHEMA.to_string(),
        target_file: target_file.to_string(),
        revision: revision.to_string(),
        payload: payload.clone(),
    };
    let bytes = serde_json::to_vec(&artifact)?;
    let put = content_store::put_if_absent(app_root, SCENE_PAYLOAD, &bytes)?;
    if put.created {
        tracing::debug!(
            content_hash = %put.content_hash,
            "scene payload content store blob created"
        );
    }
    Ok(PersistedScenePayload {
        content_hash: put.content_hash,
    })
}

pub fn load_scene_payload_artifact(
    app_root: &Path,
    target_file: &str,
    expected_revision: Option<&str>,
    content_hash: Option<&str>,
) -> anyhow::Result<Option<ScenePayloadArtifact>> {
    if let Some(hash) = content_hash.map(str::trim).filter(|value| !value.is_empty()) {
        if let Some(path) = content_store::get(app_root, SCENE_PAYLOAD, hash) {
            if let Some(artifact) = read_json_registry::<ScenePayloadArtifact>(&path)? {
                if artifact_matches_target(&artifact, target_file, expected_revision) {
                    return Ok(Some(artifact));
                }
            }
        }
    }
    Ok(None)
}

fn artifact_matches_target(
    artifact: &ScenePayloadArtifact,
    target_file: &str,
    expected_revision: Option<&str>,
) -> bool {
    if artifact.target_file.trim() != target_file.trim() {
        return false;
    }
    if let Some(expected) = expected_revision {
        if artifact.revision != expected {
            return false;
        }
    }
    true
}
