use mei_lang_kernel::PanelDecl;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::tier::{default_z_index_for_tier, DEFAULT_PANEL_TIER};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerPlanPanelEntry {
    #[serde(rename = "panelId")]
    pub panel_id: String,
    #[serde(rename = "zIndex")]
    pub z_index: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tier: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "chromeRole"
    )]
    pub chrome_role: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LayerPlanDocument {
    #[serde(rename = "schemaVersion")]
    pub schema_version: String,
    pub scene: String,
    pub tiers: std::collections::BTreeMap<String, Vec<LayerPlanPanelEntry>>,
}

pub fn build_layer_plan(scene_id: &str, panels: &[PanelDecl]) -> LayerPlanDocument {
    let mut tiers: std::collections::BTreeMap<String, Vec<LayerPlanPanelEntry>> =
        std::collections::BTreeMap::new();
    for panel in panels {
        let tier = panel
            .props
            .get("__mei_tier")
            .and_then(|v| v.as_str())
            .unwrap_or(DEFAULT_PANEL_TIER)
            .to_string();
        let chrome_role = panel
            .props
            .get("__mei_chrome_role")
            .and_then(|v| v.as_str())
            .map(str::to_string);
        let z_index = panel
            .props
            .get("z_index")
            .and_then(|v| v.as_i64())
            .unwrap_or_else(|| default_z_index_for_tier(tier.as_str()));
        tiers
            .entry(tier.clone())
            .or_default()
            .push(LayerPlanPanelEntry {
                panel_id: panel.id.clone(),
                z_index,
                tier: Some(tier),
                chrome_role,
            });
    }
    for entries in tiers.values_mut() {
        entries.sort_by(|a, b| {
            a.z_index
                .cmp(&b.z_index)
                .then_with(|| a.panel_id.cmp(&b.panel_id))
        });
    }
    LayerPlanDocument {
        schema_version: "mei-layer-plan-v1".to_string(),
        scene: scene_id.to_string(),
        tiers,
    }
}

pub fn layer_plan_to_value(plan: &LayerPlanDocument) -> Value {
    serde_json::to_value(plan).unwrap_or(json!({}))
}
