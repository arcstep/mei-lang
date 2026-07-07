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
        if !matches!(
            node.role,
            UiScopeRole::Content | UiScopeRole::Section | UiScopeRole::Region | UiScopeRole::Slot
        ) {
            continue;
        }
        if node
            .scene_id
            .as_deref()
            .is_some_and(|value| value != scene_id)
        {
            continue;
        }
        let scope = normalize_preview_scope_segments(node.preview_scope.trim());
        if scope.is_empty() {
            continue;
        }
        let Some(score) = scope_match_score(normalized, scope.as_str(), panel_area, node.role) else {
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
    let preview_scope = normalize_preview_scope_segments(preview_scope);
    if preview_scope.is_empty() {
        return None;
    }
    if panel_path == preview_scope {
        return Some(10_000 + preview_scope.len());
    }
    if panel_path.ends_with(&format!("/{preview_scope}")) {
        return Some(9_000 + preview_scope.len());
    }

    match role {
        UiScopeRole::Content => {
            content_path_align_score(panel_path, preview_scope.as_str(), panel_area)
        }
        UiScopeRole::Slot => {
            slot_path_align_score(panel_path, preview_scope.as_str(), panel_area)
        }
        UiScopeRole::Section => {
            if let Some(area) = panel_area.filter(|value| is_section_area(value)) {
                if preview_scope.ends_with(&format!("/{area}")) {
                    let region = preview_scope.strip_suffix(&format!("/{area}"))?;
                    if panel_path.starts_with(region)
                        || panel_path.starts_with(&format!("{region}/"))
                    {
                        return Some(7_500 + preview_scope.len());
                    }
                }
            }
            if path_segments_align(panel_path, preview_scope.as_str(), true) {
                let panel_depth = panel_path.split('/').filter(|s| !s.is_empty()).count();
                let scope_depth = preview_scope.split('/').filter(|s| !s.is_empty()).count();
                if panel_depth.saturating_sub(scope_depth) > 2 {
                    return None;
                }
                return Some(6_500 + preview_scope.len());
            }
            None
        }
        _ => {
            if path_segments_align(panel_path, preview_scope.as_str(), false) {
                return Some(4_000 + preview_scope.len());
            }
            None
        }
    }
}

fn normalize_preview_scope_segments(scope: &str) -> String {
    let segments: Vec<&str> = scope.split('/').filter(|segment| !segment.is_empty()).collect();
    let mut out: Vec<&str> = Vec::new();
    for segment in segments {
        if out.last().copied() != Some(segment) {
            out.push(segment);
        }
    }
    out.join("/")
}

fn path_segment_matches(panel_segment: &str, scope_segment: &str) -> bool {
    if panel_segment == scope_segment {
        return true;
    }
    if let Some((stem, suffix)) = scope_segment.rsplit_once('~') {
        if !stem.is_empty() && suffix.chars().all(|ch| ch.is_ascii_digit()) {
            return panel_segment == stem || panel_segment.starts_with(&format!("{stem}~"));
        }
    }
    if let Some((stem, suffix)) = panel_segment.rsplit_once('~') {
        if !stem.is_empty() && suffix.chars().all(|ch| ch.is_ascii_digit()) {
            return stem == scope_segment;
        }
    }
    false
}

fn path_segments_align(panel_path: &str, preview_scope: &str, require_prefix: bool) -> bool {
    let panel_segments: Vec<&str> = panel_path.split('/').filter(|s| !s.is_empty()).collect();
    let scope_segments: Vec<&str> = preview_scope.split('/').filter(|s| !s.is_empty()).collect();
    if scope_segments.is_empty() || panel_segments.is_empty() {
        return false;
    }
    if require_prefix
        && !panel_segments
            .iter()
            .take(2)
            .any(|segment| path_segment_matches(segment, scope_segments[0]))
    {
        return false;
    }
    let mut panel_index = 0usize;
    for scope_segment in &scope_segments {
        let mut found = false;
        while panel_index < panel_segments.len() {
            if path_segment_matches(panel_segments[panel_index], scope_segment) {
                found = true;
                panel_index += 1;
                break;
            }
            panel_index += 1;
        }
        if !found {
            return false;
        }
    }
    true
}

fn is_ambiguous_content_leaf(leaf: &str) -> bool {
    matches!(leaf, "metric_card" | "panel" | "body")
        || (leaf.len() < 10 && !leaf.contains('_'))
}

