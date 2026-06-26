//! Graph registry integration helpers.

use std::collections::BTreeMap;
use std::path::Path;

use mei_lang_kernel::{resolve_app_root, CompiledApp, CompileOptions};

use crate::graph::dedup::load_mcg_bundle_revisions;
use crate::graph::feature::{graph_registry_dedup_enabled, graph_registry_enabled};
use crate::graph::mcg::assemble::assemble_scope_view;
use crate::graph::mcg::metric_def_bundle::{
    load_metric_def_bundle, DatasetRuntimePayloadView, MetricDefBundleArtifact,
};
use crate::graph::mcg::panel_contract::{load_panel_contracts_from_store, partial_assemble_panel_merge};
use crate::graph::mcg::registry::McgRegistryWriter;
use crate::graph::mcg::app_skeleton::{load_app_skeleton_artifact, merge_app_skeleton_into_compiled};
use crate::graph::mcg::scene_payload::load_scene_payload_artifact;
use crate::graph::mcg::update::update_mcg_after_compile;
use crate::graph::types::GraphNodeKind;

pub fn maybe_update_graph_after_compile(
    source_root: &Path,
    app_id: &str,
    options: &CompileOptions,
    compiled: &CompiledApp,
    compile_revision: &str,
    dataset_runtime_payloads: &BTreeMap<String, DatasetRuntimePayloadView>,
) {
    if !graph_registry_dedup_enabled() {
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

fn hydrate_metric_defs_from_mcg_cas(
    app_root: &Path,
    mcg: &crate::graph::mcg::registry::McgRegistry,
    compiled: &mut CompiledApp,
) {
    for resource in &mut compiled.resources {
        let Some(dataset) = resource.dataset.as_mut() else {
            continue;
        };
        if !dataset.runtime_metric_defs.is_empty() {
            continue;
        }
        let node = mcg.nodes.iter().find(|node| {
            node.id.kind == GraphNodeKind::MetricDefBundle && node.id.key == resource.id
        });
        let Some(hash) = node
            .and_then(|node| node.payload_ref.as_ref())
            .map(|payload| payload.content_hash.as_str())
            .filter(|hash| !hash.is_empty())
        else {
            continue;
        };
        let Ok(Some(bundle)) = load_metric_def_bundle(app_root, hash) else {
            continue;
        };
        apply_metric_def_bundle_to_resource(resource.id.as_str(), dataset, &bundle);
    }
}

fn apply_metric_def_bundle_to_resource(
    owner_id: &str,
    dataset: &mut mei_lang_kernel::DatasetView,
    bundle: &MetricDefBundleArtifact,
) {
    if bundle.owner_resource_id != owner_id {
        return;
    }
    if dataset.runtime_metric_defs.is_empty() {
        dataset.runtime_metric_defs = bundle.runtime_metric_defs.clone();
    }
}

fn merge_compiled_runtime_catalog(into: &mut CompiledApp, donor: &CompiledApp) {
    for resource in &donor.resources {
        if into.resources.iter().any(|existing| existing.id == resource.id) {
            continue;
        }
        into.resources.push(resource.clone());
    }
    for (key, entry) in &donor.world_metrics {
        into.world_metrics
            .entry(key.clone())
            .or_insert_with(|| entry.clone());
    }
    for (key, value) in &donor.world_semantic_by_file {
        into.world_semantic_by_file
            .entry(key.clone())
            .or_insert_with(|| value.clone());
    }
}

fn board_catalog_fallback_targets(board_target: &str) -> Vec<String> {
    let board_target = board_target.trim();
    if !board_target.ends_with(".board.mei") {
        return Vec::new();
    }
    let Some(stem) = board_target.strip_suffix(".board.mei") else {
        return Vec::new();
    };
    vec![
        mei_lang_kernel::canonical_app_source_rel_path(&format!("{stem}.mei")),
        mei_lang_kernel::canonical_app_source_rel_path(&format!("{stem}.world.mei")),
        mei_lang_kernel::canonical_app_source_rel_path("scenes/home.mei"),
    ]
}

/// Board overlay payloads may carry bindings only; backfill datasets/metrics from sibling capsules.
fn backfill_assembled_runtime_catalog(app_root: &Path, target: &str, compiled: &mut CompiledApp) {
    let needs_resources = compiled.resources.is_empty();
    let needs_world_metrics = compiled.world_metrics.is_empty();
    if !needs_resources && !needs_world_metrics {
        return;
    }
    let mut fallback_targets = board_catalog_fallback_targets(target);
    if fallback_targets.is_empty() && (needs_resources || needs_world_metrics) {
        fallback_targets.push(mei_lang_kernel::canonical_app_source_rel_path("scenes/home.mei"));
    }
    for fallback_target in fallback_targets {
        let Some(artifact) = load_scene_payload_artifact(app_root, fallback_target.as_str(), None, None)
            .ok()
            .flatten()
        else {
            continue;
        };
        let Ok(donor) = serde_json::from_value::<CompiledApp>(artifact.payload) else {
            continue;
        };
        merge_compiled_runtime_catalog(compiled, &donor);
        if !compiled.resources.is_empty() && !compiled.world_metrics.is_empty() {
            return;
        }
    }
}

/// Restore `world_metrics` ledger from scene payload when slim artifacts stripped it on write.
pub fn hydrate_world_metrics_from_scene_payload(
    source_root: &Path,
    app_id: &str,
    target_file: &str,
    compiled: &mut CompiledApp,
) -> bool {
    if !compiled.world_metrics.is_empty() {
        return true;
    }
    let target = target_file.trim();
    if target.is_empty() {
        return false;
    }
    let app_root = resolve_app_root(source_root, app_id);
    let Some(artifact) = load_scene_payload_artifact(app_root.as_path(), target, None, None)
        .ok()
        .flatten()
    else {
        return false;
    };
    let Ok(scene_compiled) = serde_json::from_value::<CompiledApp>(artifact.payload) else {
        return false;
    };
    if scene_compiled.world_metrics.is_empty() {
        return false;
    }
    compiled.world_metrics = scene_compiled.world_metrics;
    true
}

/// Assemble-only path: load ScenePayload from disk and project scope without Starlark re-run.
pub fn try_assemble_scope_from_scene_payload(
    source_root: &Path,
    app_id: &str,
    active_scene: Option<&str>,
    active_target: &str,
) -> Option<(CompiledApp, String)> {
    if !graph_registry_dedup_enabled() {
        return None;
    }
    let target = active_target.trim();
    if target.is_empty() {
        return None;
    }
    let lookup_keys = mei_lang_kernel::app_source_rel_path_lookup_keys(target);
    let mcg = McgRegistryWriter::load(source_root, app_id);
    // MCG registry may be absent on disk-only prebuild; still load scene payload when the file exists.
    let expected_revision = lookup_keys
        .iter()
        .find_map(|key| mcg.node_revision("scene_payload", key.as_str()));
    let compile_revision = mcg
        .nodes
        .iter()
        .filter(|node| node.id.kind == GraphNodeKind::AssemblyView)
        .find_map(|node| {
            node.payload_ref
                .as_ref()
                .and_then(|payload| Some(payload.content_hash.clone()))
        })
        .unwrap_or_default();
    let (scene_node, resolved_target) = lookup_keys.iter().find_map(|key| {
        mcg.nodes.iter().find(|node| {
            node.id.kind == GraphNodeKind::ScenePayload && node.id.key == *key
        }).map(|node| (node, key.clone()))
    })?;
    let content_hash = scene_node
        .payload_ref
        .as_ref()
        .map(|payload| payload.content_hash.as_str());
    let app_root = resolve_app_root(source_root, app_id);
    let artifact = load_scene_payload_artifact(
        app_root.as_path(),
        resolved_target.as_str(),
        expected_revision.as_deref(),
        content_hash,
    )
    .ok()
    .flatten()?;
    let mut compiled: CompiledApp = serde_json::from_value(artifact.payload).ok()?;
    if let Some(sk_node) = mcg.nodes.iter().find(|node| node.id.kind == GraphNodeKind::AppSkeleton) {
        if let Some(hash) = sk_node
            .payload_ref
            .as_ref()
            .map(|payload| payload.content_hash.as_str())
            .filter(|hash| !hash.is_empty())
        {
            if let Ok(Some(skeleton)) = load_app_skeleton_artifact(app_root.as_path(), hash) {
                merge_app_skeleton_into_compiled(&mut compiled, &skeleton);
            }
        }
    }
    backfill_assembled_runtime_catalog(app_root.as_path(), resolved_target.as_str(), &mut compiled);
    hydrate_world_metrics_from_scene_payload(source_root, app_id, resolved_target.as_str(), &mut compiled);
    hydrate_metric_defs_from_mcg_cas(app_root.as_path(), &mcg, &mut compiled);
    if let Ok(changed_panels) = load_panel_contracts_from_store(app_root.as_path(), &mcg) {
        if !changed_panels.is_empty() {
            compiled = partial_assemble_panel_merge(&compiled, &changed_panels);
        }
    }
    if !crate::graph::mcg::scene_payload::scene_payload_is_assemblable(&compiled) {
        return None;
    }
    Some((
        assemble_scope_view(compiled, active_scene, Some(resolved_target.as_str())),
        compile_revision,
    ))
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
    load_mcg_bundle_revisions(source_root, app_id)
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
    workset_id: &str,
    scope_key: &str,
    owner_resource_id: &str,
    bundle_revision: &str,
    data_source_revision: &str,
    response_cache_key: &str,
    artifact_relative_path: &str,
    wall_ms: u64,
) {
    if let Err(error) = crate::graph::mrg::slots::record_mrg_slot_after_eval(
        source_root,
        app_id,
        workset_id,
        scope_key,
        owner_resource_id,
        bundle_revision,
        data_source_revision,
        response_cache_key,
        artifact_relative_path,
        wall_ms,
        false,
    ) {
        tracing::warn!(
            app_id = %app_id,
            workset_id = %workset_id,
            error = %error,
            "failed to record MRG slot after prebuild"
        );
    }
}

pub fn record_prebuild_dataframe_slot(
    source_root: &Path,
    app_id: &str,
    workset_id: &str,
    scope_key: &str,
    owner_resource_id: &str,
    bundle_revision: &str,
    data_source_revision: &str,
    shared_artifact_key: &str,
    artifact_relative_path: &str,
    wall_ms: u64,
) {
    if let Err(error) = crate::graph::mrg::slots::record_mrg_dataframe_slot_after_eval(
        source_root,
        app_id,
        workset_id,
        scope_key,
        owner_resource_id,
        bundle_revision,
        data_source_revision,
        shared_artifact_key,
        artifact_relative_path,
        wall_ms,
        false,
    ) {
        tracing::warn!(
            app_id = %app_id,
            workset_id = %workset_id,
            error = %error,
            "failed to record MRG dataframe slot after prebuild"
        );
    }
}

pub fn record_prebuild_slot_failed(
    source_root: &Path,
    app_id: &str,
    workset_id: &str,
    scope_key: &str,
    owner_resource_id: &str,
    bundle_revision: &str,
    data_source_revision: &str,
    error_message: &str,
) {
    if let Err(error) = crate::graph::mrg::slots::record_mrg_slot_failed(
        source_root,
        app_id,
        workset_id,
        scope_key,
        owner_resource_id,
        bundle_revision,
        data_source_revision,
        error_message,
    ) {
        tracing::warn!(
            app_id = %app_id,
            workset_id = %workset_id,
            error = %error,
            "failed to record MRG slot failure after prebuild"
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
