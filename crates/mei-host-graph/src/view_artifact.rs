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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ViewRevisionStatus {
    Refetch,
    AssembleLocal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ClientLayerHolding {
    pub name: String,
    pub artifact_id: String,
    pub content_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AssemblyPlan {
    pub manifest: SceneViewManifest,
    pub layer_refs: std::collections::BTreeMap<String, LayerRef>,
    pub compose_defaults: ComposeRequest,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub optional_layers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ViewRevisionResponse {
    pub ready: bool,
    pub status: ViewRevisionStatus,
    pub semantic_core: SemanticCacheCore,
    pub manifest_revision_digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface_revision_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manifest: Option<SceneViewManifest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assembly_plan: Option<AssemblyPlan>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changed_layers: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inline_layers: Option<std::collections::BTreeMap<String, Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ViewRevisionInput {
    pub manifest: SceneViewManifest,
    #[serde(default)]
    pub client_layers: Vec<ClientLayerHolding>,
    #[serde(default)]
    pub local_miss: bool,
    #[serde(default)]
    pub missing_layers: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface_revision_digest: Option<String>,
}

pub fn layer_ref_from_manifest_entry(layer_name: &str, value: &Value) -> Option<LayerRef> {
    if let Ok(layer_ref) = serde_json::from_value::<LayerRef>(value.clone()) {
        if !layer_ref.artifact_id.is_empty() && !layer_ref.content_hash.is_empty() {
            return Some(layer_ref);
        }
    }
    let artifact_id = value
        .get("artifact_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|id| !id.is_empty())?;
    let content_hash = value
        .get("content_hash")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|hash| !hash.is_empty())
        .map(str::to_string)
        .or_else(|| {
            if layer_name == "layout.overlay" {
                value
                    .get("persisted")
                    .and_then(Value::as_str)
                    .map(|persisted| format!("overlay:{persisted}"))
            } else {
                None
            }
        })?;
    Some(LayerRef {
        artifact_id: artifact_id.to_string(),
        content_hash,
        bytes: value.get("bytes").and_then(Value::as_u64),
        encoding: value
            .get("encoding")
            .and_then(Value::as_str)
            .map(str::to_string),
    })
}

pub fn collect_manifest_layer_refs(
    manifest: &SceneViewManifest,
) -> std::collections::BTreeMap<String, LayerRef> {
    let mut refs = std::collections::BTreeMap::new();
    for (name, value) in &manifest.layers {
        if let Some(layer_ref) = layer_ref_from_manifest_entry(name.as_str(), value) {
            refs.insert(name.clone(), layer_ref);
        }
    }
    refs
}

pub fn resolve_view_revision(input: &ViewRevisionInput) -> ViewRevisionResponse {
    let manifest = &input.manifest;
    let layer_refs = collect_manifest_layer_refs(manifest);
    let compose_defaults = manifest.compose_defaults.clone().unwrap_or_default();
    let manifest_revision_digest = manifest.revision_digest.clone();

    let client_by_name: std::collections::BTreeMap<_, _> = input
        .client_layers
        .iter()
        .map(|holding| (holding.name.as_str(), holding))
        .collect();

    let mut stale_layers = Vec::new();
    for (name, server_ref) in &layer_refs {
        let client = client_by_name.get(name.as_str());
        let matches = client.is_some_and(|holding| {
            holding.artifact_id == server_ref.artifact_id
                && holding.content_hash == server_ref.content_hash
        });
        if !matches {
            stale_layers.push(name.clone());
        }
    }

    if input.local_miss {
        let mut changed = input.missing_layers.clone();
        for layer in stale_layers {
            if !changed.iter().any(|existing| existing == &layer) {
                changed.push(layer);
            }
        }
        return ViewRevisionResponse {
            ready: true,
            status: ViewRevisionStatus::Refetch,
            semantic_core: manifest.semantic_core.clone(),
            manifest_revision_digest,
            surface_revision_digest: input.surface_revision_digest.clone(),
            manifest: Some(manifest.clone()),
            assembly_plan: None,
            changed_layers: changed,
            inline_layers: None,
        };
    }

    if stale_layers.is_empty() {
        return ViewRevisionResponse {
            ready: true,
            status: ViewRevisionStatus::AssembleLocal,
            semantic_core: manifest.semantic_core.clone(),
            manifest_revision_digest,
            surface_revision_digest: input.surface_revision_digest.clone(),
            manifest: Some(manifest.clone()),
            assembly_plan: Some(AssemblyPlan {
                manifest: manifest.clone(),
                layer_refs,
                compose_defaults,
                optional_layers: Vec::new(),
            }),
            changed_layers: Vec::new(),
            inline_layers: None,
        };
    }

    ViewRevisionResponse {
        ready: true,
        status: ViewRevisionStatus::Refetch,
        semantic_core: manifest.semantic_core.clone(),
        manifest_revision_digest,
        surface_revision_digest: input.surface_revision_digest.clone(),
        manifest: Some(manifest.clone()),
        assembly_plan: None,
        changed_layers: stale_layers,
        inline_layers: None,
    }
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

pub fn manifest_revision_digest(manifest: &SceneViewManifest, draft_digest: Option<&str>) -> String {
    semantic_revision_digest(manifest, draft_digest)
}

/// Digest over semantic layers only (excludes shell.* and view-only compose axes).
pub fn semantic_revision_digest(manifest: &SceneViewManifest, draft_digest: Option<&str>) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut semantic_layers = std::collections::BTreeMap::new();
    for (name, value) in &manifest.layers {
        if !name.starts_with("shell.") {
            semantic_layers.insert(name.clone(), value.clone());
        }
    }
    let data_mode = manifest
        .compose_defaults
        .as_ref()
        .and_then(|compose| compose.data_mode.as_deref())
        .unwrap_or("");
    let payload = json!({
        "semantic_core": manifest.semantic_core,
        "layers": semantic_layers,
        "data_mode": data_mode,
        "draft_digest": draft_digest.filter(|value| !value.is_empty()),
    });
    let raw = serde_json::to_string(&payload).unwrap_or_default();
    let mut hasher = DefaultHasher::new();
    raw.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// Digest over shell layer + tab/chrome/route compose axes.
pub fn surface_revision_digest_from_manifest(manifest: &SceneViewManifest) -> Option<String> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let shell_layer = manifest
        .layers
        .iter()
        .find(|(name, _)| name.starts_with("shell."))
        .map(|(_, value)| value.clone());
    let compose = manifest.compose_defaults.as_ref();
    let payload = json!({
        "shell": shell_layer,
        "route_mode": compose.and_then(|value| value.route_mode.as_deref()),
        "tab": compose.and_then(|value| value.tab.as_deref()),
        "chrome": compose.and_then(|value| value.chrome.as_deref()),
        "review_projection": compose.and_then(|value| value.review_projection.as_deref()),
    });
    let raw = serde_json::to_string(&payload).unwrap_or_default();
    if raw == "null" || raw == "{}" {
        return None;
    }
    let mut hasher = DefaultHasher::new();
    raw.hash(&mut hasher);
    Some(format!("{:016x}", hasher.finish()))
}

#[cfg(test)]
mod manifest_revision_tests {
    use super::*;
    use crate::SceneViewManifest;

    #[test]
    fn manifest_revision_digest_changes_with_draft_digest() {
        let manifest = SceneViewManifest {
            schema_version: "1".to_string(),
            app_id: "demo".to_string(),
            scene_id: "home".to_string(),
            semantic_core: crate::SemanticCacheCore {
                app_id: "demo".to_string(),
                scene_id: "home".to_string(),
                preview_scope: None,
                registry_revision: "r1".to_string(),
                client_revision: "c1".to_string(),
                data_generation: "g1".to_string(),
                compile_epoch: "e1".to_string(),
            },
            revision_digest: String::new(),
            layers: Default::default(),
            compose_defaults: None,
        };
        let base = semantic_revision_digest(&manifest, None);
        let with_draft = semantic_revision_digest(&manifest, Some("draft-abc"));
        assert_ne!(base, with_draft);
    }

    #[test]
    fn semantic_revision_digest_ignores_shell_and_compose_view_axes() {
        let mut layers = std::collections::BTreeMap::new();
        layers.insert(
            "structure.full".to_string(),
            json!({"artifact_id": "s1", "content_hash": "h1"}),
        );
        layers.insert(
            "shell.app".to_string(),
            json!({"artifact_id": "sh1", "content_hash": "shell-a"}),
        );
        let base_manifest = SceneViewManifest {
            schema_version: "1".to_string(),
            app_id: "demo".to_string(),
            scene_id: "home".to_string(),
            semantic_core: crate::SemanticCacheCore {
                app_id: "demo".to_string(),
                scene_id: "home".to_string(),
                preview_scope: None,
                registry_revision: "r1".to_string(),
                client_revision: "c1".to_string(),
                data_generation: "g1".to_string(),
                compile_epoch: "e1".to_string(),
            },
            revision_digest: String::new(),
            layers: layers.clone(),
            compose_defaults: Some(ComposeRequest {
                route_mode: Some("app".to_string()),
                tab: Some("scene".to_string()),
                chrome: Some("full".to_string()),
                review_projection: Some("live_full".to_string()),
                data_mode: Some("eval".to_string()),
                focus: None,
                scope: None,
            }),
        };
        let mut shell_variant = base_manifest.clone();
        if let Some(shell) = shell_variant.layers.get_mut("shell.app") {
            *shell = json!({"artifact_id": "sh2", "content_hash": "shell-b"});
        }
        shell_variant.compose_defaults = Some(ComposeRequest {
            route_mode: Some("layout".to_string()),
            tab: Some("preview".to_string()),
            chrome: Some("none".to_string()),
            review_projection: Some("plane_region_section".to_string()),
            data_mode: Some("eval".to_string()),
            focus: None,
            scope: None,
        });
        let semantic_a = semantic_revision_digest(&base_manifest, None);
        let semantic_b = semantic_revision_digest(&shell_variant, None);
        assert_eq!(semantic_a, semantic_b);
        let surface_a = surface_revision_digest_from_manifest(&base_manifest);
        let surface_b = surface_revision_digest_from_manifest(&shell_variant);
        assert_ne!(surface_a, surface_b);
    }
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

#[cfg(test)]
mod view_revision_tests {
    use super::*;
    use crate::SemanticCacheCore;

    fn sample_manifest(structure_hash: &str, overlay_hash: &str) -> SceneViewManifest {
        let core = SemanticCacheCore {
            app_id: "demo".to_string(),
            scene_id: "home".to_string(),
            preview_scope: None,
            registry_revision: "r1".to_string(),
            client_revision: "c1".to_string(),
            data_generation: "g1".to_string(),
            compile_epoch: "e1".to_string(),
        };
        let mut layers = std::collections::BTreeMap::new();
        layers.insert(
            "structure.full".to_string(),
            json!(LayerRef {
                artifact_id: "struct-key".to_string(),
                content_hash: structure_hash.to_string(),
                bytes: None,
                encoding: None,
            }),
        );
        layers.insert(
            "layout.overlay".to_string(),
            json!({
                "artifact_id": "overlay-key",
                "content_hash": overlay_hash,
            }),
        );
        SceneViewManifest {
            schema_version: SCENE_VIEW_MANIFEST_SCHEMA.to_string(),
            app_id: "demo".to_string(),
            scene_id: "home".to_string(),
            semantic_core: core,
            revision_digest: "manifest-digest".to_string(),
            layers,
            compose_defaults: None,
        }
    }

    #[test]
    fn resolve_view_revision_assemble_local_when_all_layers_match() {
        let manifest = sample_manifest("hash-a", "hash-b");
        let response = resolve_view_revision(&ViewRevisionInput {
            manifest,
            client_layers: vec![
                ClientLayerHolding {
                    name: "structure.full".to_string(),
                    artifact_id: "struct-key".to_string(),
                    content_hash: "hash-a".to_string(),
                },
                ClientLayerHolding {
                    name: "layout.overlay".to_string(),
                    artifact_id: "overlay-key".to_string(),
                    content_hash: "hash-b".to_string(),
                },
            ],
            local_miss: false,
            missing_layers: Vec::new(),
            surface_revision_digest: None,
        });
        assert_eq!(response.status, ViewRevisionStatus::AssembleLocal);
        assert!(response.assembly_plan.is_some());
        assert!(response.changed_layers.is_empty());
    }

    #[test]
    fn resolve_view_revision_refetch_when_layer_stale() {
        let manifest = sample_manifest("hash-a-new", "hash-b");
        let response = resolve_view_revision(&ViewRevisionInput {
            manifest,
            client_layers: vec![ClientLayerHolding {
                name: "structure.full".to_string(),
                artifact_id: "struct-key".to_string(),
                content_hash: "hash-a-old".to_string(),
            }],
            local_miss: false,
            missing_layers: Vec::new(),
            surface_revision_digest: None,
        });
        assert_eq!(response.status, ViewRevisionStatus::Refetch);
        assert!(response
            .changed_layers
            .iter()
            .any(|layer| layer == "structure.full"));
    }

    #[test]
    fn resolve_view_revision_local_miss_forces_refetch() {
        let manifest = sample_manifest("hash-a", "hash-b");
        let response = resolve_view_revision(&ViewRevisionInput {
            manifest,
            client_layers: vec![ClientLayerHolding {
                name: "structure.full".to_string(),
                artifact_id: "struct-key".to_string(),
                content_hash: "hash-a".to_string(),
            }],
            local_miss: true,
            missing_layers: vec!["structure.full".to_string()],
            surface_revision_digest: None,
        });
        assert_eq!(response.status, ViewRevisionStatus::Refetch);
        assert!(response.assembly_plan.is_none());
        assert!(response
            .changed_layers
            .iter()
            .any(|layer| layer == "structure.full"));
    }

    #[test]
    fn draft_only_stale_overlay_not_structure_when_client_missing_overlay() {
        let manifest = sample_manifest("hash-a", "hash-b-new");
        let response = resolve_view_revision(&ViewRevisionInput {
            manifest,
            client_layers: vec![
                ClientLayerHolding {
                    name: "structure.full".to_string(),
                    artifact_id: "struct-key".to_string(),
                    content_hash: "hash-a".to_string(),
                },
                ClientLayerHolding {
                    name: "layout.overlay".to_string(),
                    artifact_id: "overlay-key".to_string(),
                    content_hash: "hash-b-old".to_string(),
                },
            ],
            local_miss: false,
            missing_layers: Vec::new(),
            surface_revision_digest: None,
        });
        assert_eq!(response.status, ViewRevisionStatus::Refetch);
        assert!(!response
            .changed_layers
            .iter()
            .any(|layer| layer == "structure.full"));
        assert!(response
            .changed_layers
            .iter()
            .any(|layer| layer == "layout.overlay"));
    }
}
