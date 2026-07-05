//! Produce `structure.full` artifact from assembled `CompiledApp`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Result;
use mei_lang_kernel::{CompiledApp, UiScopeRole};

use crate::content_store::{put_if_absent, STRUCTURE_FULL_KIND as CONTENT_STRUCTURE_FULL_KIND};
use crate::types::PayloadRef;
use crate::view_artifact::{
    structure_full_cache_key, FrameViewportMeta, StructureFullDocument, StructureFullNode,
    STRUCTURE_FULL_SCHEMA,
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
            panel_id: None,
            use_keys: Vec::new(),
            frame_viewport: None,
        });
    }
    nodes.sort_by(|a, b| a.node_id.cmp(&b.node_id));
    let scene_id = compiled
        .active_scene
        .clone()
        .unwrap_or_else(|| "home".to_string());
    let mut document = StructureFullDocument {
        schema_version: STRUCTURE_FULL_SCHEMA.to_string(),
        app_id: compiled.app_id.clone(),
        scene_id,
        semantic_revision: semantic_revision.to_string(),
        scene_roots: compiled.ui_layout_index.scene_roots.clone(),
        nodes,
        frame_viewport: extract_frame_viewport_meta(compiled),
    };
    enrich_structure_bindings(&mut document);
    document
}

fn extract_frame_viewport_meta(compiled: &CompiledApp) -> Option<FrameViewportMeta> {
    let contract = compiled.scene_contract.as_ref()?;
    let frame = contract.frame.as_ref()?;
    let props = &frame.props;
    let design_width = props
        .get("design_width")
        .or_else(|| props.get("designWidth"))
        .and_then(|v| v.as_u64())
        .and_then(|n| u32::try_from(n).ok());
    let design_height = props
        .get("design_height")
        .or_else(|| props.get("designHeight"))
        .and_then(|v| v.as_u64())
        .and_then(|n| u32::try_from(n).ok());
    let scale_mode = props
        .get("scale_mode")
        .or_else(|| props.get("scaleMode"))
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let overflow_mode = props
        .get("overflow_mode")
        .or_else(|| props.get("overflowMode"))
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let aspect_ratio = props
        .get("aspect_ratio")
        .or_else(|| props.get("aspectRatio"))
        .and_then(|v| v.as_str())
        .map(str::to_string);
    Some(FrameViewportMeta {
        design_width,
        design_height,
        scale_mode,
        overflow_mode,
        aspect_ratio,
        target_file: Some(compiled.active_target_file.clone()),
        scene_id: compiled.active_scene.clone(),
        route_mode: Some("app".to_string()),
    })
}

fn enrich_structure_bindings(document: &mut StructureFullDocument) {
    let node_index: BTreeMap<String, usize> = document
        .nodes
        .iter()
        .enumerate()
        .map(|(idx, node)| (node.node_id.clone(), idx))
        .collect();
    let mut children_by_parent: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for node in &document.nodes {
        if let Some(parent_id) = node.parent_id.as_deref().filter(|v| !v.is_empty()) {
            children_by_parent
                .entry(parent_id.to_string())
                .or_default()
                .push(node.node_id.clone());
        }
    }

    for node in &mut document.nodes {
        if node.ui_role == UiScopeRole::Content.slug() {
            if let Some(kind) = node.content_kind.as_deref().filter(|v| !v.is_empty()) {
                node.use_keys = vec![kind.to_string()];
            }
        }
        if matches!(node.ui_role.as_str(), "slot" | "section")
            && !node.preview_scope.trim().is_empty()
        {
            node.panel_id = Some(node.preview_scope.clone());
        }
        if is_viewport_structure_node(node) {
            node.frame_viewport = document.frame_viewport.clone();
        }
    }

    for idx in 0..document.nodes.len() {
        let node_id = document.nodes[idx].node_id.clone();
        let mut keys = BTreeSet::new();
        collect_descendant_use_keys(&node_id, &children_by_parent, &node_index, document, &mut keys);
        if !keys.is_empty() {
            document.nodes[idx].use_keys = keys.into_iter().collect();
        }
    }
}

