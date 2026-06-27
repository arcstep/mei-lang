//! Graph registry integration helpers.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::path::Path;

use mei_lang_kernel::{resolve_app_root, CompiledApp, CompileOptions, DatasetView, LoadedResource, SourceDecl};

use crate::graph::dedup::load_mcg_bundle_revisions;
use crate::graph::feature::{graph_registry_dedup_enabled, graph_registry_enabled};
use crate::graph::mcg::assemble::assemble_scope_view;
use crate::graph::mcg::metric_def_bundle::{
    load_metric_def_bundle, DatasetRuntimePayloadView, MetricDefBundleArtifact,
};
use crate::graph::mcg::panel_contract::{load_panel_contracts_from_store, partial_assemble_panel_merge};
use crate::graph::mcg::registry::McgRegistryWriter;
use crate::graph::mcg::app_skeleton::{load_app_skeleton_artifact, merge_app_skeleton_into_compiled};
use crate::graph::content_store::{self, SCENE_PAYLOAD};
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

fn world_metrics_dataset_from_bundle(
    owner_id: &str,
    bundle: &MetricDefBundleArtifact,
) -> DatasetView {
    DatasetView {
        id: owner_id.to_string(),
        title: None,
        purpose: None,
        schema: Vec::new(),
        stage_schema: Vec::new(),
        columns: Vec::new(),
        rows: Vec::new(),
        source: SourceDecl {
            kind: "world_metrics".to_string(),
            path: String::new(),
            sheet: None,
            header_row: None,
            preview_rows: None,
            page_size: None,
            max_page_size: None,
            table: None,
            query: None,
            connection: None,
            content: None,
        },
        sources: Vec::new(),
        metrics: BTreeMap::new(),
        runtime_metric_defs: bundle.runtime_metric_defs.clone(),
        runtime_analysis_graph: Default::default(),
        runtime_analysis_contracts: Default::default(),
    }
}

/// Home/board assemble loads a slim scene payload that may omit embedded capsule
/// `__world_metrics__::{capsule}::metrics` owners; restore them from MCG MetricDefBundle CAS.
fn hydrate_imported_world_metrics_resources_from_mcg(
    app_root: &Path,
    mcg: &crate::graph::mcg::registry::McgRegistry,
    compiled: &mut CompiledApp,
) {
    for node in &mcg.nodes {
        if node.id.kind != GraphNodeKind::MetricDefBundle {
            continue;
        }
        let owner_id = node.id.key.trim();
        if !owner_id.starts_with("__world_metrics__::") {
            continue;
        }
        if let Some(existing) = compiled
            .resources
            .iter_mut()
            .find(|resource| resource.id == owner_id)
        {
            if existing
                .dataset
                .as_ref()
                .is_some_and(|dataset| dataset.has_runtime_metric_defs())
            {
                continue;
            }
            let Some(hash) = node
                .payload_ref
                .as_ref()
                .map(|payload| payload.content_hash.as_str())
                .filter(|hash| !hash.is_empty())
            else {
                continue;
            };
            let Ok(Some(bundle)) = load_metric_def_bundle(app_root, hash) else {
                continue;
            };
            if bundle.owner_resource_id != owner_id || bundle.runtime_metric_defs.is_empty() {
                continue;
            }
            if existing.dataset.is_none() {
                existing.dataset = Some(world_metrics_dataset_from_bundle(owner_id, &bundle));
            } else if let Some(dataset) = existing.dataset.as_mut() {
                apply_metric_def_bundle_to_resource(owner_id, dataset, &bundle);
            }
            continue;
        }
        let Some(hash) = node
            .payload_ref
            .as_ref()
            .map(|payload| payload.content_hash.as_str())
            .filter(|hash| !hash.is_empty())
        else {
            continue;
        };
        let Ok(Some(bundle)) = load_metric_def_bundle(app_root, hash) else {
            continue;
        };
        if bundle.owner_resource_id != owner_id || bundle.runtime_metric_defs.is_empty() {
            continue;
        }
        compiled.resources.push(LoadedResource {
            id: owner_id.to_string(),
            kind: "dataset".to_string(),
            title: None,
            document: None,
            dataset: Some(world_metrics_dataset_from_bundle(owner_id, &bundle)),
        });
    }
}

