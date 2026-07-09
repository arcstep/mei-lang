//! Resolve preview_scope / panel id for theme layout merge (ops.themes.*.layout).

use mei_lang_kernel::{UiNodeDecl, UiLayoutIndex, UiTreeNode, UiScopeRole};

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


pub(crate) fn resolve_panel_id_for_tuning_scope(panels: &[UiNodeDecl], scope_key: &str) -> Option<String> {
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


fn walk_panels_with_path<F>(panels: &[UiNodeDecl], prefix: &str, visit: &mut F)
where
    F: FnMut(&str, &UiNodeDecl),
{
    for panel in panels {
        let path = if prefix.is_empty() {
            panel.id.clone()
        } else {
            format!("{prefix}/{}", panel.id)
        };
        visit(path.as_str(), panel);
        for node in &panel.blocks {
            if let UiTreeNode::Panel(child) = node {
                walk_panels_with_path(std::slice::from_ref(child), path.as_str(), visit);
            }
        }
    }
}


pub(crate) fn find_panel_mut_for_preview_scope<'a>(
    panels: &'a mut [UiNodeDecl],
    scope: &str,
) -> Option<&'a mut UiNodeDecl> {
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
    panels: &'a mut [UiNodeDecl],
    segments: &[&str],
) -> Option<&'a mut UiNodeDecl> {
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
    panel: &'a mut UiNodeDecl,
    segments: &[&str],
) -> Option<&'a mut UiNodeDecl> {
    if panel.id == segments[0] {
        if segments.len() == 1 {
            return Some(panel);
        }
        return find_panel_mut_in_children(panel, &segments[1..]);
    }
    find_panel_mut_in_children(panel, segments)
}


fn find_panel_mut_in_children<'a>(
    panel: &'a mut UiNodeDecl,
    segments: &[&str],
) -> Option<&'a mut UiNodeDecl> {
    for node in panel.blocks.iter_mut() {
        if let UiTreeNode::Panel(child) = node {
            if let Some(found) = find_panel_mut_by_segments(std::slice::from_mut(child), segments) {
                return Some(found);
            }
        }
    }
    None
}


pub(crate) fn find_panel_mut_by_id<'a>(
    panels: &'a mut [UiNodeDecl],
    panel_id: &str,
) -> Option<&'a mut UiNodeDecl> {
    for panel in panels {
        if panel.id == panel_id {
            return Some(panel);
        }
        for node in panel.blocks.iter_mut() {
            if let UiTreeNode::Panel(child) = node {
                if let Some(found) = find_panel_mut_by_id(std::slice::from_mut(child), panel_id) {
                    return Some(found);
                }
            }
        }
    }
    None
}