fn is_viewport_structure_node(node: &StructureFullNode) -> bool {
    let haystack = format!(
        "{} {} {}",
        node.node_id, node.preview_scope, node.label
    )
    .to_ascii_lowercase();
    haystack.contains("viewport") || haystack.contains("world_viewport")
}

fn collect_descendant_use_keys(
    node_id: &str,
    children_by_parent: &BTreeMap<String, Vec<String>>,
    node_index: &BTreeMap<String, usize>,
    document: &StructureFullDocument,
    out: &mut BTreeSet<String>,
) {
    let Some(children) = children_by_parent.get(node_id) else {
        return;
    };
    for child_id in children {
        let Some(&idx) = node_index.get(child_id) else {
            continue;
        };
        let child = &document.nodes[idx];
        if child.ui_role == UiScopeRole::Content.slug() {
            if let Some(kind) = child.content_kind.as_deref().filter(|v| !v.is_empty()) {
                out.insert(kind.to_string());
            }
            if let Some(keys) = child
                .use_keys
                .iter()
                .filter(|key| !key.trim().is_empty())
                .cloned()
                .next()
            {
                out.insert(keys);
            }
        }
        collect_descendant_use_keys(child_id, children_by_parent, node_index, document, out);
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

#[cfg(test)]
mod tests {
    use super::*;
    use mei_lang_kernel::{
        CompiledApp, UiLayoutIndex, UiScopeNode, UiScopeRole,
    };

    fn sample_compiled_with_content() -> CompiledApp {
        let mut index = UiLayoutIndex::default();
        index.scene_roots = vec!["ui-scope:home/home".to_string()];
        index.nodes.insert(
            "ui-scope:home/home/T0/panel:left".to_string(),
            UiScopeNode {
                node_id: "ui-scope:home/home/T0/panel:left".to_string(),
                role: UiScopeRole::Slot,
                label: "left".to_string(),
                scope_path: vec![],
                plane: Some("T0".to_string()),
                parent_id: Some("ui-scope:home/home/T0".to_string()),
                children: vec!["ui-scope:home/home/T0/panel:left/content:metric".to_string()],
                preview_scope: "home/T0/panel:left".to_string(),
                budget: None,
                source_anchors: vec![],
                content_kind: None,
                scene_id: Some("home".to_string()),
            },
        );
        index.nodes.insert(
            "ui-scope:home/home/T0/panel:left/content:metric".to_string(),
            UiScopeNode {
                node_id: "ui-scope:home/home/T0/panel:left/content:metric".to_string(),
                role: UiScopeRole::Content,
                label: "metric".to_string(),
                scope_path: vec![],
                plane: Some("T0".to_string()),
                parent_id: Some("ui-scope:home/home/T0/panel:left".to_string()),
                children: vec![],
                preview_scope: "home/T0/panel:left/metric-card".to_string(),
                budget: None,
                source_anchors: vec![],
                content_kind: Some("metric-card".to_string()),
                scene_id: Some("home".to_string()),
            },
        );
        CompiledApp {
            app_id: "demo".to_string(),
            title: "demo".to_string(),
            app_root: ".".to_string(),
            scene_routes: vec![],
            active_scene: Some("home".to_string()),
            active_target_file: "src/scene/home/assembly.mei".to_string(),
            file_tree: vec![],
            scene_contract: None,
            scene_local_nav_by_target: Default::default(),
            scene_bindings_by_id: Default::default(),
            scene_examples_by_id: Default::default(),
            scene_projection_assembly_by_id: Default::default(),
            resources: vec![],
            world_metrics: Default::default(),
            world_semantic_by_file: Default::default(),
            component_assets: vec![],
            diagnostics: vec![],
            build_experience_index: Default::default(),
            build_board_index: Default::default(),
            build_template_index: Default::default(),
            ui_layout_index: index,
        }
    }

    #[test]
    fn structure_bindings_include_panel_and_use_keys() {
        let doc = build_structure_full_document(&sample_compiled_with_content(), "rev");
        let slot = doc
            .nodes
            .iter()
            .find(|node| node.preview_scope == "home/T0/panel:left")
            .expect("slot node");
        assert_eq!(slot.panel_id.as_deref(), Some("home/T0/panel:left"));
        assert!(slot.use_keys.iter().any(|key| key == "metric-card"));
    }
}
