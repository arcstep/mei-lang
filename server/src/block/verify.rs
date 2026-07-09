use std::path::Path;

use anyhow::{anyhow, Result};
use mei_lang_kernel::resolve_app_root;

use crate::graph::bridge::BridgeWriter;
use crate::graph::content_store::resolve_payload_ref;
use crate::graph::mcg::metric_def_bundle::load_metric_def_bundle;
use crate::graph::mcg::registry::McgRegistryWriter;
use crate::graph::mcg::scene_payload::load_scene_payload_artifact;
use crate::graph::mrg::navigation::list_navigation_entries;
use crate::graph::types::{GraphNodeKind, MaterialState};
use crate::graph::{load_mrg_registry, run_graph_doctor};

use super::types::{BlockId, BlockResult, LayerVerifyAlert, LayerVerifyReport};

pub fn block_verify(source_root: &Path, app_id: &str, block_id: &BlockId) -> Result<BlockResult> {
    let app_root = resolve_app_root(source_root, app_id);
    match block_id.kind {
        GraphNodeKind::ScenePayload => {
            let mcg = McgRegistryWriter::load(source_root, app_id);
            let node = mcg
                .nodes
                .iter()
                .find(|node| {
                    node.id.kind == GraphNodeKind::ScenePayload && node.id.key == block_id.key
                })
                .ok_or_else(|| anyhow!("MCG scene_payload node missing for `{}`", block_id.key))?;
            let pref = node
                .payload_ref
                .as_ref()
                .ok_or_else(|| anyhow!("scene_payload `{}` has no payloadRef", block_id.key))?;
            if resolve_payload_ref(app_root.as_path(), pref).is_none() {
                return Ok(BlockResult::err(
                    block_id.clone(),
                    "verify",
                    &anyhow!(
                        "scene_payload CAS missing kind={} hash={}",
                        pref.kind,
                        pref.content_hash
                    ),
                ));
            }
            Ok(BlockResult::ok(block_id.clone(), "verify"))
        }
        GraphNodeKind::MetricDefBundle => {
            let mcg = McgRegistryWriter::load(source_root, app_id);
            let node = mcg
                .nodes
                .iter()
                .find(|node| {
                    node.id.kind == GraphNodeKind::MetricDefBundle && node.id.key == block_id.key
                })
                .ok_or_else(|| {
                    anyhow!("MCG metric_def_bundle node missing for `{}`", block_id.key)
                })?;
            let pref = node.payload_ref.as_ref().ok_or_else(|| {
                anyhow!("metric_def_bundle `{}` has no payloadRef", block_id.key)
            })?;
            let bundle = load_metric_def_bundle(app_root.as_path(), pref.content_hash.as_str())?;
            if bundle.is_none() {
                return Ok(BlockResult::err(
                    block_id.clone(),
                    "verify",
                    &anyhow!(
                        "metric_def_bundle CAS missing hash={}",
                        pref.content_hash
                    ),
                ));
            }
            Ok(BlockResult::ok(block_id.clone(), "verify"))
        }
        GraphNodeKind::DataSource => verify_data_source_block(source_root, app_id, block_id),
        GraphNodeKind::MaterialSlot | GraphNodeKind::Workset => {
            let mrg = load_mrg_registry(source_root, app_id);
            let scope_key = block_id.scope_key.as_deref().unwrap_or("");
            let slot = mrg
                .slots
                .iter()
                .find(|slot| slot.slot_id.node.key == block_id.key && slot.slot_id.scope_key == scope_key)
                .ok_or_else(|| {
                    anyhow!(
                        "MRG slot missing key=`{}` scopeKey=`{scope_key}`",
                        block_id.key
                    )
                })?;
            if slot.state == MaterialState::Failed {
                return Ok(BlockResult::err(
                    block_id.clone(),
                    "verify",
                    &anyhow!("MRG slot state=Failed owner={}", slot.owner_resource_id),
                ));
            }
            let mut result = BlockResult::ok(block_id.clone(), "verify");
            result.slot_state = Some(format!("{:?}", slot.state));
            if let Some(pref) = slot.payload_ref.as_ref() {
                result.content_hash = Some(pref.content_hash.clone());
            }
            Ok(result)
        }
        other => Err(anyhow!(
            "block verify not supported for kind `{}`",
            other.slug()
        )),
    }
}

