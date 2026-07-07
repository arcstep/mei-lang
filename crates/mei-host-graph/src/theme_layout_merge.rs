//! Merge `ops.themes.*.layout` into compiled region/section props (0327 D3).

use std::collections::BTreeMap;

use serde_json::Value;

use mei_lang_kernel::{padding_profile_css, CompiledApp, PanelDecl, UiNodeDecl};

use crate::layout_tuning_merge::{
    find_panel_mut_by_id, find_panel_mut_for_preview_scope, resolve_panel_id_for_tuning_scope,
    resolve_preview_scope_for_tuning_key,
};

pub fn merge_theme_layout_into_compiled(
    compiled: &mut CompiledApp,
    theme_id: &str,
    themes: &BTreeMap<String, Value>,
) {
    let Some(theme) = themes.get(theme_id) else {
        return;
    };
    let Some(layout) = theme.get("layout").and_then(Value::as_object) else {
        return;
    };
    if layout.is_empty() {
        return;
    }
    let index = compiled.ui_layout_index.clone();
    let entries: Vec<(String, Value)> = layout
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let Some(panels) = compiled
        .scene_contract
        .as_mut()
        .map(|contract| &mut contract.panels)
    else {
        return;
    };
    let panel_snapshot = panels.clone();
    let mut targets: Vec<(String, Value)> = Vec::new();
    for (scope_path, patch) in entries {
        let tuning_key = layout_scope_to_tuning_key(scope_path.as_str());
        if let Some(panel_scope) = resolve_preview_scope_for_tuning_key(&index, tuning_key.as_str()) {
            targets.push((panel_scope, patch));
            continue;
        }
        if let Some(panel_id) =
            resolve_panel_id_for_tuning_scope(&panel_snapshot, tuning_key.as_str())
        {
            targets.push((panel_id, patch));
            continue;
        }
        if let Some(panel_id) =
            resolve_region_panel_id_for_layout_scope(&panel_snapshot, scope_path.as_str())
        {
            targets.push((panel_id, patch));
        }
    }
    for (target_id, patch) in targets {
        if let Some(panel) = find_panel_mut_for_preview_scope(panels, target_id.as_str()) {
            apply_theme_layout_patch(panel, &patch);
            continue;
        }
        if let Some(panel) = find_panel_mut_by_id(panels, target_id.as_str()) {
            apply_theme_layout_patch(panel, &patch);
        }
    }
}

fn layout_scope_to_tuning_key(scope_path: &str) -> String {
    let normalized = scope_path.trim().trim_matches('/');
    if let Some(tail) = normalized.strip_prefix("home/T1/") {
        return tail.replace('/', "/");
    }
    if let Some(tail) = normalized.strip_prefix("home/t1/") {
        return tail.replace('/', "/");
    }
    normalized.replace('/', "/")
}

fn resolve_region_panel_id_for_layout_scope(
    panels: &[PanelDecl],
    scope_path: &str,
) -> Option<String> {
    let tail = layout_scope_to_tuning_key(scope_path);
    let region_id = tail.split('/').next().unwrap_or(tail.as_str());
    let snake = region_id.replace('-', "_");
    for panel in panels {
        if let Some(found) = find_region_panel_by_id(panel, region_id, snake.as_str()) {
            return Some(found);
        }
    }
    None
}

fn find_region_panel_by_id(panel: &PanelDecl, region_id: &str, snake_id: &str) -> Option<String> {
    if ui_role(panel) == Some("region")
        && (panel.id == region_id || panel.id == snake_id || panel.id.replace('_', "-") == region_id)
    {
        return Some(panel.id.clone());
    }
    for node in &panel.blocks {
        if let UiNodeDecl::Panel(child) = node {
            if let Some(found) = find_region_panel_by_id(child, region_id, snake_id) {
                return Some(found);
            }
        }
    }
    None
}

fn ui_role(panel: &PanelDecl) -> Option<&str> {
    panel
        .props
        .as_object()
        .and_then(|m| m.get("__mei_ui_role"))
        .and_then(Value::as_str)
}

