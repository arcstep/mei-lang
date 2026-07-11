use std::path::Path;

use mei_lang_kernel::resolve_app_root;
use mei_lang_toolchain::{
    access_slim_artifacts_enabled, canonical_artifact_persist_enabled, locked_cache_env_overrides,
};

use crate::graph::feature::graph_registry_dedup_enabled;
use crate::graph::mcg::registry::McgRegistryWriter;
use crate::graph::mrg::registry::MrgRegistryWriter;
use crate::graph::observability::scan_content_store_summary;
use crate::graph::types::{GraphNodeKind, MaterialState};

use super::build::collect_build_diagnostics;
use super::report::{
    CacheDiagnosticsSection, ContentStoreDiagnosticsSection, DiskDiagnosticsSection,
    EvalDiagnosticsSection, FailedSlotDiagnostic, MaterializationDiagnosticsReport,
    McgDiagnosticsSection, MrgDiagnosticsSection, PrebuildPlanSection, ScopeGateSweepSection,
};

const DEFAULT_SECTIONS: &[&str] = &[
    "disk",
    "eval",
    "mcg",
    "mrg",
    "cache",
    "build",
    "reachability",
    "content_store",
    "gate_sweep",
];

pub fn collect_materialization_diagnostics(
    source_root: &Path,
    app_id: &str,
    sections: &[String],
    scene_id: Option<&str>,
    target_file: Option<&str>,
) -> MaterializationDiagnosticsReport {
    let app_root = resolve_app_root(source_root, app_id);
    let include_all = sections.is_empty();
    let wants = |name: &str| include_all || sections.iter().any(|section| section == name);

    let mut report = MaterializationDiagnosticsReport {
        app_id: app_id.to_string(),
        sections: if sections.is_empty() {
            DEFAULT_SECTIONS
                .iter()
                .map(|name| (*name).to_string())
                .collect()
        } else {
            sections.to_vec()
        },
        ..Default::default()
    };

    if wants("cache") {
        let mut env_overrides = locked_cache_env_overrides();
        if let Some(entry) = crate::graph::feature::graph_registry_dedup_env_override_detected() {
            env_overrides.push(entry);
        }
        report.cache = CacheDiagnosticsSection {
            access_slim_artifacts: access_slim_artifacts_enabled(),
            canonical_artifact_persist: canonical_artifact_persist_enabled(),
            graph_registry_dedup: graph_registry_dedup_enabled(),
            locked_on: true,
            env_overrides,
        };
    }

    if wants("disk") {
        report.disk = scan_disk(app_root.as_path());
    }

    if wants("eval") {
        report.eval = scan_eval(app_root.as_path());
    }

    if wants("mcg") && graph_registry_dedup_enabled() {
        let mcg = McgRegistryWriter::load(source_root, app_id);
        let app_skeleton_present = mcg
            .nodes
            .iter()
            .any(|node| node.id.kind == GraphNodeKind::AppSkeleton && node.payload_ref.is_some());
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

    if wants("content_store") {
        let summary = scan_content_store_summary(app_root.as_path());
        report.content_store = ContentStoreDiagnosticsSection {
            bytes: summary.bytes,
            files_by_kind: summary.files_by_kind,
            orphan_count: 0,
        };
    }

    if wants("gate_sweep") && graph_registry_dedup_enabled() {
        let gate = crate::graph::run_scope_gate_check(source_root, app_id, None, None);
        report.scope_gate_sweep = ScopeGateSweepSection {
            l2_miss: if gate.navigation_ready { 0 } else { 1 },
            l3_fail: if gate.assembly_ready { 0 } else { 1 },
            l4_stale: if gate.data_ready { 0 } else { 1 },
            degraded_scopes: if gate.access_ready {
                Vec::new()
            } else {
                vec![format!(
                    "{}/{}",
                    gate.scope.scene_id, gate.scope.target_file
                )]
            },
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
        let (navigation_node_count, navigation_duplicate_keys, navigation_orphan_urls) =
            crate::graph::mrg::navigation_contract::navigation_drift_metrics(source_root, app_id);
        let nav_contract =
            crate::graph::mrg::navigation_contract::verify_navigation_contract(source_root, app_id);
        if !nav_contract.ok {
            if !nav_contract.missing_access_keys.is_empty() {
                report.alerts.push(format!(
                    "MRG navigation missing access keys: {}",
                    nav_contract.missing_access_keys.join(", ")
                ));
            }
            if !nav_contract.missing_build_keys.is_empty() {
                report.alerts.push(format!(
                    "MRG navigation missing build keys: {}",
                    nav_contract.missing_build_keys.join(", ")
                ));
            }
            if !nav_contract.duplicate_keys.is_empty() {
                report.alerts.push(format!(
                    "MRG navigation duplicate keys: {}",
                    nav_contract.duplicate_keys.join(", ")
                ));
            }
        }
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
            navigation_node_count: Some(navigation_node_count),
            navigation_duplicate_keys: Some(navigation_duplicate_keys),
            navigation_orphan_urls: Some(navigation_orphan_urls),
        };
        report.failed_slots = mrg
            .slots
            .iter()
            .filter(|slot| slot.state == MaterialState::Failed)
            .map(|slot| FailedSlotDiagnostic {
                key: slot.slot_id.node.key.clone(),
                owner: slot.owner_resource_id.clone(),
                scope_key: slot.slot_id.scope_key.clone(),
                error: "MRG slot Failed".to_string(),
            })
            .collect();
    }

    if wants("build") {
        report.build = collect_build_diagnostics(source_root, app_root.as_path(), app_id);
        report.prebuild_plan = PrebuildPlanSection {
            plan_source: report.build.report_path.as_ref().and_then(|_| None),
            dirty_slot_count: None,
            mrg_eval_skips: report.build.mrg_eval_skips,
        };
        if let Ok(Some(manifest)) = mei_lang_kernel::resolve_runtime_warmup_manifest(source_root) {
            if manifest.apps.iter().any(|app| app.app_id == app_id) {
                let frontier = crate::prebuild::build_mrg_eval_frontier(
                    source_root,
                    app_id,
                    crate::prebuild::PrebuildScopeProfile::HotOnly,
                );
                report.prebuild_plan.plan_source = Some(frontier.plan_source.to_string());
                report.prebuild_plan.dirty_slot_count = Some(frontier.dirty_slot_count);
            }
        }
    }

    if wants("reachability") {
        let reachability = if scene_id.is_some() || target_file.is_some() {
            crate::graph::run_scope_gate_check(source_root, app_id, scene_id, target_file)
        } else {
            crate::readiness::reachability::check_reachability(source_root, None).scope_gate
        };
        report.reachability = serde_json::to_value(reachability).ok();
    }

    if report.mrg.stale_ratio > 0.10 && report.mrg.slot_count > 0 {
        report.alerts.push(format!(
            "MRG stale ratio {:.0}% exceeds 10% gate",
            report.mrg.stale_ratio * 100.0
        ));
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
    let build_root = mei_lang_kernel::resolve_app_build_root(app_root);
    let manifest_dir = build_root.join("manifests/compiled_app");
    let artifact_dir = build_root.join("artifacts/compiled_app");
    let (manifest_count, manifest_bytes) = dir_stats(&manifest_dir);
    let (artifact_count, artifact_bytes) = dir_stats(&artifact_dir);
    let graph_root = build_root.join("graph");
    let scene_payload_root = graph_root.join("payloads/scene");
    let (scene_payload_file_count, scene_payload_bytes) = dir_stats(&scene_payload_root);
    let eval_root = mei_lang_kernel::resolve_app_var_root(app_root).join("eval-results");
    let (eval_artifact_file_count, eval_artifact_bytes) = dir_stats(&eval_root);
    let (_, graph_bytes) = dir_stats(&graph_root);
    let (_, data_snapshots_bytes) = dir_stats(&build_root.join("data-snapshots"));
    let (_, prebuild_bytes) = dir_stats(&build_root.join("prebuild"));
    DiskDiagnosticsSection {
        compiled_app_file_count: manifest_count + artifact_count,
        compiled_app_bytes: manifest_bytes + artifact_bytes,
        scene_payload_file_count,
        scene_payload_bytes,
        eval_artifact_file_count,
        eval_artifact_bytes,
        graph_bytes,
        data_snapshots_bytes,
        prebuild_bytes,
        app_root_bytes: dir_stats(app_root).1,
    }
}

fn scan_eval(app_root: &Path) -> EvalDiagnosticsSection {
    let eval_root = mei_lang_kernel::resolve_app_var_root(app_root).join("eval-results");
    let (eval_total_files, eval_total_bytes) = dir_stats(&eval_root);
    let response_dir = eval_root.join("results").join("metric-response");
    let dataframe_dir = eval_root.join("results").join("metric-dataframe");
    let (metric_response_files, metric_response_bytes) = dir_stats(&response_dir);
    let (metric_dataframe_files, metric_dataframe_bytes) = dir_stats(&dataframe_dir);
    EvalDiagnosticsSection {
        metric_response_files,
        metric_response_bytes,
        metric_dataframe_files,
        metric_dataframe_bytes,
        eval_total_files,
        eval_total_bytes,
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn temp_app_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "mei-diag-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ))
    }

    #[test]
    fn scan_disk_reports_subtree_bytes() {
        let app_root = temp_app_root("disk");
        let build_root = mei_lang_kernel::resolve_app_build_root(&app_root);
        let var_root = mei_lang_kernel::resolve_app_var_root(&app_root);
        fs::create_dir_all(build_root.join("graph/payloads/scene")).expect("mkdir");
        fs::create_dir_all(var_root.join("eval-results/results/metric-response")).expect("mkdir");
        fs::write(build_root.join("graph/payloads/scene/a.json"), "abc").expect("write");
        fs::write(
            var_root.join("eval-results/results/metric-response/r.json"),
            "12345",
        )
        .expect("write");
        let disk = scan_disk(&app_root);
        assert_eq!(disk.scene_payload_file_count, 1);
        assert_eq!(disk.scene_payload_bytes, 3);
        let eval = scan_eval(&app_root);
        assert_eq!(eval.metric_response_files, 1);
        assert_eq!(eval.metric_response_bytes, 5);
        let _ = fs::remove_dir_all(&app_root);
    }
}
