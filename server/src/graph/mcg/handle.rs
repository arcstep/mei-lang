//! Assembly view handle: MCG node references without retaining full `CompiledApp`.

use std::path::Path;

use mei_lang_kernel::CompiledApp;

use crate::graph::mcg::registry::{McgRegistry, McgRegistryWriter};
use crate::graph::types::GraphNodeKind;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct AssemblyViewHandle {
    pub app_id: String,
    pub active_scene: Option<String>,
    pub active_target_file: String,
    pub scene_payload_hash: Option<String>,
    pub skeleton_hash: Option<String>,
    pub compile_revision: String,
    pub panel_keys: Vec<String>,
    pub projection_keys: Vec<String>,
}

impl AssemblyViewHandle {
    pub fn from_compiled_outcome(
        app_id: &str,
        active_scene: Option<&str>,
        active_target_file: &str,
        compile_revision: &str,
        scene_payload_hash: Option<String>,
        skeleton_hash: Option<String>,
        panel_keys: Vec<String>,
        projection_keys: Vec<String>,
    ) -> Self {
        Self {
            app_id: app_id.to_string(),
            active_scene: active_scene
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            active_target_file: active_target_file.trim().to_string(),
            scene_payload_hash,
            skeleton_hash,
            compile_revision: compile_revision.to_string(),
            panel_keys,
            projection_keys,
        }
    }

    pub fn from_mcg_registry(
        source_root: &Path,
        app_id: &str,
        compiled: &CompiledApp,
        compile_revision: &str,
    ) -> Self {
        let mcg = McgRegistryWriter::load(source_root, app_id);
        Self::from_compiled_and_registry(app_id, compiled, compile_revision, &mcg)
    }

    pub fn from_compiled_and_registry(
        app_id: &str,
        compiled: &CompiledApp,
        compile_revision: &str,
        mcg: &McgRegistry,
    ) -> Self {
        let target = compiled.active_target_file.trim();
        let canonical_target = mei_lang_kernel::canonical_app_source_rel_path(target);
        let scene_payload_hash = payload_hash_for_kind_key(mcg, GraphNodeKind::ScenePayload, &canonical_target);
        let skeleton_hash =
            payload_hash_for_kind_key(mcg, GraphNodeKind::AppSkeleton, app_id);
        let scene_id = compiled
            .active_scene
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("default");
        let panel_keys: Vec<String> = mcg
            .nodes
            .iter()
            .filter(|node| node.id.kind == GraphNodeKind::PanelContract)
            .filter(|node| node.id.key.starts_with(&format!("{scene_id}:")))
            .map(|node| node.id.key.clone())
            .collect();
        let projection_keys: Vec<String> = mcg
            .nodes
            .iter()
            .filter(|node| node.id.kind == GraphNodeKind::AssemblyView)
            .filter(|node| node.id.key.starts_with(&format!("{canonical_target}#")))
            .map(|node| node.id.key.clone())
            .collect();
        Self::from_compiled_outcome(
            app_id,
            compiled.active_scene.as_deref(),
            target,
            compile_revision,
            scene_payload_hash,
            skeleton_hash,
            panel_keys,
            projection_keys,
        )
    }
}

fn payload_hash_for_kind_key(
    mcg: &McgRegistry,
    kind: GraphNodeKind,
    key: &str,
) -> Option<String> {
    mcg.nodes
        .iter()
        .find(|node| node.id.kind == kind && node.id.key == key)
        .and_then(|node| node.payload_ref.as_ref())
        .map(|payload| payload.content_hash.clone())
}

pub fn hydrate_handle_for_eval(
    source_root: &Path,
    handle: &AssemblyViewHandle,
) -> anyhow::Result<CompiledApp> {
    let scene = handle.active_scene.as_deref();
    let target = handle.active_target_file.as_str();
    let (mut compiled, _) = crate::graph::try_assemble_scope_from_scene_payload(
        source_root,
        handle.app_id.as_str(),
        scene,
        target,
    )
    .ok_or_else(|| anyhow::anyhow!(
        "assemble from handle failed for target `{}`",
        target
    ))?;
    crate::graph::hydrate_compiled_for_prebuild_eval(
        source_root,
        handle.app_id.as_str(),
        &mut compiled,
        &[],
        &[],
    )?;
    Ok(compiled)
}