fn embedded_capsule_targets(
    compiled: &CompiledApp,
    mcg: &crate::graph::mcg::registry::McgRegistry,
) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for path in compiled.scene_projection_assembly_by_id.keys() {
        let canonical = mei_lang_kernel::canonical_app_source_rel_path(path.as_str());
        if canonical.starts_with("scenes/")
            && canonical.ends_with(".mei")
            && !canonical.ends_with("home.mei")
        {
            out.insert(canonical);
        }
    }
    for resource in &compiled.resources {
        if let Some(capsule) =
            mei_lang_kernel::imported_capsule_path_from_world_metrics_resource_id(resource.id.as_str())
        {
            out.insert(mei_lang_kernel::canonical_app_source_rel_path(capsule.as_str()));
        }
    }
    for node in &mcg.nodes {
        if node.id.kind != GraphNodeKind::MetricDefBundle {
            continue;
        }
        if let Some(capsule) =
            mei_lang_kernel::imported_capsule_path_from_world_metrics_resource_id(node.id.key.as_str())
        {
            out.insert(mei_lang_kernel::canonical_app_source_rel_path(capsule.as_str()));
        }
    }
    out
}

fn load_scene_payload_compiled_from_mcg(
    app_root: &Path,
    mcg: &crate::graph::mcg::registry::McgRegistry,
    target: &str,
) -> Option<CompiledApp> {
    let lookup_keys = mei_lang_kernel::app_source_rel_path_lookup_keys(target);
    let (scene_node, _resolved) = lookup_keys.iter().find_map(|key| {
        let node = mcg.nodes.iter().find(|node| {
            node.id.kind == GraphNodeKind::ScenePayload && node.id.key == *key
        })?;
        let content_hash = node
            .payload_ref
            .as_ref()
            .map(|payload| payload.content_hash.as_str())
            .unwrap_or("");
        if content_store::get(app_root, SCENE_PAYLOAD, content_hash).is_some() {
            Some((node, key.clone()))
        } else {
            None
        }
    })?;
    let content_hash = scene_node
        .payload_ref
        .as_ref()
        .map(|payload| payload.content_hash.as_str());
    let artifact = load_scene_payload_artifact(
        app_root,
        target,
        Some(scene_node.revision.as_str()),
        content_hash,
    )
    .ok()
    .flatten()?;
    serde_json::from_value::<CompiledApp>(artifact.payload).ok()
}

