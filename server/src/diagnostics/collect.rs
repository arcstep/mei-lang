use std::path::Path;

use mei_lang_kernel::resolve_app_root;
use mei_lang_toolchain::{
    access_slim_artifacts_enabled, canonical_artifact_persist_enabled,
};

use crate::graph::feature::graph_registry_dedup_enabled;
use crate::graph::mcg::app_skeleton::load_app_skeleton_artifact;
use crate::graph::mcg::registry::McgRegistryWriter;
use crate::graph::mrg::registry::MrgRegistryWriter;
use crate::graph::types::{GraphNodeKind, MaterialState};

use super::report::{
    CacheDiagnosticsSection, DiskDiagnosticsSection, MaterializationDiagnosticsReport,
    McgDiagnosticsSection, MrgDiagnosticsSection,
};

pub fn collect_materialization_diagnostics(
    source_root: &Path,
    app_id: &str,
    sections: &[String],
) -> MaterializationDiagnosticsReport {
    let app_root = resolve_app_root(source_root, app_id);
    let include_all = sections.is_empty();
    let wants = |name: &str| include_all || sections.iter().any(|section| section == name);

    let mut report = MaterializationDiagnosticsReport {
        app_id: app_id.to_string(),
        sections: if sections.is_empty() {
            vec![
                "disk".to_string(),
                "mcg".to_string(),
                "mrg".to_string(),
                "cache".to_string(),
            ]
        } else {
            sections.to_vec()
        },
        ..Default::default()
    };

    if wants("cache") {
        report.cache = CacheDiagnosticsSection {
            access_slim_artifacts: access_slim_artifacts_enabled(),
            canonical_artifact_persist: canonical_artifact_persist_enabled(),
            graph_registry_dedup: graph_registry_dedup_enabled(),
        };
    }

    if wants("disk") {
        report.disk = scan_disk(app_root.as_path());
    }

    if wants("mcg") && graph_registry_dedup_enabled() {
        let mcg = McgRegistryWriter::load(source_root, app_id);
        let app_skeleton_present = load_app_skeleton_artifact(app_root.as_path())
            .ok()
            .flatten()
            .is_some();
        report.mcg = McgDiagnosticsSection {
            node_count: mcg.nodes.len(),
            scene_payload_nodes: mcg
                .nodes
                .iter()
                .filter(|node| node.id.kind == GraphNodeKind::ScenePayload)
                .count(),
            metric_def_bundle_nodes: mcg
                .nodes
                .iter()
                .filter(|node| node.id.kind == GraphNodeKind::MetricDefBundle)
                .count(),
            app_skeleton_present,
            registry_revision: mcg.registry_revision,
        };
    }

    if wants("mrg") && graph_registry_dedup_enabled() {
        let mrg = MrgRegistryWriter::load(source_root, app_id);
        let ready = mrg
            .slots
            .iter()
            .filter(|slot| slot.state == MaterialState::Ready)
            .count();
        let stale = mrg
            .slots
            .iter()
            .filter(|slot| slot.state == MaterialState::Stale)
            .count();
        let failed = mrg
            .slots
            .iter()
            .filter(|slot| slot.state == MaterialState::Failed)
            .count();
        let total = mrg.slots.len();
        report.mrg = MrgDiagnosticsSection {
            slot_count: total,
            ready_slots: ready,
            stale_slots: stale,
            failed_slots: failed,
            stale_ratio: if total == 0 {
                0.0
            } else {
                stale as f64 / total as f64
            },
        };
    }

    if report.mrg.stale_ratio > 0.10 && report.mrg.slot_count > 0 {
        report
            .alerts
            .push(format!("MRG stale ratio {:.0}% exceeds 10% gate", report.mrg.stale_ratio * 100.0));
    }
    if report.disk.compiled_app_file_count > 120 {
        report.alerts.push(format!(
            "compiled_app file count {} exceeds Phase E gate (120)",
            report.disk.compiled_app_file_count
        ));
    }

    report
}

fn scan_disk(app_root: &Path) -> DiskDiagnosticsSection {
    let manifest_dir = app_root.join(".mei/manifests/compiled_app");
    let artifact_dir = app_root.join(".mei/artifacts/compiled_app");
    let (manifest_count, manifest_bytes) = dir_stats(&manifest_dir);
    let (artifact_count, artifact_bytes) = dir_stats(&artifact_dir);
    let scene_payload_root = app_root.join(".mei/graph/payloads/scene");
    let (scene_payload_file_count, _) = dir_stats(&scene_payload_root);
    let eval_root = app_root.join(".mei/eval-artifacts");
    let (eval_artifact_file_count, _) = dir_stats(&eval_root);
    DiskDiagnosticsSection {
        compiled_app_file_count: manifest_count + artifact_count,
        compiled_app_bytes: manifest_bytes + artifact_bytes,
        scene_payload_file_count,
        eval_artifact_file_count,
        app_root_bytes: dir_stats(app_root).1,
    }
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
