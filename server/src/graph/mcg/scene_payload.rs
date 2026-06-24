use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::graph::io::write_json_registry;
use crate::graph::paths::scene_payload_artifact_dir;
use crate::graph::types::stable_hash;

pub const SCENE_PAYLOAD_ARTIFACT_SCHEMA: &str = "mei-scene-payload-artifact-v1";

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
