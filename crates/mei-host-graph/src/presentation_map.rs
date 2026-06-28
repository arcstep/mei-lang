use std::collections::BTreeMap;

use mei_lang_kernel::{PanelDecl, UiNodeDecl};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewpointMapEntry {
    pub tier: String,
    #[serde(rename = "panelId")]
    pub panel_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "blockPath")]
    pub block_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PresentationMapDocument {
    #[serde(rename = "schemaVersion")]
    pub schema_version: String,
    pub scene: String,
    pub viewpoints: BTreeMap<String, ViewpointMapEntry>,
}

fn panel_tier(panel: &PanelDecl) -> String {
    panel
        .props
        .get("__mei_tier")
        .and_then(|v| v.as_str())
        .unwrap_or("chrome")
        .to_string()
}

fn collect_block_viewpoints(
    nodes: &[UiNodeDecl],
    panel: &PanelDecl,
    path_prefix: &str,
    out: &mut BTreeMap<String, ViewpointMapEntry>,
) {
    for (index, node) in nodes.iter().enumerate() {
        let block_path = if path_prefix.is_empty() {
            format!("{index}")
        } else {
            format!("{path_prefix}/{index}")
        };
        match node {
            UiNodeDecl::Block(block) => {
                if let Some(vp) = block.props.get("viewpoint").or_else(|| block.props.get("__mei_viewpoint"))
                {
                    if let Some(id) = resolve_viewpoint_id(vp) {
                        out.insert(
                            id.clone(),
                            ViewpointMapEntry {
                                tier: panel_tier(panel),
                                panel_id: panel.id.clone(),
                                block_path: Some(block_path),
                                label: block
                                    .props
                                    .get("label")
                                    .and_then(|v| v.as_str())
                                    .map(str::to_string),
                            },
                        );
                    }
                }
            }
            UiNodeDecl::Panel(nested) => {
                collect_block_viewpoints(&nested.blocks, nested, &block_path, out);
            }
            _ => {}
        }
    }
}

pub fn resolve_viewpoint_id(value: &Value) -> Option<String> {
    if let Some(s) = value.as_str() {
        return Some(s.to_string());
    }
    let obj = value.as_object()?;
    if let Some(id) = obj.get("id").and_then(|v| v.as_str()) {
        return Some(id.to_string());
    }
    if obj.get("__call").and_then(|v| v.as_str()) == Some("viewpoint_ref") {
        return obj
            .get("id")
            .or_else(|| obj.get("key"))
            .and_then(|v| v.as_str())
            .map(str::to_string);
    }
    None
}

pub fn merge_panel_contract_viewpoints(
    payload: &Value,
    panel_id: &str,
    tier: &str,
    out: &mut BTreeMap<String, ViewpointMapEntry>,
) {
    let Some(viewpoints) = payload.get("viewpoints").and_then(|v| v.as_array()) else {
        return;
    };
    for entry in viewpoints {
        let call = entry.get("__call").and_then(|v| v.as_str());
        if call != Some("viewpoint") {
            continue;
        }
        let args = entry.get("__args").unwrap_or(entry);
        let id = args.get("id").and_then(|v| v.as_str()).unwrap_or("");
        if id.is_empty() {
            continue;
        }
        let blocks = args
            .get("blocks")
            .and_then(|v| v.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .filter(|s| !s.is_empty());
        out.insert(
            id.to_string(),
            ViewpointMapEntry {
                tier: tier.to_string(),
                panel_id: panel_id.to_string(),
                block_path: blocks,
                label: args
                    .get("label")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
            },
        );
    }
}

pub fn build_presentation_map(
    scene_id: &str,
    panels: &[PanelDecl],
    panel_payloads: &BTreeMap<String, Value>,
) -> PresentationMapDocument {
    let mut viewpoints = BTreeMap::new();
    for panel in panels {
        let tier = panel_tier(panel);
        if let Some(vp) = panel.props.get("__mei_viewpoint") {
            if let Some(id) = resolve_viewpoint_id(vp) {
                viewpoints.insert(
                    id.clone(),
                    ViewpointMapEntry {
                        tier: tier.clone(),
                        panel_id: panel.id.clone(),
                        block_path: None,
                        label: panel.title.clone(),
                    },
                );
            }
        }
        if let Some(payload) = panel_payloads.get(panel.id.as_str()) {
            merge_panel_contract_viewpoints(payload, panel.id.as_str(), tier.as_str(), &mut viewpoints);
        }
        collect_block_viewpoints(&panel.blocks, panel, "", &mut viewpoints);
    }
    PresentationMapDocument {
        schema_version: "mei-presentation-map-v1".to_string(),
        scene: scene_id.to_string(),
        viewpoints,
    }
}

pub fn presentation_map_to_value(map: &PresentationMapDocument) -> Value {
    serde_json::to_value(map).unwrap_or(json!({}))
}
