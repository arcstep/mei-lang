//! App-level skeleton payload (build indexes, file tree) — single canonical copy per app.

use std::path::Path;

use mei_lang_kernel::CompiledApp;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::graph::io::{read_json_registry, write_json_registry};
use crate::graph::paths::scene_payload_artifact_dir;

pub const APP_SKELETON_ARTIFACT_SCHEMA: &str = "mei-app-skeleton-artifact-v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSkeletonArtifact {
    #[serde(rename = "schemaVersion")]
    pub schema_version: String,
    pub revision: String,
    pub payload: Value,
}

pub fn app_skeleton_revision(dependency_fingerprint: &str) -> String {
    format!(
        "sk:{}",
        crate::graph::types::stable_hash(dependency_fingerprint)
    )
}

pub fn app_skeleton_value_from_compiled(compiled: &CompiledApp) -> Value {
    serde_json::to_value(serde_json::json!({
        "fileTree": compiled.file_tree,
        "buildExperienceIndex": compiled.build_experience_index,
        "buildBoardIndex": compiled.build_board_index,
        "buildTemplateIndex": compiled.build_template_index,
        "sceneRoutes": compiled.scene_routes,
    }))
    .unwrap_or(Value::Null)
}

pub fn persist_app_skeleton_artifact(
    app_root: &Path,
    revision: &str,
    compiled: &CompiledApp,
) -> anyhow::Result<String> {
    let dir = scene_payload_artifact_dir(app_root);
    std::fs::create_dir_all(&dir)?;
    let rel = ".mei/graph/payloads/app-skeleton.json".to_string();
    let path = app_root.join(&rel);
    let artifact = AppSkeletonArtifact {
        schema_version: APP_SKELETON_ARTIFACT_SCHEMA.to_string(),
        revision: revision.to_string(),
        payload: app_skeleton_value_from_compiled(compiled),
    };
    write_json_registry(&path, &artifact)?;
    Ok(rel)
}

pub fn merge_app_skeleton_into_compiled(compiled: &mut CompiledApp, skeleton: &AppSkeletonArtifact) {
    let payload = &skeleton.payload;
    if let Ok(tree) = serde_json::from_value(payload.get("fileTree").cloned().unwrap_or(Value::Null)) {
        compiled.file_tree = tree;
    }
    if let Ok(index) = serde_json::from_value(
        payload
            .get("buildExperienceIndex")
            .cloned()
            .unwrap_or(Value::Null),
    ) {
        compiled.build_experience_index = index;
    }
    if let Ok(index) = serde_json::from_value(
        payload
            .get("buildBoardIndex")
            .cloned()
            .unwrap_or(Value::Null),
    ) {
        compiled.build_board_index = index;
    }
    if let Ok(index) = serde_json::from_value(
        payload
            .get("buildTemplateIndex")
            .cloned()
            .unwrap_or(Value::Null),
    ) {
        compiled.build_template_index = index;
    }
    if let Ok(routes) = serde_json::from_value(
        payload
            .get("sceneRoutes")
            .cloned()
            .unwrap_or(Value::Null),
    ) {
        compiled.scene_routes = routes;
    }
}

pub fn load_app_skeleton_artifact(app_root: &Path) -> anyhow::Result<Option<AppSkeletonArtifact>> {
    let path = scene_payload_artifact_dir(app_root).join("app-skeleton.json");
    if !path.is_file() {
        return Ok(None);
    }
    read_json_registry(&path)
}
