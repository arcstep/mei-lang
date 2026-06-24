use std::collections::BTreeMap;
use std::path::Path;

use mei_lang_kernel::CompiledApp;
use serde::{Deserialize, Serialize};

use crate::graph::io::write_json_registry;
use crate::graph::paths::panel_contract_artifact_dir;
use crate::graph::types::stable_hash;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelContractRecord {
    pub panel_key: String,
    pub scene_id: String,
    pub panel_id: String,
    pub revision: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relative_path: Option<String>,
}

pub fn extract_panel_contracts(compiled: &CompiledApp) -> Vec<PanelContractRecord> {
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
) -> Vec<PanelContractRecord> {
    contract
        .panels
        .iter()
        .map(|panel| {
            let panel_key = format!("{scene_id}:{}", panel.id);
            let fingerprint = stable_hash(&serde_json::to_string(panel).unwrap_or_default());
            PanelContractRecord {
                panel_key,
                scene_id: scene_id.to_string(),
                panel_id: panel.id.clone(),
                revision: format!("pc:{fingerprint}"),
                relative_path: None,
            }
        })
        .collect()
}

/// Persist panel contract payloads (P2 partial assembly path).
pub fn persist_panel_contracts(
    app_root: &Path,
    records: &[PanelContractRecord],
) -> anyhow::Result<BTreeMap<String, String>> {
    let dir = panel_contract_artifact_dir(app_root);
    std::fs::create_dir_all(&dir)?;
    let mut paths = BTreeMap::new();
    for record in records {
        let slug = record.panel_key.replace([':', '/'], "-");
        let rel = format!(".mei/graph/payloads/panel/{slug}.json");
        let path = app_root.join(&rel);
        write_json_registry(&path, record)?;
        paths.insert(record.panel_key.clone(), rel);
    }
    Ok(paths)
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
    use mei_lang_kernel::{PanelDecl, SceneContract, SceneDecl};

    #[test]
    fn extract_panel_contract_from_scene() {
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
                bindings: serde_json::Value::Null,
                examples: serde_json::Value::Null,
                access_export: true,
            },
            themes: Vec::new(),
            shared: serde_json::Value::Null,
            world: None,
            flow: None,
            frame: None,
            panels: vec![PanelDecl {
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
