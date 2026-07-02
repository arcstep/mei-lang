use crate::model::{BuildNodeId, BuildNodeKind, CompiledApp, UiScopeRole};

fn ssr_full_scene_for_structure_inspect(node: &BuildNodeId) -> bool {
    matches!(
        node.kind,
        BuildNodeKind::UiScope
            | BuildNodeKind::ScenePanel
            | BuildNodeKind::SceneBlock
            | BuildNodeKind::Scene
            | BuildNodeKind::Route
    )
}
use crate::{build_preview_panel_scope, build_preview_ui_scope};

/// Resolved UI scope annotation for a content block in preview.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiScopeBlockAnnotation {
    pub node_id: String,
    pub preview_scope: String,
    pub role: String,
}

/// Lookup UI scope annotation for a preview panel path (scene-relative).
pub fn ui_scope_annotation_for_preview_path(
    compiled: &CompiledApp,
    scene_id: &str,
    panel_path: &str,
) -> Option<(String, String)> {
    let normalized = panel_path.trim().trim_matches('/');
    if normalized.is_empty() {
        return None;
    }
    let mut best: Option<(usize, String, String)> = None;
    for node in compiled.ui_layout_index.nodes.values() {
        if node
            .scene_id
            .as_deref()
            .is_some_and(|value| value != scene_id)
        {
            continue;
        }
        let scope = node.preview_scope.trim();
        if scope.is_empty() {
            continue;
        }
        if scope == normalized || normalized.ends_with(&format!("/{scope}")) || normalized == scope {
            let len = scope.len();
            if best.as_ref().is_none_or(|(best_len, _, _)| len >= *best_len) {
                best = Some((len, scope.to_string(), node.role.slug().to_string()));
            }
        }
    }
    best.map(|(_, scope, role)| (scope, role))
}

/// Resolve preview dim scope for a build node (panel or ui-scope).
pub fn resolve_build_preview_scope(compiled: &CompiledApp, node: &BuildNodeId) -> Option<String> {
    build_preview_ui_scope(compiled, node).or_else(|| build_preview_panel_scope(compiled, node))
}

/// SSR preview scope: structure-tree ui-scope nodes render the full scene.
pub fn resolve_build_preview_scope_for_ssr(
    compiled: &CompiledApp,
    node: &BuildNodeId,
) -> Option<String> {
    if ssr_full_scene_for_structure_inspect(node) {
        return None;
    }
    resolve_build_preview_scope(compiled, node)
}

/// Lookup ui-scope annotation for a block inside a panel (content-level highlight).
pub fn ui_scope_for_block(
    compiled: &CompiledApp,
    scene_id: &str,
    panel_path: &str,
    block_id: &str,
) -> Option<UiScopeBlockAnnotation> {
    let panel_prefix = panel_path.trim().trim_matches('/');
    let block_key = block_id.trim();
    if block_key.is_empty() {
        return None;
    }
    let mut best: Option<(usize, UiScopeBlockAnnotation)> = None;
    for node in compiled.ui_layout_index.nodes.values() {
        if node.role != UiScopeRole::Content {
            continue;
        }
        if node
            .scene_id
            .as_deref()
            .is_some_and(|value| value != scene_id)
        {
            continue;
        }
        let scope = node.preview_scope.trim();
        if scope.is_empty() {
            continue;
        }
        let matches_block = scope.ends_with(&format!("/{block_key}")) || scope == block_key;
        if !matches_block {
            continue;
        }
        if !panel_prefix.is_empty()
            && scope != panel_prefix
            && !scope.starts_with(&format!("{panel_prefix}/"))
        {
            continue;
        }
        let len = scope.len();
        if best.as_ref().is_none_or(|(best_len, _)| len >= *best_len) {
            best = Some((
                len,
                UiScopeBlockAnnotation {
                    node_id: node.node_id.clone(),
                    preview_scope: scope.to_string(),
                    role: node.role.slug().to_string(),
                },
            ));
        }
    }
    best.map(|(_, annotation)| annotation)
}

