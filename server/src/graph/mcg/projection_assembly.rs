//! Per-export projection assembly slices (home cockpit overlays).

use std::collections::BTreeMap;
use std::path::Path;

use mei_lang_kernel::CompiledApp;
use serde_json::Value;

use crate::graph::content_store::{self, PROJECTION_ASSEMBLY};
use crate::graph::mcg::registry::McgNodeRecord;
use crate::graph::types::{GraphNodeId, GraphNodeKind, MaterialState, PayloadRef};

pub const PROJECTION_ASSEMBLY_ARTIFACT_SCHEMA: &str = "mei-projection-assembly-v1";

pub fn is_home_scene_payload_target(target_file: &str) -> bool {
    let canonical = mei_lang_kernel::canonical_app_source_rel_path(target_file.trim());
    canonical.ends_with("home.mei")
}

pub fn persist_projection_assemblies(
    app_root: &Path,
    owner_target: &str,
    projections: &BTreeMap<String, Value>,
) -> anyhow::Result<BTreeMap<String, String>> {
    let mut hashes = BTreeMap::new();
    for (projection_id, value) in projections {
        let artifact = serde_json::json!({
            "schemaVersion": PROJECTION_ASSEMBLY_ARTIFACT_SCHEMA,
            "ownerTarget": owner_target,
            "projectionId": projection_id,
            "payload": value,
        });
        let bytes = serde_json::to_vec(&artifact)?;
        let put = content_store::put_if_absent(app_root, PROJECTION_ASSEMBLY, &bytes)?;
        hashes.insert(projection_id.clone(), put.content_hash);
    }
    Ok(hashes)
}

pub fn projection_assembly_mcg_nodes(
    owner_target: &str,
    hashes: &BTreeMap<String, String>,
) -> Vec<McgNodeRecord> {
    hashes
        .iter()
        .map(|(projection_id, content_hash)| {
            let node_key = format!("{owner_target}#{projection_id}");
            McgNodeRecord {
                id: GraphNodeId::new(GraphNodeKind::AssemblyView, node_key),
                revision: format!("pa:{content_hash}"),
                state: MaterialState::Ready,
                layer: "assembly".to_string(),
                payload_ref: Some(PayloadRef::new(
                    PROJECTION_ASSEMBLY,
                    content_hash.clone(),
                    PROJECTION_ASSEMBLY_ARTIFACT_SCHEMA,
                )),
                deps: vec![format!("scene_payload:{owner_target}")],
                defs_fingerprint: None,
                owner_resource_id: None,
                assembly_inputs: Vec::new(),
                stats: None,
            }
        })
        .collect()
}

pub fn hydrate_projection_assemblies_from_mcg(
    app_root: &Path,
    mcg: &crate::graph::mcg::registry::McgRegistry,
    owner_target: &str,
    compiled: &mut CompiledApp,
) {
    if !compiled.scene_projection_assembly_by_id.is_empty() {
        return;
    }
    let owner_keys = mei_lang_kernel::app_source_rel_path_lookup_keys(owner_target);
    for node in &mcg.nodes {
        if node.id.kind != GraphNodeKind::AssemblyView {
            continue;
        }
        let key = node.id.key.as_str();
        if !key.contains('#') {
            continue;
        }
        let Some((node_target, projection_id)) = key.split_once('#') else {
            continue;
        };
        if !owner_keys.iter().any(|lookup| lookup == node_target) {
            continue;
        }
        let Some(hash) = node
            .payload_ref
            .as_ref()
            .filter(|payload| payload.kind == PROJECTION_ASSEMBLY)
            .map(|payload| payload.content_hash.as_str())
            .filter(|hash| !hash.is_empty())
        else {
            continue;
        };
        let Some(path) = content_store::get(app_root, PROJECTION_ASSEMBLY, hash) else {
            continue;
        };
        let Ok(raw) = std::fs::read_to_string(path) else {
            continue;
        };
        let Ok(artifact) = serde_json::from_str::<serde_json::Value>(&raw) else {
            continue;
        };
        let payload = artifact.get("payload").cloned().unwrap_or(Value::Null);
        compiled
            .scene_projection_assembly_by_id
            .insert(projection_id.to_string(), payload);
    }
}