pub fn layer_verify(
    source_root: &Path,
    app_id: &str,
    layer: &str,
) -> Result<LayerVerifyReport> {
    let layer = layer.trim().to_ascii_lowercase();
    let mut alerts = Vec::new();
    match layer.as_str() {
        "mcg" | "l3" => verify_mcg_layer(source_root, app_id, &mut alerts)?,
        "mrg" | "l4" => verify_mrg_layer(source_root, app_id, &mut alerts)?,
        "all" => {
            verify_mcg_layer(source_root, app_id, &mut alerts)?;
            verify_mrg_layer(source_root, app_id, &mut alerts)?;
            let doctor = run_graph_doctor(source_root, app_id);
            for alert in doctor.alerts {
                alerts.push(LayerVerifyAlert {
                    layer: alert.layer,
                    block_id: String::new(),
                    message: alert.message,
                });
            }
        }
        other => anyhow::bail!("unknown layer `{other}`; use mcg|mrg|all"),
    }
    Ok(LayerVerifyReport {
        app_id: app_id.to_string(),
        layer,
        ok: alerts.is_empty(),
        alerts,
    })
}

fn verify_mcg_layer(
    source_root: &Path,
    app_id: &str,
    alerts: &mut Vec<LayerVerifyAlert>,
) -> Result<()> {
    let app_root = resolve_app_root(source_root, app_id);
    let mcg = McgRegistryWriter::load(source_root, app_id);
    for node in &mcg.nodes {
        if !matches!(
            node.id.kind,
            GraphNodeKind::ScenePayload | GraphNodeKind::MetricDefBundle
        ) {
            continue;
        }
        let Some(pref) = node.payload_ref.as_ref() else {
            alerts.push(LayerVerifyAlert {
                layer: "L3".to_string(),
                block_id: format!("{}:{}", node.id.kind.slug(), node.id.key),
                message: "missing payloadRef".to_string(),
            });
            continue;
        };
        if payload_ref_exists(app_root.as_path(), pref, node.id.key.as_str()) {
            continue;
        }
        alerts.push(LayerVerifyAlert {
            layer: "L3".to_string(),
            block_id: format!("{}:{}", node.id.kind.slug(), node.id.key),
            message: format!(
                "CAS missing kind={} hash={}",
                pref.kind, pref.content_hash
            ),
        });
    }
    verify_bridge_exports(source_root, app_id, alerts);
    verify_page_instance_inputs(source_root, app_id, app_root.as_path(), alerts);
    Ok(())
}

fn verify_bridge_exports(
    source_root: &Path,
    app_id: &str,
    alerts: &mut Vec<LayerVerifyAlert>,
) {
    let Some(bridge) = BridgeWriter::load(source_root, app_id) else {
        alerts.push(LayerVerifyAlert {
            layer: "L3".to_string(),
            block_id: "bridge".to_string(),
            message: "bridge.json missing".to_string(),
        });
        return;
    };
    let mcg = McgRegistryWriter::load(source_root, app_id);
    for export in &bridge.exports {
        let mcg_present = mcg.nodes.iter().any(|node| {
            node.id.kind == export.mcg_node.kind && node.id.key == export.mcg_node.key
        });
        if !mcg_present {
            alerts.push(LayerVerifyAlert {
                layer: "L3".to_string(),
                block_id: format!(
                    "bridge:{}:{}",
                    export.mcg_node.kind.slug(),
                    export.mcg_node.key
                ),
                message: "bridge export MCG node missing".to_string(),
            });
        }
    }
}

fn verify_page_instance_inputs(
    source_root: &Path,
    app_id: &str,
    app_root: &Path,
    alerts: &mut Vec<LayerVerifyAlert>,
) {
    let mcg = McgRegistryWriter::load(source_root, app_id);
    for node in &mcg.nodes {
        if node.id.kind != GraphNodeKind::PageInstance {
            continue;
        }
        for input in &node.assembly_inputs {
            let input_present = mcg.nodes.iter().any(|candidate| {
                candidate.id.kind.slug() == input.kind.as_str()
                    && candidate.id.key == input.key
                    && candidate
                        .payload_ref
                        .as_ref()
                        .is_some_and(|pref| payload_ref_exists(app_root, pref, candidate.id.key.as_str()))
            });
            if !input_present {
                alerts.push(LayerVerifyAlert {
                    layer: "L3".to_string(),
                    block_id: format!("page_instance:{}:input", node.id.key),
                    message: format!(
                        "PageInstance input unresolved kind={} key={}",
                        input.kind, input.key
                    ),
                });
            }
        }
    }
}

