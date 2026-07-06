//! `eval.slot_group[*]` artifact helpers.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;
use mei_lang_kernel::{CompiledApp, DataMode, MetricShape};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::content_store::{put_if_absent, EVAL_SLOT_GROUP_KIND};
use crate::layer_store::{layer_entry_meta, store_layer, take_layer};
use crate::mrg::registry::MrgRegistryWriter;
use crate::semantic_cache::SemanticCacheCore;
use crate::structure_full::slot_group_id_for_node;
use crate::structure_full::build_structure_full_document;
use crate::types::{MaterialState, PayloadRef};
use crate::view_artifact::eval_slot_group_cache_key;

pub const EVAL_SLOT_GROUP_SCHEMA: &str = "eval-slot-group-v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvalSlotGroupDocument {
    pub schema_version: String,
    pub slot_group_id: String,
    pub data_mode: String,
    pub slots: BTreeMap<String, Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bootstrap_seed: Option<Value>,
}

pub fn collect_slot_groups(structure: &crate::view_artifact::StructureFullDocument) -> Vec<String> {
    let mut groups = BTreeMap::new();
    groups.insert("scene:default".to_string(), ());
    for node in &structure.nodes {
        if matches!(node.ui_role.as_str(), "slot" | "section" | "content") {
            groups.insert(slot_group_id_for_node(node), ());
        }
    }
    groups.into_keys().collect()
}

fn material_state_slug(state: &MaterialState) -> &'static str {
    match state {
        MaterialState::Missing => "missing",
        MaterialState::Warming => "warming",
        MaterialState::Ready => "ready",
        MaterialState::Stale => "stale",
        MaterialState::Failed => "failed",
    }
}

fn delivery_class_for_metric(
    manifest: Option<&crate::mrg::client_bootstrap::ClientBootstrapManifest>,
    metric_id: &str,
) -> String {
    manifest
        .and_then(|doc| {
            doc.metrics
                .iter()
                .find(|metric| metric.id == metric_id)
                .map(|metric| match metric.contract.shape {
                    MetricShape::Dataframe => "dataframe_page1".to_string(),
                    MetricShape::Scalar => "metric_scalar".to_string(),
                    _ => "metric_scalar".to_string(),
                })
        })
        .unwrap_or_else(|| "metric_scalar".to_string())
}

fn slot_mount_json(
    slot: &crate::mrg::registry::MrgSlotRecord,
    data_mode: &str,
    manifest: Option<&crate::mrg::client_bootstrap::ClientBootstrapManifest>,
) -> Value {
    let metric_id = slot
        .slot_id
        .node
        .key
        .split("::")
        .nth(1)
        .unwrap_or(slot.owner_resource_id.as_str())
        .to_string();
    json!({
        "metric_id": metric_id,
        "slot_key": format!("{}::{}", slot.slot_id.node.key, slot.slot_id.scope_key),
        "owner_resource_id": slot.owner_resource_id,
        "payload_ref": slot.payload_ref,
        "state": material_state_slug(&slot.state),
        "data_mode": data_mode,
        "client_eligible": slot.client_eligible,
        "delivery_class": delivery_class_for_metric(manifest, metric_id.as_str()),
    })
}

fn scene_mounts(
    workspace_root: Option<&Path>,
    app_id: &str,
    scene_id: &str,
    data_mode: &str,
) -> Vec<Value> {
    let Some(workspace_root) = workspace_root else {
        return Vec::new();
    };
    let manifest =
        crate::mrg::client_bootstrap::read_client_bootstrap(workspace_root, app_id, scene_id);
    let registry = MrgRegistryWriter::load(workspace_root, app_id);
    registry
        .slots
        .iter()
        .filter(|slot| slot.slot_id.scope_key == scene_id)
        .map(|slot| slot_mount_json(slot, data_mode, manifest.as_ref()))
        .collect()
}

