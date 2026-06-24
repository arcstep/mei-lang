use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::graph::io::{read_json_registry, write_json_registry};
use crate::graph::mcg::metric_def_bundle::MetricDefBundleRecord;
use crate::graph::paths::bridge_path;
use crate::graph::types::GraphNodeId;

pub const BRIDGE_SCHEMA_VERSION: &str = "mei-graph-bridge-v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeExportEntry {
    #[serde(rename = "mcgNode")]
    pub mcg_node: GraphNodeId,
    #[serde(rename = "mrgNode")]
    pub mrg_node: GraphNodeId,
    #[serde(rename = "defsFingerprint")]
    pub defs_fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvalidationPolicy {
    #[serde(rename = "mcgKind")]
    pub mcg_kind: String,
    #[serde(rename = "mrgPropagate")]
    pub mrg_propagate: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(default, rename = "mrgTargets", skip_serializing_if = "Vec::is_empty")]
    pub mrg_targets: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeExport {
    #[serde(rename = "schemaVersion")]
    pub schema_version: String,
    #[serde(rename = "appId")]
    pub app_id: String,
    pub exports: Vec<BridgeExportEntry>,
    #[serde(rename = "invalidationPolicies")]
    pub invalidation_policies: Vec<InvalidationPolicy>,
}

pub fn export_bridge(
    app_id: &str,
    bundles: &BTreeMap<String, MetricDefBundleRecord>,
) -> BridgeExport {
    use crate::graph::types::GraphNodeKind;
    let exports = bundles
        .values()
        .map(|bundle| BridgeExportEntry {
            mcg_node: GraphNodeId::new(
                GraphNodeKind::MetricDefBundle,
                bundle.owner_resource_id.clone(),
            ),
            mrg_node: GraphNodeId::new(
                GraphNodeKind::EvalPlan,
                bundle.owner_resource_id.clone(),
            ),
            defs_fingerprint: bundle.defs_fingerprint.clone(),
        })
        .collect();
    BridgeExport {
        schema_version: BRIDGE_SCHEMA_VERSION.to_string(),
        app_id: app_id.to_string(),
        exports,
        invalidation_policies: default_invalidation_policies(),
    }
}

fn default_invalidation_policies() -> Vec<InvalidationPolicy> {
    vec![
        InvalidationPolicy {
            mcg_kind: "scene_payload".to_string(),
            mrg_propagate: false,
            note: Some("UI-only; unless metric ref in panel props changes".to_string()),
            mrg_targets: Vec::new(),
        },
        InvalidationPolicy {
            mcg_kind: "metric_def_bundle".to_string(),
            mrg_propagate: true,
            note: None,
            mrg_targets: vec!["eval_plan".to_string(), "material_slot".to_string()],
        },
    ]
}

pub struct BridgeWriter;

impl BridgeWriter {
    pub fn load(source_root: &std::path::Path, app_id: &str) -> Option<BridgeExport> {
        read_json_registry::<BridgeExport>(&bridge_path(source_root, app_id))
            .ok()
            .flatten()
            .filter(|bridge| bridge.schema_version == BRIDGE_SCHEMA_VERSION)
    }

    pub fn save(source_root: &std::path::Path, bridge: &BridgeExport) -> anyhow::Result<()> {
        write_json_registry(&bridge_path(source_root, bridge.app_id.as_str()), bridge)
    }
}
