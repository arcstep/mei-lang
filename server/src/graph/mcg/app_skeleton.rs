//! App-level skeleton payload (build indexes, file tree) — single canonical copy per app.

use std::path::Path;

use mei_lang_kernel::CompiledApp;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::graph::content_store::{self, content_store_enabled};
use crate::graph::io::{read_json_registry, write_json_registry};

pub const APP_SKELETON_ARTIFACT_SCHEMA: &str = "mei-app-skeleton-artifact-v1";
pub const APP_SKELETON_REL: &str = ".mei/graph/payloads/app-skeleton.json";

pub fn app_skeleton_artifact_path(app_root: &Path) -> std::path::PathBuf {
    app_root.join(APP_SKELETON_REL)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSkeletonArtifact {
    #[serde(rename = "schemaVersion")]
    pub schema_version: String,
    pub revision: String,
    pub payload: Value,
}

#[derive(Debug, Clone)]
pub struct PersistedAppSkeleton {
    pub relative_path: String,
    pub content_hash: Option<String>,
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
) -> anyhow::Result<PersistedAppSkeleton> {
    let artifact = AppSkeletonArtifact {
        schema_version: APP_SKELETON_ARTIFACT_SCHEMA.to_string(),
        revision: revision.to_string(),
        payload: app_skeleton_value_from_compiled(compiled),
    };
    let bytes = serde_json::to_vec(&artifact)?;
    if content_store_enabled() {
        let put = content_store::put_if_absent(app_root, "app_skeleton", &bytes)?;
        let rel = put
            .path
            .strip_prefix(app_root)
            .map(|path| path.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| put.path.display().to_string());
        return Ok(PersistedAppSkeleton {
            relative_path: rel,
            content_hash: Some(put.content_hash),
        });
    }
    let path = app_skeleton_artifact_path(app_root);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    write_json_registry(&path, &artifact)?;
    Ok(PersistedAppSkeleton {
        relative_path: APP_SKELETON_REL.to_string(),
        content_hash: None,
    })
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

pub fn load_app_skeleton_artifact(
    app_root: &Path,
    content_hash: Option<&str>,
) -> anyhow::Result<Option<AppSkeletonArtifact>> {
    if let Some(hash) = content_hash.map(str::trim).filter(|value| !value.is_empty()) {
        let pref = crate::graph::types::PayloadRef {
            kind: "app_skeleton".to_string(),
            relative_path: String::new(),
            schema_version: APP_SKELETON_ARTIFACT_SCHEMA.to_string(),
            content_hash: Some(hash.to_string()),
        };
        if let Some(path) = content_store::resolve_payload_ref(app_root, &pref) {
            if let Some(artifact) = read_json_registry::<AppSkeletonArtifact>(&path)? {
                return Ok(Some(artifact));
            }
        }
        if let Some(path) = content_store::get(app_root, "app_skeleton", hash) {
            if let Some(artifact) = read_json_registry::<AppSkeletonArtifact>(&path)? {
                return Ok(Some(artifact));
            }
        }
    }
    let path = app_skeleton_artifact_path(app_root);
    if !path.is_file() {
        let legacy = app_root.join("build/active/graph/payloads/app-skeleton.json");
        if legacy.is_file() {
            return read_json_registry(&legacy);
        }
        return Ok(None);
    }
    read_json_registry(&path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_app_skeleton_reads_payloads_root_not_scene_subdir() {
        let app_root = std::env::temp_dir().join(format!(
            "mei-app-sk-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(app_root.join(".mei/graph/payloads")).expect("mkdir");
        std::fs::write(
            app_skeleton_artifact_path(&app_root),
            r#"{"schemaVersion":"mei-app-skeleton-artifact-v1","revision":"sk:test","payload":{}}"#,
        )
        .expect("write");
        let loaded = load_app_skeleton_artifact(&app_root, None)
            .expect("load")
            .expect("some");
        assert_eq!(loaded.revision, "sk:test");
        let _ = std::fs::remove_dir_all(&app_root);
    }
}
