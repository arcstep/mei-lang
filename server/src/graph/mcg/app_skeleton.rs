//! App-level skeleton payload (build indexes, file tree) — single canonical copy per app.

use std::path::Path;

use mei_lang_kernel::CompiledApp;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::graph::content_store::{self, APP_SKELETON};
use crate::graph::io::read_json_registry;

pub const APP_SKELETON_ARTIFACT_SCHEMA: &str = "mei-app-skeleton-artifact-v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSkeletonArtifact {
    #[serde(rename = "schemaVersion")]
    pub schema_version: String,
    pub revision: String,
    pub payload: Value,
}

#[derive(Debug, Clone)]
pub struct PersistedAppSkeleton {
    pub content_hash: String,
}

pub fn app_skeleton_revision(dependency_fingerprint: &str) -> String {
    format!(
        "sk:{}",
        crate::graph::types::stable_hash(dependency_fingerprint)
    )
}

pub fn app_skeleton_value_from_compiled(compiled: &CompiledApp) -> Value {
    serde_json::to_value(serde_json::json!({
        "appId": compiled.app_id,
        "title": compiled.title,
        "fileTree": compiled.file_tree,
        "buildExperienceIndex": compiled.build_experience_index,
        "buildBoardIndex": compiled.build_t2_page_index,
        "buildTemplateIndex": compiled.build_template_index,
        "sceneRoutes": compiled.scene_routes,
        "resources": compiled.resources,
        "worldMetrics": compiled.world_metrics,
        "worldSemanticByFile": compiled.world_semantic_by_file,
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
    let put = content_store::put_if_absent(app_root, APP_SKELETON, &bytes)?;
    Ok(PersistedAppSkeleton {
        content_hash: put.content_hash,
    })
}

pub fn merge_app_skeleton_into_compiled(
    compiled: &mut CompiledApp,
    skeleton: &AppSkeletonArtifact,
) {
    let payload = &skeleton.payload;
    if let Ok(tree) =
        serde_json::from_value(payload.get("fileTree").cloned().unwrap_or(Value::Null))
    {
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
        compiled.build_t2_page_index = index;
    }
    if let Ok(index) = serde_json::from_value(
        payload
            .get("buildTemplateIndex")
            .cloned()
            .unwrap_or(Value::Null),
    ) {
        compiled.build_template_index = index;
    }
    if let Ok(routes) =
        serde_json::from_value(payload.get("sceneRoutes").cloned().unwrap_or(Value::Null))
    {
        compiled.scene_routes = routes;
    }
    if let Some(app_id) = payload
        .get("appId")
        .and_then(|v| v.as_str())
        .map(str::to_string)
    {
        if !app_id.trim().is_empty() {
            compiled.app_id = app_id;
        }
    }
    if let Ok(title) =
        serde_json::from_value::<String>(payload.get("title").cloned().unwrap_or(Value::Null))
    {
        compiled.title = title;
    }
    if let Ok(resources) =
        serde_json::from_value(payload.get("resources").cloned().unwrap_or(Value::Null))
    {
        compiled.resources = resources;
    }
    if let Ok(world_metrics) =
        serde_json::from_value(payload.get("worldMetrics").cloned().unwrap_or(Value::Null))
    {
        compiled.world_metrics = world_metrics;
    }
    if let Ok(semantic) = serde_json::from_value(
        payload
            .get("worldSemanticByFile")
            .cloned()
            .unwrap_or(Value::Null),
    ) {
        compiled.world_semantic_by_file = semantic;
    }
}

pub fn load_app_skeleton_from_mcg(
    source_root: &Path,
    app_id: &str,
) -> anyhow::Result<Option<AppSkeletonArtifact>> {
    let mcg = crate::graph::mcg::registry::McgRegistryWriter::load(source_root, app_id);
    let Some(hash) = mcg
        .nodes
        .iter()
        .find(|node| node.id.kind == crate::graph::types::GraphNodeKind::AppSkeleton)
        .and_then(|node| node.payload_ref.as_ref())
        .map(|payload| payload.content_hash.as_str())
        .filter(|hash| !hash.is_empty())
    else {
        return Ok(None);
    };
    let app_root = mei_lang_kernel::resolve_app_root(source_root, app_id);
    load_app_skeleton_artifact(app_root.as_path(), hash)
}
pub fn load_app_skeleton_artifact(
    app_root: &Path,
    content_hash: &str,
) -> anyhow::Result<Option<AppSkeletonArtifact>> {
    let hash = content_hash.trim();
    if hash.is_empty() {
        return Ok(None);
    }
    let Some(path) = content_store::get(app_root, APP_SKELETON, hash) else {
        return Ok(None);
    };
    read_json_registry(&path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persist_and_load_app_skeleton_via_cas() {
        let app_root = std::env::temp_dir().join(format!(
            "mei-app-sk-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(app_root.join("build/active")).expect("mkdir");
        let compiled = CompiledApp {
            app_id: "demo".to_string(),
            title: String::new(),
            app_root: String::new(),
            scene_routes: Vec::new(),
            active_scene: None,
            stage_registry: Default::default(),
            stage_programs: Default::default(),
            scene_slot_modules: Default::default(),
            content_capabilities: Default::default(),
            narration_catalogs: Default::default(),
            active_target_file: String::new(),
            file_tree: Vec::new(),
            scene_contract: None,
            scene_local_nav_by_target: Default::default(),
            scene_bindings_by_id: Default::default(),
            scene_examples_by_id: Default::default(),
            scene_projection_assembly_by_id: Default::default(),
            resources: Vec::new(),
            world_metrics: Default::default(),
            world_semantic_by_file: Default::default(),
            component_assets: Vec::new(),
            diagnostics: Vec::new(),
            build_experience_index: Default::default(),
            build_t2_page_index: Default::default(),
            build_template_index: Default::default(),
        };
        let persisted =
            persist_app_skeleton_artifact(&app_root, "sk:test", &compiled).expect("persist");
        let loaded = load_app_skeleton_artifact(&app_root, persisted.content_hash.as_str())
            .expect("load")
            .expect("some");
        assert_eq!(loaded.revision, "sk:test");
        let _ = std::fs::remove_dir_all(&app_root);
    }
}
