//! `eval.slot_group[*]` artifact helpers.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;
use mei_lang_kernel::{CompiledApp, DataMode};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::content_store::{put_if_absent, EVAL_SLOT_GROUP_KIND};
use crate::layer_store::{store_layer, take_layer};
use crate::semantic_cache::SemanticCacheCore;
use crate::structure_full::slot_group_id_for_node;
use crate::types::PayloadRef;
use crate::structure_full::build_structure_full_document;
use crate::view_artifact::eval_slot_group_cache_key;

pub const EVAL_SLOT_GROUP_SCHEMA: &str = "eval-slot-group-v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvalSlotGroupDocument {
    pub schema_version: String,
    pub slot_group_id: String,
    pub data_mode: String,
    pub slots: BTreeMap<String, Value>,
}

pub fn collect_slot_groups(structure: &crate::view_artifact::StructureFullDocument) -> Vec<String> {
    let mut groups = BTreeMap::new();
    for node in &structure.nodes {
        if matches!(node.ui_role.as_str(), "slot" | "section" | "content") {
            groups.insert(slot_group_id_for_node(node), ());
        }
    }
    if groups.is_empty() {
        return vec!["scene:default".to_string()];
    }
    groups.into_keys().collect()
}

pub fn build_eval_slot_group_document(
    compiled: &CompiledApp,
    structure: &crate::view_artifact::StructureFullDocument,
    slot_group_id: &str,
    data_mode: DataMode,
) -> EvalSlotGroupDocument {
    let mut slots = BTreeMap::new();
    for node in &structure.nodes {
        if slot_group_id_for_node(node) != slot_group_id {
            continue;
        }
        slots.insert(
            node.preview_scope.clone(),
            json!({
                "node_id": node.node_id,
                "ui_role": node.ui_role,
                "label": node.label,
                "content_kind": node.content_kind,
            }),
        );
    }
    if slots.is_empty() {
        slots.insert(
            "scene:default".to_string(),
            json!({
                "scene_id": compiled.active_scene,
                "app_id": compiled.app_id,
            }),
        );
    }
    EvalSlotGroupDocument {
        schema_version: EVAL_SLOT_GROUP_SCHEMA.to_string(),
        slot_group_id: slot_group_id.to_string(),
        data_mode: data_mode.slug().to_string(),
        slots,
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
        let pref = PayloadRef::new(EVAL_SLOT_GROUP_KIND, "cached", EVAL_SLOT_GROUP_SCHEMA);
        return Ok((doc, pref, true));
    }
    let structure = build_structure_full_document(compiled, layout_policy_revision);
    let document = build_eval_slot_group_document(compiled, &structure, slot_group_id, data_mode);
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
    fn slot_groups_follow_preview_scope() {
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
            }],
        };
        let groups = collect_slot_groups(&structure);
        assert_eq!(groups, vec!["scope:panel:left".to_string()]);
    }
}
