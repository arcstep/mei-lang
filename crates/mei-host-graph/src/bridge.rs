use std::collections::BTreeMap;

use crate::mcg::registry::McgRegistry;
use crate::types::GraphNodeKind;

pub const BRIDGE_SCHEMA_VERSION: &str = "mei-graph-bridge-v1";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BridgeExportEntry {
    #[serde(rename = "mcgNode")]
    pub mcg_node: crate::types::GraphNodeId,
    #[serde(rename = "mrgNode")]
    pub mrg_node: crate::types::GraphNodeId,
    #[serde(rename = "defsFingerprint")]
    pub defs_fingerprint: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BridgeExport {
    #[serde(rename = "schemaVersion")]
    pub schema_version: String,
    #[serde(rename = "appId")]
    pub app_id: String,
    pub exports: Vec<BridgeExportEntry>,
    #[serde(rename = "invalidationPolicies")]
    pub invalidation_policies: Vec<InvalidationPolicy>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct InvalidationPolicy {
    #[serde(rename = "mcgKind")]
    pub mcg_kind: String,
    #[serde(rename = "mrgPropagate")]
    pub mrg_propagate: bool,
}

pub fn export_bridge_from_mcg(
    app_id: &str,
    registry: &McgRegistry,
    bundle_owners: &BTreeMap<String, (String, String)>,
) -> BridgeExport {
    let exports = bundle_owners
        .iter()
        .map(|(owner, (_rev, fingerprint))| BridgeExportEntry {
            mcg_node: crate::types::GraphNodeId::new(GraphNodeKind::MetricDefBundle, owner.clone()),
            mrg_node: crate::types::GraphNodeId::new(GraphNodeKind::EvalPlan, owner.clone()),
            defs_fingerprint: fingerprint.clone(),
        })
        .collect();

    for node in registry
        .nodes
        .iter()
        .filter(|n| n.id.kind == GraphNodeKind::MetricDefBundle)
    {
        if bundle_owners.contains_key(&node.id.key) {
            continue;
        }
        // fallback from MCG nodes
    }

    BridgeExport {
        schema_version: BRIDGE_SCHEMA_VERSION.to_string(),
        app_id: app_id.to_string(),
        exports,
        invalidation_policies: vec![
            InvalidationPolicy {
                mcg_kind: "metric_def_bundle".to_string(),
                mrg_propagate: true,
            },
            InvalidationPolicy {
                mcg_kind: "page_instance".to_string(),
                mrg_propagate: false,
            },
        ],
    }
}

pub fn save_bridge(source_root: &std::path::Path, bridge: &BridgeExport) -> anyhow::Result<()> {
    crate::io::write_json_registry(
        &crate::paths::bridge_path(source_root, bridge.app_id.as_str()),
        bridge,
    )
}