fn payload_ref_exists(app_root: &Path, pref: &crate::graph::types::PayloadRef, node_key: &str) -> bool {
    if resolve_payload_ref(app_root, pref).is_some() {
        return true;
    }
    match pref.kind.as_str() {
        "scene_payload" => load_scene_payload_artifact(app_root, node_key, None, None).is_ok(),
        "metric_response" => mei_lang_datasets::metric_response_result_artifact_exists(
            app_root,
            pref.content_hash.as_str(),
        ),
        "metric_dataframe" => mei_lang_datasets::metric_dataframe_result_artifact_exists(
            app_root,
            pref.content_hash.as_str(),
        ),
        _ => load_metric_def_bundle(app_root, pref.content_hash.as_str())
            .ok()
            .flatten()
            .is_some(),
    }
}

fn verify_data_source_block(
    source_root: &Path,
    app_id: &str,
    block_id: &BlockId,
) -> Result<BlockResult> {
    let app_root = resolve_app_root(source_root, app_id);
    let manifest_path = mei_lang_kernel::data_snapshot_import_manifest_path(app_root.as_path());
    let Ok(raw) = std::fs::read_to_string(&manifest_path) else {
        return Ok(BlockResult::err(
            block_id.clone(),
            "verify",
            &anyhow!("data snapshot import manifest missing"),
        ));
    };
    let Ok(manifest) =
        serde_json::from_str::<mei_lang_kernel::DataSnapshotImportManifest>(&raw)
    else {
        return Ok(BlockResult::err(
            block_id.clone(),
            "verify",
            &anyhow!("data snapshot import manifest parse failed"),
        ));
    };
    let key = block_id.key.as_str();
    let mut checked = false;
    for entry in manifest.entries {
        if key != "all" && !entry.source_path.contains(key) {
            continue;
        }
        checked = true;
        let artifact = app_root.join(entry.artifact_path.trim_start_matches('/'));
        if !artifact.is_file() {
            return Ok(BlockResult::err(
                block_id.clone(),
                "verify",
                &anyhow!("DataSource parquet missing: {}", entry.source_path),
            ));
        }
    }
    if !checked && key != "all" {
        return Ok(BlockResult::err(
            block_id.clone(),
            "verify",
            &anyhow!("data source key `{key}` not found in import manifest"),
        ));
    }
    Ok(BlockResult::ok(block_id.clone(), "verify"))
}

fn verify_mrg_layer(
    source_root: &Path,
    app_id: &str,
    alerts: &mut Vec<LayerVerifyAlert>,
) -> Result<()> {
    let entries = list_navigation_entries(source_root, app_id);
    for required in ["default_access", "default_build"] {
        if !entries.iter().any(|entry| entry.key == required) {
            alerts.push(LayerVerifyAlert {
                layer: "L4".to_string(),
                block_id: format!("navigation:{required}"),
                message: format!("navigation missing `{required}`"),
            });
        }
    }
    let app_root = resolve_app_root(source_root, app_id);
    let mrg = load_mrg_registry(source_root, app_id);
    for slot in &mrg.slots {
        if slot.state == MaterialState::Failed {
            alerts.push(LayerVerifyAlert {
                layer: "L4".to_string(),
                block_id: format!(
                    "material_slot:{}@{}",
                    slot.slot_id.node.key, slot.slot_id.scope_key
                ),
                message: format!("Failed slot owner={}", slot.owner_resource_id),
            });
        }
        if slot.state == MaterialState::Ready {
            if let Some(pref) = slot.payload_ref.as_ref() {
                if resolve_payload_ref(app_root.as_path(), pref).is_none() {
                    alerts.push(LayerVerifyAlert {
                        layer: "L4".to_string(),
                        block_id: format!(
                            "material_slot:{}@{}",
                            slot.slot_id.node.key, slot.slot_id.scope_key
                        ),
                        message: format!(
                            "Ready slot CAS missing hash={}",
                            pref.content_hash
                        ),
                    });
                }
            }
        }
    }
    Ok(())
}