pub fn build_eval_slot_group_document(
    compiled: &CompiledApp,
    structure: &crate::view_artifact::StructureFullDocument,
    slot_group_id: &str,
    data_mode: DataMode,
    workspace_root: Option<&Path>,
) -> EvalSlotGroupDocument {
    let mode_slug = data_mode.slug();
    let scene_id = compiled
        .active_scene
        .clone()
        .unwrap_or_else(|| structure.scene_id.clone());
    let scene_mounts = scene_mounts(workspace_root, compiled.app_id.as_str(), scene_id.as_str(), mode_slug);
    let mut slots = BTreeMap::new();
    for node in &structure.nodes {
        if slot_group_id_for_node(node) != slot_group_id {
            continue;
        }
        let scope_key = if node.preview_scope.trim().is_empty() {
            "scene:default".to_string()
        } else {
            node.preview_scope.clone()
        };
        let mut entry = json!({
            "node_id": node.node_id,
            "ui_role": node.ui_role,
            "label": node.label,
            "content_kind": node.content_kind,
            "panel_id": node.panel_id,
            "use_keys": node.use_keys,
        });
        if let Some(obj) = entry.as_object_mut() {
            if slot_group_id == "scene:default" {
                obj.insert("mounts".to_string(), Value::Array(scene_mounts.clone()));
            } else if !scene_mounts.is_empty() {
                obj.insert("mounts".to_string(), Value::Array(scene_mounts.clone()));
            } else {
                obj.insert("mounts".to_string(), Value::Array(Vec::new()));
            }
        }
        slots.insert(scope_key, entry);
    }
    if slots.is_empty() && slot_group_id == "scene:default" {
        slots.insert(
            "scene:default".to_string(),
            json!({
                "scene_id": scene_id,
                "app_id": compiled.app_id,
                "mounts": scene_mounts,
            }),
        );
    }
    EvalSlotGroupDocument {
        schema_version: EVAL_SLOT_GROUP_SCHEMA.to_string(),
        slot_group_id: slot_group_id.to_string(),
        data_mode: mode_slug.to_string(),
        slots,
        bootstrap_seed: if slot_group_id == "scene:default" {
            workspace_root.and_then(|root| {
                crate::mrg::client_bootstrap::client_bootstrap_eval_seed_json(
                    root,
                    compiled.app_id.as_str(),
                    scene_id.as_str(),
                )
            })
        } else {
            None
        },
    }
}

pub fn persist_eval_slot_group(app_root: &Path, document: &EvalSlotGroupDocument) -> Result<PayloadRef> {
    let bytes = serde_json::to_vec(document)?;
    let put = put_if_absent(app_root, EVAL_SLOT_GROUP_KIND, &bytes)?;
    Ok(PayloadRef::new(
        EVAL_SLOT_GROUP_KIND,
        put.content_hash,
        EVAL_SLOT_GROUP_SCHEMA,
    ))
}

pub fn ensure_eval_slot_group_cached(
    workspace_root: &Path,
    compiled: &CompiledApp,
    semantic_core: &SemanticCacheCore,
    slot_group_id: &str,
    data_mode: DataMode,
    layout_policy_revision: &str,
) -> Result<(EvalSlotGroupDocument, PayloadRef, bool)> {
    let cache_key = eval_slot_group_cache_key(
        semantic_core,
        slot_group_id,
        data_mode.slug(),
        "default",
    );
    if let Some(bytes) = take_layer(cache_key.as_str()) {
        let doc: EvalSlotGroupDocument = serde_json::from_slice(bytes.as_slice())?;
        let content_hash = layer_entry_meta(cache_key.as_str())
            .map(|(_, hash)| hash)
            .filter(|hash| !hash.is_empty())
            .unwrap_or_else(|| "cached".to_string());
        let pref = PayloadRef::new(EVAL_SLOT_GROUP_KIND, content_hash.as_str(), EVAL_SLOT_GROUP_SCHEMA);
        return Ok((doc, pref, true));
    }
    let structure = build_structure_full_document(compiled, layout_policy_revision);
    let document = build_eval_slot_group_document(
        compiled,
        &structure,
        slot_group_id,
        data_mode,
        Some(workspace_root),
    );
    let app_root = mei_lang_kernel::resolve_app_root(workspace_root, compiled.app_id.as_str());
    let pref = persist_eval_slot_group(app_root.as_path(), &document)?;
    let bytes = serde_json::to_vec(&document)?;
    store_layer(
        cache_key,
        EVAL_SLOT_GROUP_KIND,
        pref.content_hash.as_str(),
        bytes.as_slice(),
    );
    Ok((document, pref, false))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view_artifact::StructureFullNode;

    #[test]
    fn slot_groups_include_scene_default() {
        let structure = crate::view_artifact::StructureFullDocument {
            schema_version: "structure-full-v1".to_string(),
            app_id: "demo".to_string(),
            scene_id: "home".to_string(),
            semantic_revision: "rev".to_string(),
            scene_roots: vec![],
            nodes: vec![StructureFullNode {
                node_id: "n1".to_string(),
                ui_role: "slot".to_string(),
                preview_scope: "panel:left".to_string(),
                label: "left".to_string(),
                parent_id: None,
                children: vec![],
                plane: None,
                content_kind: None,
                panel_id: None,
                use_keys: vec![],
                frame_viewport: None,
            }],
            frame_viewport: None,
        };
        let groups = collect_slot_groups(&structure);
        assert!(groups.iter().any(|group| group == "scene:default"));
        assert!(groups.iter().any(|group| group == "scope:panel:left"));
    }
}
