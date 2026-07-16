//! Layered graph observability: status / inspect / doctor (CLI ≡ HTTP).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use mei_lang_kernel::{resolve_app_root, resolve_runtime_warmup_manifest};
use serde::Serialize;

use crate::graph::content_store::{content_store_root, resolve_payload_ref};
use crate::graph::mcg::registry::{McgRegistry, McgRegistryWriter, MCG_REGISTRY_SCHEMA_VERSION};
use crate::graph::mcg::scene_payload::load_scene_payload_artifact;
use crate::graph::mrg::navigation_contract;
use crate::graph::mrg::registry::{MrgRegistry, MrgRegistryWriter, MRG_REGISTRY_SCHEMA_VERSION};
use crate::graph::types::MaterialState;
use crate::readiness::scope_gate;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphAppStatus {
    pub app_id: String,
    pub mcg: McgStatusSummary,
    pub mrg: MrgStatusSummary,
    pub bridge: BridgeStatusSummary,
    pub content_store: ContentStoreSummary,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McgStatusSummary {
    pub registry_revision: String,
    pub node_count: usize,
    pub path: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MrgStatusSummary {
    pub registry_revision: String,
    pub navigation_count: usize,
    pub slot_ready: usize,
    pub slot_stale: usize,
    pub slot_failed: usize,
    pub path: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeStatusSummary {
    pub present: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentStoreSummary {
    pub bytes: u64,
    pub files_by_kind: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphStatusReport {
    pub apps: Vec<GraphAppStatus>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphDoctorAlert {
    pub layer: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphDoctorReport {
    pub app_id: String,
    pub ok: bool,
    pub alerts: Vec<GraphDoctorAlert>,
}

pub fn run_graph_status(source_root: &Path, app_filter: Option<&str>) -> GraphStatusReport {
    let app_ids = resolve_app_ids(source_root, app_filter);
    GraphStatusReport {
        apps: app_ids
            .into_iter()
            .map(|app_id| graph_app_status(source_root, app_id.as_str()))
            .collect(),
    }
}

fn graph_app_status(source_root: &Path, app_id: &str) -> GraphAppStatus {
    let app_root = resolve_app_root(source_root, app_id);
    let mcg = McgRegistryWriter::load(source_root, app_id);
    let mrg = MrgRegistryWriter::load(source_root, app_id);
    let bridge = crate::graph::paths::bridge_path(source_root, app_id);
    GraphAppStatus {
        app_id: app_id.to_string(),
        mcg: McgStatusSummary {
            registry_revision: mcg.registry_revision.clone(),
            node_count: mcg.nodes.len(),
            path: crate::graph::paths::mcg_registry_path(source_root, app_id)
                .display()
                .to_string(),
        },
        mrg: MrgStatusSummary {
            registry_revision: mrg.registry_revision.clone(),
            navigation_count: mrg
                .nodes
                .iter()
                .filter(|node| {
                    matches!(
                        node,
                        crate::graph::mrg::nodes::MrgNodeRecord::Navigation { .. }
                    )
                })
                .count(),
            slot_ready: mrg
                .slots
                .iter()
                .filter(|slot| slot.state == MaterialState::Ready)
                .count(),
            slot_stale: mrg
                .slots
                .iter()
                .filter(|slot| slot.state == MaterialState::Stale)
                .count(),
            slot_failed: mrg
                .slots
                .iter()
                .filter(|slot| slot.state == MaterialState::Failed)
                .count(),
            path: crate::graph::paths::mrg_registry_path(source_root, app_id)
                .display()
                .to_string(),
        },
        bridge: BridgeStatusSummary {
            present: bridge.is_file(),
        },
        content_store: scan_content_store_summary(app_root.as_path()),
    }
}

pub fn run_graph_doctor(source_root: &Path, app_id: &str) -> GraphDoctorReport {
    let app_root = resolve_app_root(source_root, app_id);
    let mut alerts = Vec::new();
    let mcg = McgRegistryWriter::load(source_root, app_id);
    let mrg = MrgRegistryWriter::load(source_root, app_id);

    if mcg.schema_version != MCG_REGISTRY_SCHEMA_VERSION {
        alerts.push(GraphDoctorAlert {
            layer: "MCG".to_string(),
            message: format!(
                "registry schema {} != expected {}",
                mcg.schema_version, MCG_REGISTRY_SCHEMA_VERSION
            ),
        });
    }
    if mrg.schema_version != MRG_REGISTRY_SCHEMA_VERSION {
        alerts.push(GraphDoctorAlert {
            layer: "MRG".to_string(),
            message: format!(
                "registry schema {} != expected {}",
                mrg.schema_version, MRG_REGISTRY_SCHEMA_VERSION
            ),
        });
    }

    let canonical_mrg = crate::graph::paths::mrg_registry_path(source_root, app_id);
    let legacy_mrg = crate::graph::paths::legacy_workspace_graph_root(source_root, app_id)
        .join("mrg-registry.json");
    if !canonical_mrg.is_file() && legacy_mrg.is_file() {
        alerts.push(GraphDoctorAlert {
            layer: "MRG".to_string(),
            message: format!(
                "legacy MRG registry present without canonical path: {}",
                legacy_mrg.display()
            ),
        });
    }

    let nav = navigation_contract::verify_navigation_contract(source_root, app_id);
    if !nav.ok {
        if !nav.missing_access_keys.is_empty() {
            alerts.push(GraphDoctorAlert {
                layer: "L2".to_string(),
                message: format!(
                    "navigation missing access keys: {}",
                    nav.missing_access_keys.join(", ")
                ),
            });
        }
        if !nav.duplicate_keys.is_empty() {
            alerts.push(GraphDoctorAlert {
                layer: "L2".to_string(),
                message: format!(
                    "navigation duplicate keys: {}",
                    nav.duplicate_keys.join(", ")
                ),
            });
        }
    }

    for node in &mcg.nodes {
        if let Some(pref) = node.payload_ref.as_ref() {
            if !payload_ref_exists(app_root.as_path(), pref, node.id.key.as_str()) {
                alerts.push(GraphDoctorAlert {
                    layer: "L3".to_string(),
                    message: format!(
                        "MCG payloadRef miss kind={} hash={}.. node={}",
                        pref.kind,
                        &pref.content_hash.chars().take(8).collect::<String>(),
                        node.id.key
                    ),
                });
            }
        }
    }

    for slot in &mrg.slots {
        if slot.state == MaterialState::Ready {
            if let Some(pref) = slot.payload_ref.as_ref() {
                if !payload_ref_exists(app_root.as_path(), pref, slot.slot_id.node.key.as_str()) {
                    alerts.push(GraphDoctorAlert {
                        layer: "L4".to_string(),
                        message: format!(
                            "MRG Ready slot CAS miss key={} hash={}..",
                            slot.slot_id.node.key,
                            &pref.content_hash.chars().take(8).collect::<String>()
                        ),
                    });
                }
            }
        }
        if slot.state == MaterialState::Failed {
            alerts.push(GraphDoctorAlert {
                layer: "L4".to_string(),
                message: format!(
                    "MRG Failed slot key={} owner={}",
                    slot.slot_id.node.key, slot.owner_resource_id
                ),
            });
        }
    }

    let referenced_hashes = collect_registry_content_hashes(&mcg, &mrg);
    let orphan_count = count_orphan_cas_blobs(app_root.as_path(), &referenced_hashes);
    if orphan_count > 0 {
        alerts.push(GraphDoctorAlert {
            layer: "CAS".to_string(),
            message: format!("orphan CAS blobs: {orphan_count}"),
        });
    }

    check_data_source_parquet(source_root, app_id, &mut alerts);

    GraphDoctorReport {
        app_id: app_id.to_string(),
        ok: alerts.is_empty(),
        alerts,
    }
}

fn check_data_source_parquet(source_root: &Path, app_id: &str, alerts: &mut Vec<GraphDoctorAlert>) {
    let app_root = resolve_app_root(source_root, app_id);
    let manifest_path = mei_lang_kernel::data_snapshot_import_manifest_path(app_root.as_path());
    let Ok(raw) = std::fs::read_to_string(&manifest_path) else {
        return;
    };
    let Ok(manifest) = serde_json::from_str::<mei_lang_kernel::DataSnapshotImportManifest>(&raw)
    else {
        return;
    };
    for entry in manifest.entries {
        let artifact = app_root.join(entry.artifact_path.trim_start_matches('/'));
        if !artifact.is_file() {
            alerts.push(GraphDoctorAlert {
                layer: "L4".to_string(),
                message: format!("DataSource snapshot missing: {}", entry.source_path),
            });
        }
    }
}

fn payload_ref_exists(
    app_root: &Path,
    pref: &crate::graph::types::PayloadRef,
    node_key: &str,
) -> bool {
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
        _ => false,
    }
}

fn collect_registry_content_hashes(mcg: &McgRegistry, mrg: &MrgRegistry) -> BTreeSet<String> {
    let mut hashes = BTreeSet::new();
    for node in &mcg.nodes {
        if let Some(pref) = node.payload_ref.as_ref() {
            if !pref.content_hash.trim().is_empty() {
                hashes.insert(format!("{}/{}", pref.kind, pref.content_hash));
            }
        }
    }
    for slot in &mrg.slots {
        if let Some(pref) = slot.payload_ref.as_ref() {
            if !pref.content_hash.trim().is_empty() {
                hashes.insert(format!("{}/{}", pref.kind, pref.content_hash));
            }
        }
    }
    hashes
}

fn count_orphan_cas_blobs(app_root: &Path, referenced: &BTreeSet<String>) -> usize {
    let root = content_store_root(app_root);
    if !root.is_dir() {
        return 0;
    }
    let mut orphans = 0usize;
    let Ok(entries) = std::fs::read_dir(&root) else {
        return 0;
    };
    for kind_entry in entries.flatten() {
        if !kind_entry.path().is_dir() {
            continue;
        }
        let kind = kind_entry.file_name().to_string_lossy().to_string();
        let Ok(files) = std::fs::read_dir(kind_entry.path()) else {
            continue;
        };
        for file in files.flatten() {
            let file_name = file.file_name();
            let Some(name) = file_name.to_str() else {
                continue;
            };
            let name = name.strip_suffix(".json").unwrap_or(name);
            let key = format!("{kind}/{name}");
            if !referenced.contains(key.as_str()) {
                orphans += 1;
            }
        }
    }
    orphans
}

pub fn scan_content_store_summary(app_root: &Path) -> ContentStoreSummary {
    let root = content_store_root(app_root);
    let mut files_by_kind = BTreeMap::new();
    let mut bytes = 0u64;
    if root.is_dir() {
        if let Ok(kinds) = std::fs::read_dir(&root) {
            for kind_entry in kinds.flatten() {
                if !kind_entry.path().is_dir() {
                    continue;
                }
                let kind = kind_entry.file_name().to_string_lossy().to_string();
                let (count, kind_bytes) = dir_stats(&kind_entry.path());
                *files_by_kind.entry(kind).or_insert(0) += count;
                bytes += kind_bytes;
            }
        }
    }
    ContentStoreSummary {
        bytes,
        files_by_kind,
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphInspectReport {
    pub app_id: String,
    pub layer: String,
    pub mcg_nodes: Option<Vec<McgInspectNode>>,
    pub mrg_slots: Option<Vec<MrgInspectSlot>>,
    pub cas: Option<ContentStoreSummary>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McgInspectNode {
    pub kind: String,
    pub key: String,
    pub revision: String,
    pub state: String,
    pub content_hash_prefix: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MrgInspectSlot {
    pub owner: String,
    pub scope_key: String,
    pub state: String,
    pub content_hash_prefix: Option<String>,
}

pub fn run_graph_inspect(
    source_root: &Path,
    app_id: &str,
    layer: &str,
    hash_filter: Option<&str>,
) -> GraphInspectReport {
    let app_root = resolve_app_root(source_root, app_id);
    let mut report = GraphInspectReport {
        app_id: app_id.to_string(),
        layer: layer.to_string(),
        mcg_nodes: None,
        mrg_slots: None,
        cas: None,
    };
    match layer {
        "mcg" | "all" => {
            let mcg = McgRegistryWriter::load(source_root, app_id);
            report.mcg_nodes = Some(
                mcg.nodes
                    .iter()
                    .map(|node| McgInspectNode {
                        kind: format!("{:?}", node.id.kind),
                        key: node.id.key.clone(),
                        revision: node.revision.clone(),
                        state: format!("{:?}", node.state),
                        content_hash_prefix: node
                            .payload_ref
                            .as_ref()
                            .map(|pref| pref.content_hash.chars().take(8).collect()),
                    })
                    .collect(),
            );
        }
        _ => {}
    }
    match layer {
        "mrg" | "all" => {
            let mrg = MrgRegistryWriter::load(source_root, app_id);
            report.mrg_slots = Some(
                mrg.slots
                    .iter()
                    .map(|slot| MrgInspectSlot {
                        owner: slot.owner_resource_id.clone(),
                        scope_key: slot.slot_id.scope_key.clone(),
                        state: format!("{:?}", slot.state),
                        content_hash_prefix: slot
                            .payload_ref
                            .as_ref()
                            .map(|pref| pref.content_hash.chars().take(8).collect()),
                    })
                    .collect(),
            );
        }
        _ => {}
    }
    match layer {
        "cas" | "all" => {
            report.cas = Some(scan_content_store_summary(app_root.as_path()));
            if let Some(hash) = hash_filter {
                let _ = hash;
            }
        }
        _ => {}
    }
    report
}

pub fn run_scope_gate_check(
    source_root: &Path,
    app_id: &str,
    scene_id: Option<&str>,
    target_file: Option<&str>,
) -> scope_gate::ScopeGateReport {
    scope_gate::check_scope_gate(source_root, app_id, scene_id, target_file)
}

fn resolve_app_ids(source_root: &Path, app_filter: Option<&str>) -> Vec<String> {
    if let Some(app_id) = app_filter.map(str::trim).filter(|value| !value.is_empty()) {
        return vec![app_id.to_string()];
    }
    if let Ok(Some(manifest)) = resolve_runtime_warmup_manifest(source_root) {
        return manifest.apps.into_iter().map(|app| app.app_id).collect();
    }
    Vec::new()
}

fn dir_stats(root: &Path) -> (usize, u64) {
    if !root.is_dir() {
        return (0, 0);
    }
    let mut files = 0usize;
    let mut bytes = 0u64;
    walk_dir(root, &mut files, &mut bytes);
    (files, bytes)
}

fn walk_dir(path: &Path, files: &mut usize, bytes: &mut u64) {
    let entries = match std::fs::read_dir(path) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let entry_path = entry.path();
        if entry_path.is_dir() {
            walk_dir(entry_path.as_path(), files, bytes);
        } else if entry_path.is_file() {
            *files += 1;
            if let Ok(meta) = entry.metadata() {
                *bytes += meta.len();
            }
        }
    }
}
