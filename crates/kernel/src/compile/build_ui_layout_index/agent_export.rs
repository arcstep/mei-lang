use crate::model::{BuildNodeId, CompiledApp, UiScopeRole};
use crate::{build_preview_panel_scope, build_preview_ui_scope};

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
