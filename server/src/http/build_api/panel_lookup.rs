//! Resolve v2 content panel keys (`content/inspection-stats`) to MCG `panel_contract` nodes.

use crate::graph::mcg::registry::{McgNodeRecord, McgRegistry};
use crate::graph::types::GraphNodeKind;

pub fn panel_contract_lookup_keys(panel_key: &str, scene_id: &str) -> Vec<String> {
    let mut keys = Vec::new();
    let mut push = |key: &str| {
        if !keys.iter().any(|existing| existing == key) {
            keys.push(key.to_string());
        }
    };

    if panel_key.starts_with("panel_contract:") {
        push(panel_key);
        if let Some(stripped) = panel_key.strip_prefix("panel_contract:") {
            push(stripped);
        }
        return keys;
    }

    push(&format!("panel_contract:{panel_key}"));
    push(panel_key);
    if !panel_key.contains(':') {
        push(&format!("panel_contract:{scene_id}:{panel_key}"));
        push(&format!("{scene_id}:{panel_key}"));
    }
    if let Some(basename) = panel_key.rsplit('/').next() {
        if basename != panel_key {
            push(&format!("panel_contract:{basename}"));
            push(basename);
        }
    }
    keys
}

pub fn find_panel_contract_node<'a>(
    registry: &'a McgRegistry,
    panel_key: &str,
    scene_id: &str,
) -> Option<&'a McgNodeRecord> {
    for key in panel_contract_lookup_keys(panel_key, scene_id) {
        if let Some(node) = registry.nodes.iter().find(|node| {
            node.id.kind == GraphNodeKind::PanelContract && node.id.key == key
        }) {
            return Some(node);
        }
    }
    None
}

pub fn panel_preview_target(panel_key: &str) -> String {
    if let Some((scene, panel)) = panel_key.split_once(':') {
        if !scene.contains('/') && !panel.is_empty() {
            let file_slug = panel.replace('_', "-");
            return format!("src/scene/{scene}/{file_slug}.panel.mei");
        }
    }
    let basename = panel_key.rsplit('/').next().unwrap_or(panel_key);
    format!("src/content/panels/{basename}.panel.mei")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_content_inspection_stats_by_basename() {
        let keys = panel_contract_lookup_keys("content/inspection-stats", "home");
        assert!(keys.contains(&"panel_contract:inspection-stats".to_string()));
    }

    #[test]
    fn preview_target_from_content_key() {
        assert_eq!(
            panel_preview_target("content/inspection-stats"),
            "src/content/panels/inspection-stats.panel.mei"
        );
    }
}