/// Technical micro/slot/budget metadata for section/content inspector.
pub fn format_ui_scope_technical_detail(compiled: &CompiledApp, node: &BuildNodeId) -> Option<String> {
    let entry = compiled.ui_layout_index.lookup(node)?;
    if !matches!(
        entry.role,
        UiScopeRole::Section | UiScopeRole::Content | UiScopeRole::Region
    ) {
        return None;
    }
    let subtree = technical_nodes_under(entry, compiled);
    if subtree.is_empty() {
        return None;
    }
    let mut md = String::from("## 技术配置\n\n");
    for item in subtree {
        let key = item.role.agent_key();
        md.push_str(&format!("- {key}: `{}`\n", item.label));
        if let Some(budget) = &item.budget {
            if let Some(gap) = budget.gap.as_deref() {
                md.push_str(&format!("  - gap: `{gap}`\n"));
            }
            if let Some(padding) = budget.padding.as_deref() {
                md.push_str(&format!("  - padding: `{padding}`\n"));
            }
            for (width_key, width) in &budget.widths {
                md.push_str(&format!("  - {width_key}: `{width}`\n"));
            }
        }
        if let Some(kind) = item.content_kind.as_deref().filter(|v| !v.is_empty()) {
            md.push_str(&format!("  - kind: `{kind}`\n"));
        }
    }
    Some(md)
}

fn technical_nodes_under<'a>(
    entry: &'a crate::model::UiScopeNode,
    compiled: &'a CompiledApp,
) -> Vec<&'a crate::model::UiScopeNode> {
    let mut collected = Vec::new();
    collect_technical_nodes(entry, compiled, &mut collected);
    collected
}

fn collect_technical_nodes<'a>(
    entry: &'a crate::model::UiScopeNode,
    compiled: &'a CompiledApp,
    out: &mut Vec<&'a crate::model::UiScopeNode>,
) {
    if matches!(
        entry.role,
        UiScopeRole::MicroLayout | UiScopeRole::Slot | UiScopeRole::Budget
    ) {
        out.push(entry);
    }
    for child_id in &entry.children {
        if let Some(child) = compiled.ui_layout_index.nodes.get(child_id) {
            collect_technical_nodes(child, compiled, out);
        }
    }
}

/// Format Agent-facing adjustment scope markdown for a ui-scope node.
pub fn format_ui_scope_agent_context(compiled: &CompiledApp, node: &BuildNodeId) -> Option<String> {
    let entry = compiled.ui_layout_index.lookup(node)?;
    let chain = compiled.ui_layout_index.ancestor_chain(&entry.node_id);
    let mut md = String::from("## 调整范围\n\n");
    md.push_str(&format!("- app: `{}`\n", compiled.app_id));
    for item in chain {
        let key = item.role.agent_key();
        md.push_str(&format!("- {key}: `{}`\n", item.label));
        if item.role == UiScopeRole::Plane {
            if let Some(plane) = item.plane.as_deref() {
                md.push_str(&format!("- plane_tier: `{plane}`\n"));
            }
        }
    }
    if let Some(budget) = &entry.budget {
        md.push_str("\n## 当前 budget\n\n");
        if let Some(gap) = budget.gap.as_deref() {
            md.push_str(&format!("- gap: `{gap}`\n"));
        }
        if let Some(padding) = budget.padding.as_deref() {
            md.push_str(&format!("- padding: `{padding}`\n"));
        }
        if let Some(height) = budget.card_height {
            md.push_str(&format!("- card_height: `{height}`\n"));
        }
        for (key, value) in &budget.widths {
            md.push_str(&format!("- {key}: `{value}`\n"));
        }
    }
    if !entry.source_anchors.is_empty() {
        md.push_str("\n## 建议修改文件\n\n");
        for anchor in &entry.source_anchors {
            if !anchor.file.is_empty() {
                md.push_str(&format!("- `{}`\n", anchor.file));
            }
        }
    }
    Some(md)
}
