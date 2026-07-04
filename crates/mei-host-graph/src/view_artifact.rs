//! Layered view artifacts: structure.full, manifest, compose request, and cache keys.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::semantic_cache::{build_semantic_cache_core, SemanticCacheCore};

pub const STRUCTURE_FULL_KIND: &str = "structure_full";
pub const STRUCTURE_INDEX_KIND: &str = "structure_index";
pub const THEME_TOKENS_KIND: &str = "theme_tokens";
pub const LAYOUT_OVERLAY_KIND: &str = "layout_overlay";
pub const EVAL_SLOT_GROUP_KIND: &str = "eval_slot_group";
pub const SCENE_VIEW_MANIFEST_SCHEMA: &str = "scene-view-manifest-v1";
pub const STRUCTURE_FULL_SCHEMA: &str = "structure-full-v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StructureFullNode {
    pub node_id: String,
    pub ui_role: String,
    pub preview_scope: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plane: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_kind: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StructureFullDocument {
    pub schema_version: String,
    pub app_id: String,
    pub scene_id: String,
    pub semantic_revision: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scene_roots: Vec<String>,
    pub nodes: Vec<StructureFullNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LayerRef {
    pub artifact_id: String,
    pub content_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encoding: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ComposeRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tab: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chrome: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review_projection: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focus: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SceneViewManifest {
    pub schema_version: String,
    pub app_id: String,
    pub scene_id: String,
    pub semantic_core: SemanticCacheCore,
    pub revision_digest: String,
    pub layers: std::collections::BTreeMap<String, Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compose_defaults: Option<ComposeRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct WysiwygPanelPatch {
    pub preview_scope: String,
    pub ui_role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<Value>,
}

pub fn structure_full_cache_key(
    semantic_core: &SemanticCacheCore,
    layout_policy_revision: &str,
) -> String {
    let wrapper = json!({
        "artifact": STRUCTURE_FULL_KIND,
        "semantic_core": semantic_core,
        "layout_policy_revision": layout_policy_revision,
        "schema_version": STRUCTURE_FULL_SCHEMA,
    });
    serde_json::to_string(&wrapper).unwrap_or_else(|_| STRUCTURE_FULL_KIND.to_string())
}

pub fn eval_slot_group_cache_key(
    semantic_core: &SemanticCacheCore,
    slot_group_id: &str,
    data_mode: &str,
    filter_signature: &str,
) -> String {
    let wrapper = json!({
        "artifact": EVAL_SLOT_GROUP_KIND,
        "semantic_core": semantic_core,
        "slot_group_id": slot_group_id,
        "data_mode": data_mode,
        "filter_signature": filter_signature,
    });
    serde_json::to_string(&wrapper).unwrap_or_else(|_| EVAL_SLOT_GROUP_KIND.to_string())
}

pub fn theme_tokens_cache_key(theme_digest: &str) -> String {
    json!({
        "artifact": THEME_TOKENS_KIND,
        "theme_digest": theme_digest,
    })
    .to_string()
}

pub fn layout_overlay_persisted_cache_key(layout_policy_revision: &str) -> String {
    json!({
        "artifact": LAYOUT_OVERLAY_KIND,
        "surface": "persisted",
        "layout_policy_revision": layout_policy_revision,
    })
    .to_string()
}

pub fn layout_overlay_session_cache_key(app_id: &str, draft_session: &str, draft_digest: &str) -> String {
    json!({
        "artifact": LAYOUT_OVERLAY_KIND,
        "surface": "session",
        "app_id": app_id,
        "draft_session": draft_session,
        "draft_digest": draft_digest,
    })
    .to_string()
}

pub fn shell_cache_key(
    route_mode: &str,
    tab: &str,
    chrome: &str,
    auth_sig: Option<u64>,
    shell_schema_revision: &str,
) -> String {
    json!({
        "artifact": format!("shell.{route_mode}"),
        "tab": tab,
        "chrome": chrome,
        "auth_sig": auth_sig,
        "shell_schema_revision": shell_schema_revision,
    })
    .to_string()
}

pub fn manifest_revision_digest(manifest: &SceneViewManifest) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let raw = serde_json::to_string(manifest).unwrap_or_default();
    let mut hasher = DefaultHasher::new();
    raw.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

pub fn build_semantic_core_for_scene(
    workspace_root: &std::path::Path,
    app_id: &str,
    scene_id: &str,
) -> SemanticCacheCore {
    let registry = crate::mcg::registry::McgRegistryWriter::load(workspace_root, app_id);
    let registry_revision = registry.registry_revision.trim().to_string();
    let client_revision = crate::mrg::client_bootstrap::read_client_bootstrap(
        workspace_root,
        app_id,
        scene_id,
    )
    .map(|manifest| manifest.client_revision)
    .filter(|value| !value.trim().is_empty())
    .unwrap_or_else(|| crate::mrg::client_bootstrap::NO_CLIENT_BOOTSTRAP_REVISION.to_string());
    let app_root = mei_lang_kernel::resolve_app_root(workspace_root, app_id);
    let data_generation =
        mei_lang_kernel::load_cache_generation(app_root.as_path(), app_id).data_generation;
    let compile_epoch = crate::mrg::client_bootstrap::read_client_bootstrap(
        workspace_root,
        app_id,
        scene_id,
    )
    .map(|manifest| manifest.workset_id)
    .filter(|value| !value.trim().is_empty())
    .unwrap_or_else(|| client_revision.clone());
    build_semantic_cache_core(
        app_id,
        scene_id,
        None,
        registry_revision,
        client_revision,
        data_generation,
        compile_epoch,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structure_key_ignores_route_and_projection() {
        let core = SemanticCacheCore {
            app_id: "demo".to_string(),
            scene_id: "home".to_string(),
            preview_scope: None,
            registry_revision: "r1".to_string(),
            client_revision: "c1".to_string(),
            data_generation: "g1".to_string(),
            compile_epoch: "e1".to_string(),
        };
        let key_a = structure_full_cache_key(&core, "layout0");
        let key_b = structure_full_cache_key(&core, "layout0");
        assert_eq!(key_a, key_b);
        assert!(!key_a.contains("plane_region"));
    }

    #[test]
    fn eval_key_includes_data_mode_not_route() {
        let core = build_semantic_cache_core("demo", "home", None, "r", "c", "g", "e");
        let eval = eval_slot_group_cache_key(&core, "panel:left", "eval", "default");
        assert!(eval.contains("eval"));
        assert!(!eval.contains("build"));
    }
}
