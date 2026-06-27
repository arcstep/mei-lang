use std::path::Path;

use mei_lang_kernel::CompiledApp;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::graph::content_store::{self, SCENE_PAYLOAD};
use crate::graph::io::read_json_registry;
use crate::graph::types::stable_hash;

pub const SCENE_PAYLOAD_ARTIFACT_SCHEMA: &str = "mei-scene-payload-artifact-v3";
pub const SCENE_PAYLOAD_ARTIFACT_SCHEMA_V2: &str = "mei-scene-payload-artifact-v2";

pub fn scene_payload_uses_full_compiled_artifact(artifact: &ScenePayloadArtifact) -> bool {
    artifact.schema_version == SCENE_PAYLOAD_ARTIFACT_SCHEMA_V2
        || scene_payload_is_full_compiled_payload(&artifact.payload)
}

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

pub fn scene_payload_slim_value_from_compiled(compiled: &CompiledApp) -> Value {
    serde_json::to_value(serde_json::json!({
        "activeTargetFile": compiled.active_target_file,
        "activeScene": compiled.active_scene,
        "sceneContract": compiled.scene_contract,
        "sceneLocalNavByTarget": compiled.scene_local_nav_by_target,
        "sceneBindingsById": compiled.scene_bindings_by_id,
        "sceneExamplesById": compiled.scene_examples_by_id,
        "sceneProjectionAssemblyById": compiled.scene_projection_assembly_by_id,
        "componentAssets": compiled.component_assets,
        "diagnostics": compiled.diagnostics,
    }))
    .unwrap_or(Value::Null)
}

pub fn scene_payload_value_for_persist(compiled: &CompiledApp) -> Value {
    scene_payload_slim_value_from_compiled(compiled)
}

pub fn scene_payload_is_full_compiled_payload(payload: &Value) -> bool {
    payload.get("resources").is_some() || payload.get("appId").is_some()
}

pub fn merge_slim_scene_payload_into_compiled(compiled: &mut CompiledApp, payload: &Value) {
    if let Ok(target) = serde_json::from_value(payload.get("activeTargetFile").cloned().unwrap_or(Value::Null)) {
        compiled.active_target_file = target;
    }
    if let Ok(scene) = serde_json::from_value(payload.get("activeScene").cloned().unwrap_or(Value::Null)) {
        compiled.active_scene = scene;
    }
    if let Ok(contract) = serde_json::from_value(payload.get("sceneContract").cloned().unwrap_or(Value::Null)) {
        compiled.scene_contract = contract;
    }
    if let Ok(nav) = serde_json::from_value(
        payload
            .get("sceneLocalNavByTarget")
            .cloned()
            .unwrap_or(Value::Null),
    ) {
        compiled.scene_local_nav_by_target = nav;
    }
    if let Ok(bindings) = serde_json::from_value(
        payload
            .get("sceneBindingsById")
            .cloned()
            .unwrap_or(Value::Null),
    ) {
        compiled.scene_bindings_by_id = bindings;
    }
    if let Ok(examples) = serde_json::from_value(
        payload
            .get("sceneExamplesById")
            .cloned()
            .unwrap_or(Value::Null),
    ) {
        compiled.scene_examples_by_id = examples;
    }
    if let Ok(projection) = serde_json::from_value(
        payload
            .get("sceneProjectionAssemblyById")
            .cloned()
            .unwrap_or(Value::Null),
    ) {
        compiled.scene_projection_assembly_by_id = projection;
    }
    if let Ok(assets) = serde_json::from_value(
        payload
            .get("componentAssets")
            .cloned()
            .unwrap_or(Value::Null),
    ) {
        compiled.component_assets = assets;
    }
    if let Ok(diags) = serde_json::from_value(payload.get("diagnostics").cloned().unwrap_or(Value::Null)) {
        compiled.diagnostics = diags;
    }
}

pub fn compiled_from_scene_payload_artifact(
    artifact: &ScenePayloadArtifact,
    skeleton: Option<&super::app_skeleton::AppSkeletonArtifact>,
    app_id: &str,
    app_root: &str,
) -> Option<CompiledApp> {
    if scene_payload_uses_full_compiled_artifact(artifact) {
        return serde_json::from_value::<CompiledApp>(artifact.payload.clone()).ok();
    }
    let mut compiled = CompiledApp {
        app_id: app_id.to_string(),
        title: String::new(),
        app_root: app_root.to_string(),
        active_scene: None,
        active_target_file: artifact.target_file.clone(),
        scene_routes: Vec::new(),
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
        build_board_index: Default::default(),
        build_template_index: Default::default(),
    };
    if let Some(sk) = skeleton {
        super::app_skeleton::merge_app_skeleton_into_compiled(&mut compiled, sk);
    }
    merge_slim_scene_payload_into_compiled(&mut compiled, &artifact.payload);
    Some(compiled)
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
