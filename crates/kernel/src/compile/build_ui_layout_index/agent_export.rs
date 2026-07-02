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

/// Resolved UI scope annotation for a preview panel path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiScopePanelAnnotation {
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
    ui_scope_annotation_for_preview_panel(compiled, scene_id, panel_path, None)
        .map(|annotation| (annotation.preview_scope, annotation.role))
}

/// Fuzzy-match walker preview_scope to rendered panel_path (apps differ in slot vs panel ids).
pub fn ui_scope_annotation_for_preview_panel(
    compiled: &CompiledApp,
    scene_id: &str,
    panel_path: &str,
    panel_area: Option<&str>,
) -> Option<UiScopePanelAnnotation> {
    let normalized = panel_path.trim().trim_matches('/');
    if normalized.is_empty() {
        return None;
    }
    let mut best: Option<(usize, UiScopePanelAnnotation)> = None;
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
        let Some(score) = scope_match_score(normalized, scope, panel_area, node.role) else {
            continue;
        };
        if best.as_ref().is_none_or(|(best_score, _)| score > *best_score) {
            best = Some((
                score,
                UiScopePanelAnnotation {
                    node_id: node.node_id.clone(),
                    preview_scope: scope.to_string(),
                    role: node.role.slug().to_string(),
                },
            ));
        }
    }
    best.map(|(_, annotation)| annotation)
}

fn scope_match_score(
    panel_path: &str,
    preview_scope: &str,
    panel_area: Option<&str>,
    role: UiScopeRole,
) -> Option<usize> {
    if panel_path == preview_scope {
        return Some(10_000 + preview_scope.len());
    }
    if panel_path.ends_with(&format!("/{preview_scope}")) {
        return Some(9_000 + preview_scope.len());
    }
    if let Some(leaf) = preview_scope.rsplit('/').next().filter(|v| !v.is_empty()) {
        if panel_path.ends_with(leaf) || panel_path.contains(&format!("/{leaf}")) {
            let base = match role {
                UiScopeRole::Content => 6_000,
                UiScopeRole::Section => 5_000,
                _ => 4_000,
            };
            return Some(base + preview_scope.len());
        }
        if let Some((stem, suffix)) = leaf.rsplit_once('~') {
            if !stem.is_empty() && suffix.chars().all(|ch| ch.is_ascii_digit()) {
                if panel_path.contains(stem) {
                    return Some(5_500 + preview_scope.len());
                }
            }
        }
    }
    if role == UiScopeRole::Section {
        if let Some(area) = panel_area.filter(|value| is_section_area(value)) {
            if preview_scope.ends_with(&format!("/{area}")) {
                let region = preview_scope.strip_suffix(&format!("/{area}"))?;
                if panel_path.starts_with(region)
                    || panel_path.contains(&format!("{region}/"))
                    || panel_path.starts_with(&format!("{region}/"))
                {
                    return Some(7_000 + preview_scope.len());
                }
            }
        }
    }
    if panel_path == preview_scope
        || normalized_ends_with_scope(panel_path, preview_scope)
    {
        return Some(preview_scope.len());
    }
    None
}

fn is_section_area(value: &str) -> bool {
    let area = value.trim();
    !area.is_empty() && area != "auto" && area != "body"
}

fn normalized_ends_with_scope(panel_path: &str, preview_scope: &str) -> bool {
    panel_path.ends_with(&format!("/{preview_scope}"))
        || preview_scope
            .split('/')
            .filter(|segment| !segment.is_empty())
            .all(|segment| panel_path.contains(segment))
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
    let match_keys = block_match_keys(block_key);
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
        let matches_block = match_keys
            .iter()
            .any(|key| scope.ends_with(&format!("/{key}")) || scope == *key);
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

fn block_match_keys(block_key: &str) -> Vec<String> {
    let mut keys = vec![block_key.to_string()];
    if let Some((stem, suffix)) = block_key.rsplit_once('~') {
        if !stem.is_empty() && suffix.chars().all(|ch| ch.is_ascii_digit()) {
            keys.push(stem.to_string());
        }
    }
    keys
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
