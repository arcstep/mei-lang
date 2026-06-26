use super::rebuild::snapshot_to_root;
use super::{build_view_reachability_stale, ensure_board_and_template_roots, rebuild_reachability_tree_from_compiled, root_to_snapshot, source_root_from_app};



use crate::compile::reachability_tree::{
        ReachabilityTreeNode,
        ReachabilityTreeRoot,
    };
use crate::model::{
    BuildNodeId, BuildNodeKind, CompiledApp, Diagnostic,
    ReachabilityTreeNodeSnapshot, ReachabilityTreeRootSnapshot,
    Severity,
};

pub fn merge_build_view_tree_roots(
    experience_snapshot: Vec<ReachabilityTreeRootSnapshot>,
    board_root: ReachabilityTreeRoot,
    template_root: ReachabilityTreeRoot,
    template_files_root: ReachabilityTreeRoot,
) -> Vec<ReachabilityTreeRootSnapshot> {
    let mut merged = experience_snapshot;
    if !board_root.children.is_empty() {
        if let Some(scenes) = merged.first_mut() {
            let _ = scenes;
        }
        merged.insert(1, root_to_snapshot(board_root));
    }
    if !template_root.children.is_empty() {
        let insert_at = if merged.len() > 1 && merged[1].group == "boards" {
            2
        } else {
            1
        };
        merged.insert(insert_at, root_to_snapshot(template_root));
    }
    if !template_files_root.children.is_empty() {
        let insert_at = merged
            .iter()
            .position(|root| root.group == "templates")
            .map(|idx| idx + 1)
            .unwrap_or_else(|| {
                if merged.len() > 1 && merged[1].group == "boards" {
                    2
                } else {
                    1
                }
            });
        merged.insert(insert_at, root_to_snapshot(template_files_root));
    }
    merged
}

pub fn reachability_roots_from_compiled(compiled: &CompiledApp) -> Vec<ReachabilityTreeRoot> {
    let mut roots = if build_view_reachability_stale(compiled) {
        rebuild_reachability_tree_from_compiled(compiled)
    } else {
        let mut roots = compiled
            .build_experience_index
            .reachability_snapshot
            .iter()
            .map(snapshot_to_root)
            .collect();
        ensure_board_and_template_roots(&mut roots, compiled);
        roots
    };
    ensure_mcg_root(&mut roots, compiled);
    enrich_reachability_tree_compile_coords(&mut roots, compiled);
    normalize_reachability_tree_roots(&mut roots);
    strip_stock_facet_roots_for_business_app(&mut roots, compiled);
    roots
}

fn strip_stock_facet_roots_for_business_app(
    roots: &mut Vec<ReachabilityTreeRoot>,
    compiled: &CompiledApp,
) {
    if crate::mei_config::is_stock_catalog_app(compiled.app_id.as_str()) {
        return;
    }
    roots.retain(|root| root.group != "templates" && root.group != "template_files");
}

fn ensure_mcg_root(roots: &mut Vec<ReachabilityTreeRoot>, compiled: &CompiledApp) {
    if roots.iter().any(|root| root.group == "mcg") {
        return;
    }
    let source_root = source_root_from_app(compiled);
    let mcg_root = crate::compile::build_mcg_index::build_mcg_tree_root(
        source_root.as_path(),
        compiled.app_id.as_str(),
    );
    if mcg_root.children.is_empty() {
        return;
    }
    roots.push(mcg_root);
}

fn normalize_reachability_tree_roots(roots: &mut [ReachabilityTreeRoot]) {
    for root in roots {
        if root.group == "templates" {
            root.label = "Components".to_string();
        }
        if root.group == "template_files" {
            root.label = "Templates".to_string();
        }
    }
}

pub fn enrich_reachability_tree_compile_coords(
    roots: &mut [ReachabilityTreeRoot],
    compiled: &CompiledApp,
) {
    for root in roots {
        for child in &mut root.children {
            enrich_node_compile_coords(child, compiled);
        }
    }
}

fn enrich_node_compile_coords(node: &mut ReachabilityTreeNode, compiled: &CompiledApp) {
    if let Some(parsed) = BuildNodeId::parse(&node.node_id) {
        if let Some(coord) = crate::compile::build_experience::compile_coordinate_for_node(&parsed, compiled)
        {
            node.compile_scene = coord.scene_id.unwrap_or_default();
            node.compile_target = coord.preview_target;
        } else if matches!(
            parsed.kind,
            BuildNodeKind::Component | BuildNodeKind::Template
        ) {
            mark_preview_unavailable(node);
        }
    }
    for child in &mut node.children {
        enrich_node_compile_coords(child, compiled);
    }
}

fn mark_preview_unavailable(node: &mut ReachabilityTreeNode) {
    if !node
        .badges
        .iter()
        .any(|badge| badge == "preview:unavailable")
    {
        node.badges.push("preview:unavailable".to_string());
    }
}

/// At compile time, mark component/template nodes without workspace preview targets.
pub fn annotate_stock_preview_availability(
    snapshot: &mut [ReachabilityTreeRootSnapshot],
    compiled: &CompiledApp,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !crate::mei_config::is_stock_catalog_app(compiled.app_id.as_str()) {
        return;
    }
    for root in snapshot.iter_mut() {
        for child in &mut root.children {
            annotate_snapshot_preview_availability(child, compiled, diagnostics);
        }
    }
}

fn annotate_snapshot_preview_availability(
    node: &mut ReachabilityTreeNodeSnapshot,
    compiled: &CompiledApp,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(parsed) = BuildNodeId::parse(&node.node_id) {
        if matches!(
            parsed.kind,
            BuildNodeKind::Component | BuildNodeKind::Template
        ) && crate::compile::build_experience::compile_coordinate_for_node(&parsed, compiled).is_none()
        {
            if !node
                .badges
                .iter()
                .any(|badge| badge == "preview:unavailable")
            {
                node.badges.push("preview:unavailable".to_string());
            }
            diagnostics.push(Diagnostic {
                severity: Severity::Warning,
                code: "previewUnavailable".to_string(),
                message: format!(
                    "Build preview unavailable for `{}`: missing workspace stock example or template scene",
                    node.label
                ),
                source_path: None,
            });
        }
    }
    for child in &mut node.children {
        annotate_snapshot_preview_availability(child, compiled, diagnostics);
    }
}