/// Merge full embedded capsule catalogs (datasets + world_metrics owners) for home assemble.
fn backfill_embedded_capsule_catalog_from_mcg(
    app_root: &Path,
    mcg: &crate::graph::mcg::registry::McgRegistry,
    compiled: &mut CompiledApp,
) {
    for capsule in embedded_capsule_targets(compiled, mcg) {
        let Some(donor) = load_scene_payload_compiled_from_mcg(app_root, mcg, capsule.as_str()) else {
            continue;
        };
        merge_compiled_runtime_catalog(compiled, &donor);
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

fn merge_dataset_view(into: &mut DatasetView, donor: &DatasetView) {
    if into.runtime_metric_defs.is_empty() && !donor.runtime_metric_defs.is_empty() {
        into.runtime_metric_defs = donor.runtime_metric_defs.clone();
    }
    if into.runtime_analysis_contracts.is_empty() && !donor.runtime_analysis_contracts.is_empty() {
        into.runtime_analysis_contracts = donor.runtime_analysis_contracts.clone();
    }
    if into.runtime_analysis_graph.nodes.is_empty()
        && !donor.runtime_analysis_graph.nodes.is_empty()
    {
        into.runtime_analysis_graph = donor.runtime_analysis_graph.clone();
    }
    if into.metrics.is_empty() && !donor.metrics.is_empty() {
        into.metrics = donor.metrics.clone();
    }
    if into.rows.is_empty() && !donor.rows.is_empty() {
        into.rows = donor.rows.clone();
    }
    if into.columns.is_empty() && !donor.columns.is_empty() {
        into.columns = donor.columns.clone();
    }
    if into.schema.is_empty() && !donor.schema.is_empty() {
        into.schema = donor.schema.clone();
    }
    if into.stage_schema.is_empty() && !donor.stage_schema.is_empty() {
        into.stage_schema = donor.stage_schema.clone();
    }
    if into.sources.is_empty() && !donor.sources.is_empty() {
        into.sources = donor.sources.clone();
    }
    if into.source.path.trim().is_empty() && !donor.source.path.trim().is_empty() {
        into.source = donor.source.clone();
    }
}

fn merge_compiled_runtime_catalog(into: &mut CompiledApp, donor: &CompiledApp) {
    for resource in &donor.resources {
        if let Some(existing) = into.resources.iter_mut().find(|existing| existing.id == resource.id)
        {
            match (existing.dataset.as_mut(), resource.dataset.as_ref()) {
                (Some(into_dataset), Some(donor_dataset)) => {
                    merge_dataset_view(into_dataset, donor_dataset);
                }
                (None, Some(donor_dataset)) => {
                    existing.dataset = Some(donor_dataset.clone());
                }
                _ => {}
            }
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

/// Shared hydrate path for prebuild eval and assemble — restores embedded capsule owners/catalog from MCG.
pub fn hydrate_compiled_for_embedded_capsules(
    source_root: &Path,
    app_id: &str,
    compiled: &mut CompiledApp,
) -> anyhow::Result<()> {
    let app_root = resolve_app_root(source_root, app_id);
    let mcg = McgRegistryWriter::load(source_root, app_id);
    hydrate_imported_world_metrics_resources_from_mcg(app_root.as_path(), &mcg, compiled);
    backfill_embedded_capsule_catalog_from_mcg(app_root.as_path(), &mcg, compiled);
    hydrate_metric_defs_from_mcg_cas(app_root.as_path(), &mcg, compiled);
    Ok(())
}

fn capsule_paths_from_metric_ids(metric_ids: &[String]) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for metric_id in metric_ids {
        if let Some(capsule) = metric_id
            .split("::")
            .next()
            .map(str::trim)
            .filter(|segment| segment.starts_with("scenes/") && segment.ends_with(".mei"))
        {
            out.insert(mei_lang_kernel::canonical_app_source_rel_path(capsule));
        }
    }
    out
}

fn hydrate_capsule_scene_payloads_from_mcg(
    app_root: &Path,
    mcg: &crate::graph::mcg::registry::McgRegistry,
    compiled: &mut CompiledApp,
    capsule_paths: &BTreeSet<String>,
) {
    for capsule in capsule_paths {
        let Some(donor) = load_scene_payload_compiled_from_mcg(app_root, mcg, capsule.as_str()) else {
            continue;
        };
        merge_compiled_runtime_catalog(compiled, &donor);
        let world = capsule
            .strip_suffix(".mei")
            .map(|stem| format!("{stem}.world.mei"))
            .map(|path| mei_lang_kernel::canonical_app_source_rel_path(path.as_str()));
        if let Some(world) = world {
            if let Some(world_donor) = load_scene_payload_compiled_from_mcg(app_root, mcg, world.as_str()) {
                merge_compiled_runtime_catalog(compiled, &world_donor);
            }
        }
    }
}

fn capsule_paths_for_prebuild_hydrate(
    metric_ids: &[String],
    owner_resource_ids: &[String],
) -> BTreeSet<String> {
    let mut out = capsule_paths_from_metric_ids(metric_ids);
    for owner in owner_resource_ids {
        if let Some(capsule) =
            mei_lang_kernel::imported_capsule_path_from_world_metrics_resource_id(owner.as_str())
        {
            out.insert(mei_lang_kernel::canonical_app_source_rel_path(capsule.as_str()));
        }
    }
    out
}

/// Prebuild eval SSOT: board backfill + embedded capsule catalog + metric-def CAS hydrate.
pub fn hydrate_compiled_for_prebuild_eval(
    source_root: &Path,
    app_id: &str,
    compiled: &mut CompiledApp,
    metric_ids: &[String],
    owner_resource_ids: &[String],
) -> anyhow::Result<()> {
    let app_root = resolve_app_root(source_root, app_id);
    let target = compiled.active_target_file.trim().to_string();
    if !target.is_empty() {
        backfill_assembled_runtime_catalog(app_root.as_path(), target.as_str(), compiled);
        hydrate_world_metrics_from_scene_payload(source_root, app_id, target.as_str(), compiled);
    }
    hydrate_compiled_for_embedded_capsules(source_root, app_id, compiled)?;
    let capsules = capsule_paths_for_prebuild_hydrate(metric_ids, owner_resource_ids);
    if !capsules.is_empty() {
        let mcg = McgRegistryWriter::load(source_root, app_id);
        hydrate_capsule_scene_payloads_from_mcg(app_root.as_path(), &mcg, compiled, &capsules);
        hydrate_metric_defs_from_mcg_cas(app_root.as_path(), &mcg, compiled);
    }
    Ok(())
}

/// Discover all world-metrics owner resource ids for home/embedded capsule artifact planning.
pub fn discover_world_metrics_owner_ids(
    source_root: &Path,
    app_id: &str,
    compiled: &CompiledApp,
) -> BTreeSet<String> {
    let mut owners = BTreeSet::new();
    for resource in &compiled.resources {
        let id = resource.id.trim();
        if id == "__world_metrics__" || id.starts_with("__world_metrics__::") {
            owners.insert(normalize_world_metrics_owner_id(id));
        }
    }
    let mcg = McgRegistryWriter::load(source_root, app_id);
    for node in &mcg.nodes {
        if node.id.kind != GraphNodeKind::MetricDefBundle {
            continue;
        }
        let key = node.id.key.trim();
        if key == "__world_metrics__" || key.starts_with("__world_metrics__::") {
            owners.insert(normalize_world_metrics_owner_id(key));
        }
    }
    owners
}

/// World-metrics owner ids use capsule paths like `scenes/foo.mei`, not MCG canonical `src/scenes/...`.
fn normalize_world_metrics_owner_id(owner_id: &str) -> String {
    let owner_id = owner_id.trim();
    if owner_id == "__world_metrics__" {
        return owner_id.to_string();
    }
    let Some(inner) = owner_id.strip_prefix("__world_metrics__::") else {
        return owner_id.to_string();
    };
    let Some(path) = inner.strip_suffix("::metrics") else {
        return owner_id.to_string();
    };
    let path = path.strip_prefix("src/").unwrap_or(path);
    if path.is_empty() {
        return owner_id.to_string();
    }
    format!("__world_metrics__::{path}::metrics")
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
    let app_root = resolve_app_root(source_root, app_id);
    let (scene_node, resolved_target) = lookup_keys.iter().find_map(|key| {
        let node = mcg.nodes.iter().find(|node| {
            node.id.kind == GraphNodeKind::ScenePayload && node.id.key == *key
        })?;
        let content_hash = node
            .payload_ref
            .as_ref()
            .map(|payload| payload.content_hash.as_str())
            .unwrap_or("");
        if content_store::get(app_root.as_path(), SCENE_PAYLOAD, content_hash).is_some() {
            Some((node, key.clone()))
        } else {
            None
        }
    })?;
    let content_hash = scene_node
        .payload_ref
        .as_ref()
        .map(|payload| payload.content_hash.as_str());
    let artifact = load_scene_payload_artifact(
        app_root.as_path(),
        resolved_target.as_str(),
        Some(scene_node.revision.as_str()),
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
    hydrate_imported_world_metrics_resources_from_mcg(app_root.as_path(), &mcg, &mut compiled);
    backfill_embedded_capsule_catalog_from_mcg(app_root.as_path(), &mcg, &mut compiled);
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
