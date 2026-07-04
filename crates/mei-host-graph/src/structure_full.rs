//! Produce `structure.full` artifact from assembled `CompiledApp`.

use std::path::Path;

use anyhow::Result;
use mei_lang_kernel::{CompiledApp, UiScopeRole};

use crate::content_store::{put_if_absent, STRUCTURE_FULL_KIND as CONTENT_STRUCTURE_FULL_KIND};
use crate::types::PayloadRef;
use crate::view_artifact::{
    structure_full_cache_key, StructureFullDocument, StructureFullNode, STRUCTURE_FULL_SCHEMA,
};

pub fn build_structure_full_document(
    compiled: &CompiledApp,
    semantic_revision: &str,
) -> StructureFullDocument {
    let mut nodes = Vec::new();
    for node in compiled.ui_layout_index.nodes.values() {
        nodes.push(StructureFullNode {
            node_id: node.node_id.clone(),
            ui_role: node.role.slug().to_string(),
            preview_scope: node.preview_scope.clone(),
            label: node.label.clone(),
            parent_id: node.parent_id.clone(),
            children: node.children.clone(),
            plane: node.plane.clone(),
            content_kind: node.content_kind.clone(),
        });
    }
    nodes.sort_by(|a, b| a.node_id.cmp(&b.node_id));
    StructureFullDocument {
        schema_version: STRUCTURE_FULL_SCHEMA.to_string(),
        app_id: compiled.app_id.clone(),
        scene_id: compiled
            .active_scene
            .clone()
            .unwrap_or_else(|| "home".to_string()),
        semantic_revision: semantic_revision.to_string(),
        scene_roots: compiled.ui_layout_index.scene_roots.clone(),
        nodes,
    }
}

pub fn persist_structure_full(
    app_root: &Path,
    document: &StructureFullDocument,
) -> Result<PayloadRef> {
    let bytes = serde_json::to_vec(document)?;
    let put = put_if_absent(app_root, CONTENT_STRUCTURE_FULL_KIND, &bytes)?;
    Ok(PayloadRef::new(
        CONTENT_STRUCTURE_FULL_KIND,
        put.content_hash,
        STRUCTURE_FULL_SCHEMA,
    ))
}

pub fn structure_full_from_compiled(
    workspace_root: &Path,
    compiled: &CompiledApp,
    semantic_core: &crate::semantic_cache::SemanticCacheCore,
    layout_policy_revision: &str,
) -> Result<(StructureFullDocument, PayloadRef, String)> {
    let cache_key = structure_full_cache_key(semantic_core, layout_policy_revision);
    let document = build_structure_full_document(compiled, cache_key.as_str());
    let app_root = mei_lang_kernel::resolve_app_root(workspace_root, compiled.app_id.as_str());
    let pref = persist_structure_full(app_root.as_path(), &document)?;
    Ok((document, pref, cache_key))
}

pub fn build_structure_index_document(
    document: &StructureFullDocument,
) -> serde_json::Value {
    let mut by_scope = std::collections::BTreeMap::new();
    for node in &document.nodes {
        by_scope.insert(node.preview_scope.clone(), node.node_id.clone());
    }
    serde_json::json!({
        "schema_version": crate::view_artifact::STRUCTURE_INDEX_KIND,
        "by_preview_scope": by_scope,
    })
}

pub fn ui_role_depth_rank(role: &str) -> u8 {
    match role.trim().to_ascii_lowercase().as_str() {
        "plane" => 0,
        "region" => 1,
        "section" => 2,
        "slot" | "content" => 3,
        _ => 99,
    }
}

pub fn nodes_within_projection<'a>(
    document: &'a StructureFullDocument,
    max_role: Option<&str>,
) -> Vec<&'a StructureFullNode> {
    let max_depth = max_role.map(ui_role_depth_rank).unwrap_or(99);
    document
        .nodes
        .iter()
        .filter(|node| ui_role_depth_rank(node.ui_role.as_str()) <= max_depth)
        .collect()
}

pub fn slot_group_id_for_node(node: &StructureFullNode) -> String {
    if node.ui_role == UiScopeRole::Content.slug() {
        return format!("content:{}", node.preview_scope);
    }
    if node.ui_role == UiScopeRole::Slot.slug() || node.ui_role == UiScopeRole::Section.slug() {
        return format!("scope:{}", node.preview_scope);
    }
    format!("node:{}", node.node_id)
}