fn apply_theme_layout_patch(panel: &mut PanelDecl, patch: &Value) {
    if let Some(rows) = patch.get("sectionRows").and_then(Value::as_array) {
        let fr_rows: Vec<String> = rows
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect();
        if !fr_rows.is_empty() {
            if let Some(layout) = panel.layout.as_mut() {
                layout.rows = Some(fr_rows);
            }
        }
    }
    if let Some(gap) = patch.get("gap").and_then(Value::as_str) {
        if let Some(layout) = panel.layout.as_mut() {
            layout.gap = Some(gap.to_string());
        }
    }
    if let Some(compound_width) = patch.get("compoundWidth").and_then(Value::as_str) {
        if let Some(map) = panel.props.as_object_mut() {
            map.insert(
                "width".to_string(),
                Value::String(compound_width.to_string()),
            );
        }
        if let Some(layout) = panel.layout.as_mut() {
            if let Some(columns) = layout.columns.as_mut() {
                for col in columns.iter_mut() {
                    if col.ends_with("px") && col.parse::<f64>().is_ok() {
                        *col = compound_width.to_string();
                    }
                }
            }
        }
    }
    if let Some(strip_gap) = patch.get("stripGap").and_then(Value::as_str) {
        if let Some(layout) = panel.layout.as_mut() {
            layout.gap = Some(strip_gap.to_string());
        }
    }
    if let Some(profile) = patch
        .get("paddingProfile")
        .or_else(|| patch.get("padding_profile"))
        .and_then(Value::as_str)
    {
        if let Some(map) = panel.props.as_object_mut() {
            map.insert(
                "__mei_padding_profile".to_string(),
                Value::String(profile.to_string()),
            );
        }
        if let Some(padding) = padding_profile_css(profile) {
            let mut body_map = panel
                .body_props
                .as_object()
                .cloned()
                .unwrap_or_default();
            body_map.insert("padding".to_string(), Value::String(padding.to_string()));
            panel.body_props = Value::Object(body_map);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mei_lang_kernel::{LayoutDecl, SceneContract, SceneDecl, UiScopeNode, UiScopeRole};
    use serde_json::json;

    #[test]
    fn apply_theme_layout_patch_updates_region_rows() {
        let mut region = PanelDecl {
            kind: "panel".to_string(),
            id: "left_rail".to_string(),
            title: None,
            head: None,
            area: None,
            layout: Some(LayoutDecl {
                layout_type: "grid".to_string(),
                direction: None,
                columns: None,
                rows: Some(vec!["1fr".to_string()]),
                areas: None,
                gap: Some("12px".to_string()),
                padding: None,
                align: None,
                justify: None,
            }),
            blocks: vec![],
            slot: None,
            props: json!({"__mei_ui_role": "region"}),
            head_props: json!({}),
            body_props: json!({}),
            base: None,
            import_scope: None,
        };
        apply_theme_layout_patch(
            &mut region,
            &json!({
                "sectionRows": ["1fr", "2fr", "3fr"],
                "gap": "8px",
            }),
        );
        assert_eq!(
            region.layout.as_ref().unwrap().rows,
            Some(vec![
                "1fr".to_string(),
                "2fr".to_string(),
                "3fr".to_string()
            ])
        );
        assert_eq!(
            region.layout.as_ref().unwrap().gap.as_deref(),
            Some("8px")
        );
    }

    #[test]
    fn apply_theme_layout_patch_sets_section_padding_profile() {
        let mut section = PanelDecl {
            kind: "panel".to_string(),
            id: "enforcement".to_string(),
            title: None,
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
        };
        apply_theme_layout_patch(
            &mut section,
            &json!({"paddingProfile": "dense_strip_100"}),
        );
        assert_eq!(
            section
                .props
                .get("__mei_padding_profile")
                .and_then(Value::as_str),
            Some("dense_strip_100")
        );
        assert!(section.body_props.get("padding").is_some());
    }
}
