use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{anyhow, Result};
use mei_lang_kernel::resolve_app_root;

use crate::graph::load_mrg_registry;
use crate::graph::mcg::registry::McgRegistryWriter;
use crate::graph::types::{GraphNodeKind, MaterialState};

use super::types::{BlockId, BlockResult};

pub fn block_inspect(source_root: &Path, app_id: &str, block_id: &BlockId) -> Result<BlockResult> {
    let app_root = resolve_app_root(source_root, app_id);
    let mut details = BTreeMap::new();
    match block_id.kind {
        GraphNodeKind::ScenePayload | GraphNodeKind::MetricDefBundle => {
            let mcg = McgRegistryWriter::load(source_root, app_id);
            let node = mcg
                .nodes
                .iter()
                .find(|node| node.id.kind == block_id.kind && node.id.key == block_id.key)
                .ok_or_else(|| anyhow!("MCG node not found: {}", block_id.stable_key()))?;
            if let Some(pref) = node.payload_ref.as_ref() {
                details.insert("contentHash".to_string(), pref.content_hash.clone());
                details.insert("payloadKind".to_string(), pref.kind.clone());
            }
            details.insert("nodeRevision".to_string(), node.revision.clone());
            let mut result = BlockResult::ok(block_id.clone(), "inspect");
            result.details = details;
            Ok(result)
        }
        GraphNodeKind::MaterialSlot | GraphNodeKind::Workset => {
            let mrg = load_mrg_registry(source_root, app_id);
            let scope_key = block_id.scope_key.as_deref().unwrap_or("");
            let slot = mrg
                .slots
                .iter()
                .find(|slot| slot.slot_id.node.key == block_id.key && slot.slot_id.scope_key == scope_key)
                .ok_or_else(|| anyhow!("MRG slot not found: {}", block_id.stable_key()))?;
            details.insert("owner".to_string(), slot.owner_resource_id.clone());
            details.insert("state".to_string(), format!("{:?}", slot.state));
            details.insert(
                "bundleRevision".to_string(),
                slot.metric_def_bundle_revision.clone(),
            );
            if let Some(pref) = slot.payload_ref.as_ref() {
                details.insert("contentHash".to_string(), pref.content_hash.clone());
                let artifact = app_root.join(format!(
                    "build/active/metric-response/{}.json",
                    pref.content_hash
                ));
                if artifact.is_file() {
                    details.insert("artifactPath".to_string(), artifact.display().to_string());
                }
            }
            if slot.state == MaterialState::Failed {
                return Ok(BlockResult {
                    ok: false,
                    block_id: block_id.clone(),
                    action: "inspect".to_string(),
                    slot_state: Some("Failed".to_string()),
                    error_chain: Some(format!(
                        "MRG slot Failed owner={} scopeKey={scope_key}",
                        slot.owner_resource_id
                    )),
                    details,
                    ..BlockResult::ok(block_id.clone(), "inspect")
                });
            }
            let mut result = BlockResult::ok(block_id.clone(), "inspect");
            result.slot_state = Some(format!("{:?}", slot.state));
            result.details = details;
            Ok(result)
        }
        other => Err(anyhow!(
            "block inspect not supported for kind `{}`",
            other.slug()
        )),
    }
}
