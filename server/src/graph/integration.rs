//! Graph registry integration helpers.

use std::collections::BTreeMap;
use std::path::Path;

use mei_lang_kernel::{CompiledApp, CompileOptions};

use crate::graph::feature::graph_registry_enabled;
use crate::graph::mcg::metric_def_bundle::DatasetRuntimePayloadView;
use crate::graph::mcg::registry::McgRegistryWriter;
use crate::graph::mcg::update::update_mcg_after_compile;

pub fn maybe_update_graph_after_compile(
    source_root: &Path,
    app_id: &str,
    options: &CompileOptions,
    compiled: &CompiledApp,
    compile_revision: &str,
    dataset_runtime_payloads: &BTreeMap<String, DatasetRuntimePayloadView>,
) {
    if !graph_registry_enabled() {
        return;
    }
    let dependency_fingerprint = compile_revision.to_string();
    match update_mcg_after_compile(
        source_root,
        app_id,
        options,
        compiled,
        compile_revision,
        dependency_fingerprint.as_str(),
        dataset_runtime_payloads,
    ) {
        Ok(outcome) => {
            if let Some(rev) = outcome.scene_payload_revision.as_deref() {
                tracing::debug!(
                    app_id = %app_id,
                    scene_payload_revision = %rev,
                    "MCG scene payload updated"
                );
            }
        }
        Err(error) => {
            tracing::warn!(
                app_id = %app_id,
                error = %error,
                "failed to update MCG registry after compile"
            );
        }
    }
}

pub fn runtime_payloads_from_compiled(compiled: &CompiledApp) -> BTreeMap<String, DatasetRuntimePayloadView> {
    let mut payloads = BTreeMap::new();
    for resource in &compiled.resources {
        let Some(dataset) = resource.dataset.as_ref() else {
            continue;
        };
        if dataset.runtime_metric_defs.is_empty() {
            continue;
        }
        payloads.insert(
            resource.id.clone(),
            DatasetRuntimePayloadView {
                runtime_metric_defs: dataset.runtime_metric_defs.clone(),
            },
        );
    }
    payloads
}

/// Skip scope_artifacts eval for owners whose MetricDefBundle revision is unchanged.
pub fn bundle_unchanged_owners(source_root: &Path, app_id: &str) -> BTreeMap<String, String> {
    if !graph_registry_enabled() {
        return BTreeMap::new();
    }
    McgRegistryWriter::load(source_root, app_id)
        .nodes
        .into_iter()
        .filter(|node| node.id.kind == crate::graph::types::GraphNodeKind::MetricDefBundle)
        .map(|node| (node.id.key, node.revision))
        .collect()
}

pub fn app_graph_fingerprint(source_root: &Path, app_id: &str) -> String {
    if !graph_registry_enabled() {
        return String::new();
    }
    let mcg = McgRegistryWriter::load(source_root, app_id);
    format!("mcg={}", mcg.registry_revision)
}

pub fn record_prebuild_slot(
    source_root: &Path,
    app_id: &str,
    metric_id: &str,
    owner_resource_id: &str,
    bundle_revision: &str,
    data_source_revision: &str,
    response_cache_key: &str,
    wall_ms: u64,
) {
    if let Err(error) = crate::graph::mrg::slots::record_mrg_slot_after_eval(
        source_root,
        app_id,
        metric_id,
        "default",
        owner_resource_id,
        bundle_revision,
        data_source_revision,
        response_cache_key,
        ".mei/eval-artifacts/results/metric-response/",
        wall_ms,
        false,
    ) {
        tracing::warn!(
            app_id = %app_id,
            metric_id = %metric_id,
            error = %error,
            "failed to record MRG slot after prebuild"
        );
    }
}

pub fn schedule_warmup_frontier(source_root: &Path, app_id: &str, scene_id: &str) {
    if !graph_registry_enabled() {
        return;
    }
    let mut mrg = crate::graph::mrg::registry::MrgRegistryWriter::load(source_root, app_id);
    let navigation_edges_added =
        crate::graph::mrg::warmup::record_navigation_edge(&mut mrg, "default", scene_id);
    let mut outcome = crate::graph::mrg::warmup::warm_frontier_slots(&mrg, scene_id, 1);
    outcome.navigation_edges_added = navigation_edges_added;
    if !outcome.scheduled_slots.is_empty() || outcome.navigation_edges_added > 0 {
        tracing::debug!(
            app_id = %app_id,
            scene_id = %scene_id,
            scheduled = outcome.scheduled_slots.len(),
            navigation_edges = outcome.navigation_edges_added,
            "MRG warmup frontier scheduled"
        );
    }
    mrg.finalize();
    let _ = crate::graph::mrg::registry::MrgRegistryWriter::save(source_root, &mrg);
}
