use std::path::Path;

use mei_lang_kernel::CompiledApp;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::graph::io::{read_json_registry, write_json_registry};
use crate::graph::paths::scene_payload_artifact_dir;
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
) -> anyhow::Result<String> {
    let slug = scope_slug(target_file);
    let dir = scene_payload_artifact_dir(app_root);
    std::fs::create_dir_all(&dir)?;
    let rel = format!(".mei/graph/payloads/scene/{slug}.json");
    let path = app_root.join(&rel);
    let artifact = ScenePayloadArtifact {
        schema_version: SCENE_PAYLOAD_ARTIFACT_SCHEMA.to_string(),
        target_file: target_file.to_string(),
        revision: revision.to_string(),
        payload: payload.clone(),
    };
    write_json_registry(&path, &artifact)?;
    Ok(rel)
}

pub fn load_scene_payload_artifact(
    app_root: &Path,
    target_file: &str,
    expected_revision: Option<&str>,
) -> anyhow::Result<Option<ScenePayloadArtifact>> {
    let slug = scope_slug(target_file);
    let path = scene_payload_artifact_dir(app_root).join(format!("{slug}.json"));
    if !path.is_file() {
        return Ok(None);
    }
    let Some(artifact) = read_json_registry::<ScenePayloadArtifact>(&path)? else {
        return Ok(None);
    };
    if artifact.target_file.trim() != target_file.trim() {
        return Ok(None);
    }
    if let Some(expected) = expected_revision {
        if artifact.revision != expected {
            return Ok(None);
        }
    }
    Ok(Some(artifact))
}

pub fn scope_slug(target_file: &str) -> String {
    target_file
        .trim()
        .trim_start_matches('/')
        .replace(['/', '.'], "-")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_slug_normalizes() {
        assert_eq!(scope_slug("scenes/home.mei"), "scenes-home-mei");
    }
}