fn content_path_align_score(
    panel_path: &str,
    preview_scope: &str,
    panel_area: Option<&str>,
) -> Option<usize> {
    let panel_segments: Vec<&str> = panel_path.split('/').filter(|s| !s.is_empty()).collect();
    let scope_segments: Vec<&str> = preview_scope.split('/').filter(|s| !s.is_empty()).collect();
    if scope_segments.is_empty() || panel_segments.is_empty() {
        return None;
    }
    let leaf = scope_segments[scope_segments.len() - 1];
    let panel_leaf = panel_segments[panel_segments.len() - 1];
    if !path_segment_matches(panel_leaf, leaf) {
        return None;
    }

    let area = panel_area
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != "auto");
    let mut panel_index = 0usize;
    let mut matched = 0usize;
    for scope_segment in &scope_segments {
        if area.is_some_and(|value| path_segment_matches(value, scope_segment)) {
            matched += 1;
            continue;
        }
        if is_optional_slot_segment(scope_segment) {
            if is_ambiguous_content_leaf(leaf) {
                if area.is_some_and(|value| path_segment_matches(value, scope_segment)) {
                    matched += 1;
                    continue;
                }
            } else {
                matched += 1;
                continue;
            }
        }
        let mut found = false;
        while panel_index < panel_segments.len() {
            if path_segment_matches(panel_segments[panel_index], scope_segment) {
                found = true;
                matched += 1;
                panel_index += 1;
                break;
            }
            panel_index += 1;
        }
        if found {
            continue;
        }
        if is_ambiguous_content_leaf(leaf) {
            return None;
        }
        return None;
    }

    if matched == 0 {
        return None;
    }
    Some(6_000 + matched * 200 + preview_scope.len())
}

fn slot_path_align_score(
    panel_path: &str,
    preview_scope: &str,
    panel_area: Option<&str>,
) -> Option<usize> {
    let preview_scope = normalize_preview_scope_segments(preview_scope);
    if preview_scope.is_empty() {
        return None;
    }
    let panel_path = panel_path.trim().trim_matches('/');
    if panel_path.is_empty() {
        return None;
    }
    if panel_path == preview_scope || panel_path.ends_with(&format!("/{preview_scope}")) {
        return Some(9_500 + preview_scope.len());
    }
    if let Some(area) = panel_area
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != "auto")
    {
        if preview_scope.ends_with(&format!("/{area}"))
            && (panel_path.ends_with(&format!("/{area}"))
                || panel_path.contains(&format!("/{area}/")))
        {
            return Some(8_000 + preview_scope.len());
        }
    }
    let scope_leaf = preview_scope.rsplit('/').next().unwrap_or("");
    if scope_leaf.is_empty() {
        return None;
    }
    let panel_leaf = panel_path.rsplit('/').next().unwrap_or("");
    if !path_segment_matches(panel_leaf, scope_leaf) {
        return None;
    }
    if path_segments_align(panel_path, preview_scope.as_str(), false) {
        return Some(8_500 + preview_scope.len());
    }
    None
}

fn is_optional_slot_segment(segment: &str) -> bool {
    matches!(
        segment,
        "first" | "second" | "third" | "fourth" | "fifth" | "sixth" | "compound" | "main" | "top"
            | "bottom" | "primary" | "sub_a" | "sub_b" | "sub_c" | "b0" | "b1" | "b2" | "rtop"
            | "rbottom" | "summary" | "chart" | "table" | "triptych" | "secondary_a"
            | "secondary_b" | "pending" | "doing" | "done"
    )
}

fn is_section_area(value: &str) -> bool {
    let area = value.trim();
    !area.is_empty() && area != "auto" && area != "body"
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
        let scope = normalize_preview_scope_segments(node.preview_scope.trim());
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
            && !path_segments_align(panel_prefix, scope.as_str(), false)
        {
            continue;
        }
        let score = scope.len();
        if best.as_ref().is_none_or(|(best_score, _)| score >= *best_score) {
            best = Some((
                score,
                UiScopeBlockAnnotation {
                    node_id: node.node_id.clone(),
                    preview_scope: scope,
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

/// Technical slot/budget metadata for region/section/content inspector.
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
            if let Some(rows) = budget.content_rows.as_ref() {
                md.push_str(&format!("  - content_rows: `{rows:?}`\n"));
            }
            if let Some(gap) = budget.content_gap.as_deref() {
                md.push_str(&format!("  - content_gap: `{gap}`\n"));
            }
            if let Some(h) = budget.section_derived_height_px {
                md.push_str(&format!("  - section_derived_height_px: `{h:.0}`\n"));
            }
            if let Some(profile) = budget.padding_profile.as_deref() {
                md.push_str(&format!("  - padding_profile: `{profile}`\n"));
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
        UiScopeRole::Slot | UiScopeRole::Budget
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

#[cfg(test)]
mod content_align_tests {
    use super::*;

    #[test]
    fn compound_metric_card_path_aligns() {
        let score = content_path_align_score(
            "left_rail/enforcement/panel/enforcement-stats/enforcement_strip_layout/enforcement_objects_card/panel/enforcement_objects_top",
            "left_rail/enforcement/enforcement_strip_layout/compound/enforcement_objects_top",
            None,
        );
        assert!(score.is_some(), "compound metric card should align");
    }

    #[test]
    fn status_flow_metric_card_path_aligns() {
        let score = content_path_align_score(
            "right_rail/issue/panel/issue-stats/issue_status_flow/panel/metric_card",
            "right_rail/issue/issue_status_flow/summary/metric_card",
            Some("summary"),
        );
        assert!(score.is_some(), "status-flow summary card should align");
    }

    #[test]
    fn metric_summary_group_path_aligns() {
        let score = content_path_align_score(
            "left_rail/penalty/panel/penalty-stats/penalty_count_summary/primary/penalty_count_summary_primary",
            "left_rail/penalty/penalty_count_summary/primary/penalty_count_summary_primary",
            Some("primary"),
        );
        assert!(score.is_some(), "metric-summary primary card should align");
    }
}
