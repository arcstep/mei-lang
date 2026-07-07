//! SSR merge of ops.layoutTuning into compiled panel budget fields (P2).

use std::collections::HashSet;

use serde_json::Value;

use mei_lang_kernel::{
    padding_profile_css, CompiledApp, PanelDecl, UiLayoutIndex, UiNodeDecl, UiScopeRole,
};

/// Merge tuning patches aligned to `ui_layout_index.preview_scope`, then unmapped keys via path fallback.
pub fn merge_layout_tuning_into_compiled(compiled: &mut CompiledApp, tuning: Option<&Value>) {
    let Some(tuning) = tuning.and_then(Value::as_object) else {
        return;
    };
    if tuning.is_empty() {
        return;
    }
    let index = compiled.ui_layout_index.clone();
    let mut applied = HashSet::new();
    merge_layout_tuning_via_index(compiled, tuning, &index, &mut applied);
    merge_layout_tuning_fallback(compiled, tuning, &applied);
}

pub fn merge_layout_tuning_via_index(
    compiled: &mut CompiledApp,
    tuning: &serde_json::Map<String, Value>,
    index: &UiLayoutIndex,
    applied: &mut HashSet<String>,
) {
    let Some(contract) = compiled.scene_contract.as_mut() else {
        return;
    };
    for (scope_key, patch) in tuning {
        let Some(panel_scope) = resolve_preview_scope_for_tuning_key(index, scope_key.as_str()) else {
            continue;
        };
        let Some(panel) = find_panel_mut_for_preview_scope(&mut contract.panels, panel_scope.as_str())
        else {
            continue;
        };
        apply_tuning_patch(panel, patch);
        applied.insert(scope_key.clone());
    }
}

fn merge_layout_tuning_fallback(
    compiled: &mut CompiledApp,
    tuning: &serde_json::Map<String, Value>,
    applied: &HashSet<String>,
) {
    let unmapped: Vec<(String, Value)> = tuning
        .iter()
        .filter(|(key, _)| !applied.contains(key.as_str()))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    if unmapped.is_empty() {
        return;
    }
    let Some(contract) = compiled.scene_contract.as_mut() else {
        return;
    };
    for (scope_key, patch) in unmapped {
        if let Some(panel) = find_panel_mut_for_preview_scope(&mut contract.panels, scope_key.as_str())
        {
            apply_tuning_patch(panel, &patch);
            continue;
        }
        if let Some(panel_id) = resolve_panel_id_for_tuning_scope(&contract.panels, scope_key.as_str())
        {
            if let Some(panel) = find_panel_mut_by_id(&mut contract.panels, panel_id.as_str()) {
                apply_tuning_patch(panel, &patch);
            }
        }
    }
    for panel in contract.panels.iter_mut() {
        merge_layout_tuning_panel_props_fallback(panel, tuning, applied);
    }
}

pub(crate) fn resolve_preview_scope_for_tuning_key(index: &UiLayoutIndex, scope_key: &str) -> Option<String> {
    let key = normalize_preview_scope(scope_key);
    if key.is_empty() {
        return None;
    }
    let mut best: Option<(usize, String)> = None;
    for node in index.nodes.values() {
        if !matches!(
            node.role,
            UiScopeRole::Section | UiScopeRole::Content | UiScopeRole::Budget
        ) {
            continue;
        }
        let scope = normalize_preview_scope(node.preview_scope.as_str());
        if scope.is_empty() {
            continue;
        }
        let score = tuning_scope_match_score(key.as_str(), scope.as_str());
        if score == 0 {
            continue;
        }
        if best.as_ref().is_none_or(|(best_score, _)| score > *best_score) {
            best = Some((score, scope));
        }
    }
    best.map(|(_, scope)| scope)
}

fn tuning_scope_match_score(scope_key: &str, preview_scope: &str) -> usize {
    if scope_key == preview_scope {
        return 10_000 + scope_key.len();
    }
    if preview_scope.ends_with(&format!("/{scope_key}")) {
        return 9_000 + scope_key.len();
    }
    if scope_key.ends_with(preview_scope) {
        return 8_000 + preview_scope.len();
    }
    let key_tail = scope_key.rsplit('/').next().unwrap_or(scope_key);
    let preview_tail = preview_scope.rsplit('/').next().unwrap_or(preview_scope);
    if key_tail == preview_tail && !key_tail.is_empty() {
        return 7_000 + preview_scope.len();
    }
    0
}

fn normalize_preview_scope(scope: &str) -> String {
    scope.trim().trim_matches('/').to_string()
}

pub(crate) fn resolve_panel_id_for_tuning_scope(panels: &[PanelDecl], scope_key: &str) -> Option<String> {
    let mut best: Option<(usize, String)> = None;
    walk_panels_with_path(panels, "", &mut |path, panel| {
        let score = tuning_scope_match_score(
            normalize_preview_scope(scope_key).as_str(),
            normalize_preview_scope(path).as_str(),
        );
        if score == 0 {
            return;
        }
        if best.as_ref().is_none_or(|(best_score, _)| score > *best_score) {
            best = Some((score, panel.id.clone()));
        }
    });
    best.map(|(_, id)| id)
}

