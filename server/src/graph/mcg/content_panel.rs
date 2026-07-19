use std::collections::BTreeMap;
use std::path::Path;

use mei_lang_kernel::CompiledApp;
use serde::{Deserialize, Serialize};

use crate::graph::content_store::{self, CONTENT_PANEL};
use crate::graph::types::stable_hash;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentPanelRecord {
    pub panel_key: String,
    pub scene_id: String,
    pub panel_id: String,
    pub revision: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub panel: Option<serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct PersistedContentPanel {
    pub content_hash: String,
}

pub fn extract_content_panels(compiled: &CompiledApp) -> Vec<ContentPanelRecord> {
    let scene_id = compiled
        .active_scene
        .as_deref()
        .unwrap_or("default")
        .to_string();
    let Some(contract) = compiled.scene_contract.as_ref() else {
        return Vec::new();
    };
    extract_from_contract(contract, scene_id.as_str())
}

fn extract_from_contract(
    contract: &mei_lang_kernel::SceneContract,
    scene_id: &str,
) -> Vec<ContentPanelRecord> {
    contract
        .panels
        .iter()
        .map(|panel| {
            let panel_key = format!("{scene_id}:{}", panel.id);
            let fingerprint = stable_hash(&serde_json::to_string(panel).unwrap_or_default());
            ContentPanelRecord {
                panel_key,
                scene_id: scene_id.to_string(),
                panel_id: panel.id.clone(),
                revision: format!("pc:{fingerprint}"),
                panel: Some(serde_json::to_value(panel).unwrap_or(serde_json::Value::Null)),
            }
        })
        .collect()
}

/// Persist panel contract payloads (P2 partial assembly path).
pub fn persist_content_panels(
    app_root: &Path,
    records: &[ContentPanelRecord],
) -> anyhow::Result<BTreeMap<String, PersistedContentPanel>> {
    let mut paths = BTreeMap::new();
    for record in records {
        let bytes = serde_json::to_vec(record)?;
        let put = content_store::put_if_absent(app_root, CONTENT_PANEL, &bytes)?;
        paths.insert(
            record.panel_key.clone(),
            PersistedContentPanel {
                content_hash: put.content_hash,
            },
        );
    }
    Ok(paths)
}

pub fn load_content_panels_from_store(
    app_root: &Path,
    mcg: &crate::graph::mcg::registry::McgRegistry,
) -> anyhow::Result<BTreeMap<String, serde_json::Value>> {
    use crate::graph::content_store::{self, CONTENT_PANEL};
    use crate::graph::types::GraphNodeKind;
    let mut panels = BTreeMap::new();
    for node in &mcg.nodes {
        if node.id.kind != GraphNodeKind::ContentPanel {
            continue;
        }
        let Some(hash) = node
            .payload_ref
            .as_ref()
            .map(|payload| payload.content_hash.as_str())
            .filter(|hash| !hash.is_empty())
        else {
            continue;
        };
        let Some(path) = content_store::get(app_root, CONTENT_PANEL, hash) else {
            continue;
        };
        let raw = std::fs::read_to_string(path)?;
        let record: ContentPanelRecord = serde_json::from_str(&raw)?;
        if let Some(panel) = record.panel {
            panels.insert(record.panel_key.clone(), panel);
        }
    }
    Ok(panels)
}

pub fn partial_assemble_panel_merge(
    base: &CompiledApp,
    changed_panels: &BTreeMap<String, serde_json::Value>,
) -> CompiledApp {
    let mut merged = base.clone();
    let scene_id = merged
        .active_scene
        .as_deref()
        .unwrap_or("default")
        .to_string();
    if let Some(contract) = merged.scene_contract.as_mut() {
        for panel in &mut contract.panels {
            let key = format!("{scene_id}:{}", panel.id);
            if let Some(replacement) = changed_panels.get(&key) {
                if let Ok(updated) = serde_json::from_value(replacement.clone()) {
                    *panel = updated;
                }
            }
        }
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;
    use mei_lang_kernel::{SceneContract, SceneDecl, UiNodeDecl};

    #[test]
    fn extract_content_panel_from_scene() {
        let contract = SceneContract {
            scene: SceneDecl {
                kind: "scene".to_string(),
                id: "home".to_string(),
                world: None,
                flow: None,
                frame: None,
                profile: None,
                theme: None,
                summary: None,
                goal: None,
                state: serde_json::Value::Null,
                shared: serde_json::Value::Null,
                local_nav: serde_json::Value::Null,
                params: serde_json::Value::Null,
                capabilities: serde_json::Value::Null,
                bindings: serde_json::Value::Null,
                examples: serde_json::Value::Null,
                access_export: true,
                t2_pages: Vec::new(),
            },
            themes: Vec::new(),
            shared: serde_json::Value::Null,
            world: None,
            flow: None,
            frame: None,
            panels: vec![UiNodeDecl {
                id: "left".to_string(),
                kind: "panel".to_string(),
                ..Default::default()
            }],
        };
        let records = extract_from_contract(&contract, "home");
        assert_eq!(records.len(), 1);
        assert!(records[0].revision.starts_with("pc:"));
    }
}