fn walk_panels_with_path<F>(panels: &[PanelDecl], prefix: &str, visit: &mut F)
where
    F: FnMut(&str, &PanelDecl),
{
    for panel in panels {
        let path = if prefix.is_empty() {
            panel.id.clone()
        } else {
            format!("{prefix}/{}", panel.id)
        };
        visit(path.as_str(), panel);
        for node in &panel.blocks {
            if let UiNodeDecl::Panel(child) = node {
                walk_panels_with_path(std::slice::from_ref(child), path.as_str(), visit);
            }
        }
    }
}

pub(crate) fn find_panel_mut_for_preview_scope<'a>(
    panels: &'a mut [PanelDecl],
    scope: &str,
) -> Option<&'a mut PanelDecl> {
    let segments: Vec<&str> = scope
        .split('/')
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .collect();
    if segments.is_empty() {
        return None;
    }
    find_panel_mut_by_segments(panels, &segments)
}

fn find_panel_mut_by_segments<'a>(
    panels: &'a mut [PanelDecl],
    segments: &[&str],
) -> Option<&'a mut PanelDecl> {
    if segments.is_empty() {
        return None;
    }
    for panel in panels.iter_mut() {
        if let Some(found) = find_panel_mut_by_segments_at(panel, segments) {
            return Some(found);
        }
    }
    None
}

fn find_panel_mut_by_segments_at<'a>(
    panel: &'a mut PanelDecl,
    segments: &[&str],
) -> Option<&'a mut PanelDecl> {
    if panel.id == segments[0] {
        if segments.len() == 1 {
            return Some(panel);
        }
        return find_panel_mut_in_children(panel, &segments[1..]);
    }
    find_panel_mut_in_children(panel, segments)
}

fn find_panel_mut_in_children<'a>(
    panel: &'a mut PanelDecl,
    segments: &[&str],
) -> Option<&'a mut PanelDecl> {
    for node in panel.blocks.iter_mut() {
        if let UiNodeDecl::Panel(child) = node {
            if let Some(found) = find_panel_mut_by_segments(std::slice::from_mut(child), segments) {
                return Some(found);
            }
        }
    }
    None
}

pub(crate) fn find_panel_mut_by_id<'a>(
    panels: &'a mut [PanelDecl],
    panel_id: &str,
) -> Option<&'a mut PanelDecl> {
    for panel in panels {
        if panel.id == panel_id {
            return Some(panel);
        }
        for node in panel.blocks.iter_mut() {
            if let UiNodeDecl::Panel(child) = node {
                if let Some(found) = find_panel_mut_by_id(std::slice::from_mut(child), panel_id) {
                    return Some(found);
                }
            }
        }
    }
    None
}

fn merge_layout_tuning_panel_props_fallback(
    panel: &mut PanelDecl,
    tuning: &serde_json::Map<String, Value>,
    applied: &HashSet<String>,
) {
    apply_tuning_for_scope(panel, tuning, applied);
    for node in panel.blocks.iter_mut() {
        if let UiNodeDecl::Panel(child) = node {
            merge_layout_tuning_panel_props_fallback(child, tuning, applied);
        }
    }
}

fn apply_tuning_for_scope(
    panel: &mut PanelDecl,
    tuning: &serde_json::Map<String, Value>,
    applied: &HashSet<String>,
) {
    let Some(scope) = preview_scope_key(panel) else {
        return;
    };
    if applied.contains(scope.as_str()) {
        return;
    }
    let Some(patch) = tuning
        .get(&scope)
        .or_else(|| tuning.get(panel.id.as_str()))
    else {
        return;
    };
    apply_tuning_patch(panel, patch);
}

fn apply_tuning_patch(panel: &mut PanelDecl, patch: &Value) {
    let Some(map) = panel.props.as_object_mut() else {
        return;
    };
    if let Some(budget) = patch_field(patch, "content_budget", "contentBudget") {
        map.insert("__mei_content_budget".to_string(), budget.clone());
    }
    if let Some(profile) = patch_field(patch, "padding_profile", "paddingProfile")
        .and_then(Value::as_str)
    {
        map.insert(
            "__mei_padding_profile".to_string(),
            Value::String(profile.to_string()),
        );
        if let Some(padding) = padding_profile_css(profile) {
            let mut body_map = panel
                .body_props
                .as_object()
                .cloned()
                .unwrap_or_default();
            body_map.insert("padding".to_string(), Value::String(padding.to_string()));
            body_map
                .entry("box_sizing".to_string())
                .or_insert_with(|| Value::String("border-box".to_string()));
            body_map
                .entry("min_height".to_string())
                .or_insert_with(|| Value::String("0".to_string()));
            panel.body_props = Value::Object(body_map);
        }
    }
    if let Some(slot_height) = patch_field(patch, "slot_height", "slotHeight") {
        let px_value = slot_height
            .as_str()
            .and_then(parse_px)
            .or_else(|| slot_height.as_i64())
            .or_else(|| slot_height.as_u64().map(|value| value as i64))
            .or_else(|| slot_height.as_f64().map(|value| value.round() as i64));
        if let Some(slot_height) = px_value {
            if let Some(budget) = map.get_mut("__mei_content_budget") {
                if let Some(rows) = budget.get_mut("rows").and_then(Value::as_array_mut) {
                    if let Some(first) = rows.first_mut() {
                        *first = Value::from(slot_height);
                    }
                }
            }
        }
    }
}

fn patch_field<'a>(patch: &'a Value, snake: &str, camel: &str) -> Option<&'a Value> {
    patch.get(snake).or_else(|| patch.get(camel))
}

fn parse_px(value: &str) -> Option<i64> {
    let trimmed = value.trim();
    if trimmed == "0" {
        return Some(0);
    }
    trimmed
        .strip_suffix("px")
        .or_else(|| trimmed.strip_suffix("PX"))
        .and_then(|n| n.trim().parse().ok())
}

fn preview_scope_key(panel: &PanelDecl) -> Option<String> {
    panel
        .props
        .as_object()
        .and_then(|m| m.get("preview_scope"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mei_lang_kernel::UiScopeNode;
    use serde_json::json;

    fn section_panel(id: &str) -> PanelDecl {
        PanelDecl {
            kind: "panel".to_string(),
            id: id.to_string(),
            title: Some("Section".to_string()),
            head: None,
            area: None,
            layout: None,
            blocks: vec![],
            slot: None,
            props: json!({"__mei_ui_role": "section"}),
            head_props: json!({}),
            body_props: json!({}),
            base: None,
            import_scope: None,
        }
    }

    #[test]
    fn apply_tuning_patch_updates_body_props_padding() {
        let mut panel = PanelDecl {
            kind: "panel".to_string(),
            id: "enforcement".to_string(),
            title: Some("执法要素".to_string()),
            head: None,
            area: None,
            layout: None,
            blocks: vec![],
            slot: None,
            props: json!({
                "__mei_ui_role": "section",
                "__mei_padding_profile": "dense_strip_100",
            }),
            head_props: json!({}),
            body_props: json!({"padding": "8px 4px 2px 4px"}),
            base: None,
            import_scope: None,
        };
        apply_tuning_patch(
            &mut panel,
            &json!({"paddingProfile": "compact"}),
        );
        assert_eq!(
            panel
                .props
                .get("__mei_padding_profile")
                .and_then(Value::as_str),
            Some("compact")
        );
        assert_eq!(
            panel
                .body_props
                .get("padding")
                .and_then(Value::as_str),
            Some("8px 6px 6px 6px")
        );
    }

    #[test]
    fn merge_via_index_hits_section_preview_scope_not_leaf_id_only() {
        let mut left_rail = section_panel("left_rail");
        left_rail.blocks = vec![UiNodeDecl::Panel(section_panel("enforcement"))];
        let mut panels = vec![left_rail];
        let mut index = UiLayoutIndex::default();
        index.nodes.insert(
            "section-enforcement".to_string(),
            UiScopeNode {
                node_id: "section-enforcement".to_string(),
                role: UiScopeRole::Section,
                label: "执法要素".to_string(),
                scope_path: vec![],
                plane: None,
                parent_id: None,
                children: vec![],
                preview_scope: "left_rail/enforcement".to_string(),
                budget: None,
                source_anchors: vec![],
                content_kind: None,
                scene_id: Some("home".to_string()),
            },
        );
        let tuning = serde_json::Map::from_iter([(
            "left_rail/enforcement".to_string(),
            json!({"paddingProfile": "compact"}),
        )]);
        let scope = resolve_preview_scope_for_tuning_key(&index, "left_rail/enforcement")
            .expect("scope");
        assert_eq!(scope, "left_rail/enforcement");
        let panel = find_panel_mut_for_preview_scope(&mut panels, scope.as_str()).expect("panel");
        apply_tuning_patch(panel, tuning.get("left_rail/enforcement").unwrap());
        assert_eq!(
            panel
                .props
                .get("__mei_padding_profile")
                .and_then(Value::as_str),
            Some("compact")
        );
    }

    #[test]
    fn resolve_panel_id_for_tuning_scope_finds_nested_section_under_plane() {
        let mut enforcement = section_panel("enforcement");
        enforcement.props = json!({"__mei_ui_role": "section"});
        let mut left_rail = section_panel("left_rail");
        left_rail.props = json!({"__mei_ui_role": "region"});
        left_rail.blocks = vec![UiNodeDecl::Panel(enforcement)];
        let mut t1 = section_panel("t1");
        t1.props = json!({"__mei_ui_role": "plane"});
        t1.blocks = vec![UiNodeDecl::Panel(left_rail)];
        let panel_id =
            resolve_panel_id_for_tuning_scope(&[t1], "left_rail/enforcement").expect("panel id");
        assert_eq!(panel_id, "enforcement");
    }
}
