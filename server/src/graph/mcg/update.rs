use std::collections::BTreeMap;
use std::path::Path;

use mei_lang_kernel::{resolve_app_root, CompiledApp};
use serde_json::Value;

use crate::graph::bridge::export_bridge;
use crate::graph::feature::graph_registry_dedup_enabled;
use crate::graph::mcg::assemble::{
    assemble_page_instance, page_instance_revision, AssemblyInputRecord, PageInstanceInputs,
};
use crate::graph::content_store::{APP_SKELETON, METRIC_DEF_BUNDLE, SCENE_PAYLOAD, CONTENT_PANEL};
use crate::graph::mcg::metric_def_bundle::{
    extract_metric_def_bundles, persist_metric_def_bundle, DatasetRuntimePayloadView,
    MetricDefBundleRecord, METRIC_DEF_BUNDLE_ARTIFACT_SCHEMA,
};
use crate::graph::mcg::app_skeleton::{app_skeleton_revision, load_app_skeleton_artifact, persist_app_skeleton_artifact};
use crate::graph::mcg::content_panel::{extract_content_panels, partial_assemble_panel_merge, persist_content_panels};
use crate::graph::mcg::registry::{AssemblyInputRef, McgEdgeRecord, McgNodeRecord, McgRegistryWriter};
use crate::graph::mcg::scene_payload::{
    load_scene_payload_artifact, persist_scene_payload_artifact, scene_payload_revision,
};
use crate::graph::mrg::invalidation::{apply_mcg_invalidation, changed_bundle_owners};
use crate::graph::mrg::registry::MrgRegistryWriter;
use crate::graph::types::{GraphNodeId, GraphNodeKind, MaterialState, PayloadRef};

#[derive(Debug, Clone, Default)]
pub struct McgUpdateOutcome {
    pub scene_payload_revision: Option<String>,
    pub page_instance_revision: Option<String>,
    pub bundle_revisions: BTreeMap<String, String>,
    pub bundles_unchanged: Vec<String>,
}

pub fn update_mcg_after_compile(
    source_root: &Path,
    app_id: &str,
    options: &mei_lang_kernel::CompileOptions,
    compiled: &CompiledApp,
    compile_revision: &str,
    dependency_fingerprint: &str,
    dataset_runtime_payloads: &BTreeMap<String, DatasetRuntimePayloadView>,
) -> anyhow::Result<McgUpdateOutcome> {
    if !graph_registry_dedup_enabled() {
        return Ok(McgUpdateOutcome::default());
    }
    let app_root = resolve_app_root(source_root, app_id);
    let target_file = compiled.active_target_file.clone();
    let scene_revision = scene_payload_revision(target_file.as_str(), dependency_fingerprint);
    let mut outcome = McgUpdateOutcome {
        scene_payload_revision: Some(scene_revision.clone()),
        ..Default::default()
    };

    let mut registry = McgRegistryWriter::load(source_root, app_id);
    let previous_bundles = registry
        .nodes
        .iter()
        .filter(|node| node.id.kind == GraphNodeKind::MetricDefBundle)
        .map(|node| (node.id.key.clone(), node.revision.clone()))
        .collect::<BTreeMap<_, _>>();
    let previous_scene_rev = registry.node_revision("scene_payload", target_file.as_str());

    let bundles = extract_metric_def_bundles(compiled, dataset_runtime_payloads);
    let compiled_for_payload = if bundles
        .iter()
        .all(|(owner_id, bundle)| {
            previous_bundles
                .get(owner_id)
                .is_some_and(|prev| prev == &bundle.revision)
        })
        && previous_scene_rev.is_some()
    {
        panel_only_scene_payload_compiled(
            app_root.as_path(),
            target_file.as_str(),
            previous_scene_rev.as_deref(),
            compiled,
        )
    } else {
        compiled.clone()
    };

    if let Ok(persisted) = persist_scene_payload_artifact(
        app_root.as_path(),
        target_file.as_str(),
        scene_revision.as_str(),
        &scene_payload_value(&compiled_for_payload),
    ) {
        let payload_bytes = serde_json::to_string(&scene_payload_value(&compiled_for_payload))
            .map(|text| text.len() as u64)
            .unwrap_or(0);
        registry.upsert_node(McgNodeRecord {
            id: GraphNodeId::new(GraphNodeKind::ScenePayload, target_file.clone()),
            revision: scene_revision.clone(),
            state: MaterialState::Ready,
            layer: "compile".to_string(),
            payload_ref: Some(PayloadRef::new(
                SCENE_PAYLOAD,
                persisted.content_hash,
                super::scene_payload::SCENE_PAYLOAD_ARTIFACT_SCHEMA,
            )),
            deps: Vec::new(),
            defs_fingerprint: None,
            owner_resource_id: None,
            assembly_inputs: Vec::new(),
            stats: Some(
                [("payloadBytes".to_string(), payload_bytes)]
                    .into_iter()
                    .collect(),
            ),
        });
    }
    if super::projection_assembly::is_home_scene_payload_target(target_file.as_str())
        && !compiled.scene_projection_assembly_by_id.is_empty()
    {
        if let Ok(hashes) = super::projection_assembly::persist_projection_assemblies(
            app_root.as_path(),
            target_file.as_str(),
            &compiled.scene_projection_assembly_by_id,
        ) {
            for node in
                super::projection_assembly::projection_assembly_mcg_nodes(target_file.as_str(), &hashes)
            {
                registry.upsert_node(node);
            }
        }
    }

    let sk_rev = app_skeleton_revision(dependency_fingerprint);
    let mut skeleton_compiled = compiled.clone();
    if let Some(existing_node) = registry
        .nodes
        .iter()
        .find(|node| node.id.kind == GraphNodeKind::AppSkeleton)
    {
        if let Some(hash) = existing_node
            .payload_ref
            .as_ref()
            .map(|payload| payload.content_hash.as_str())
            .filter(|hash| !hash.is_empty())
        {
            if let Ok(Some(existing_sk)) =
                load_app_skeleton_artifact(app_root.as_path(), hash)
            {
                let mut donor = CompiledApp {
                    app_id: app_id.to_string(),
                    title: String::new(),
                    app_root: String::new(),
                    active_scene: None,
                    active_target_file: String::new(),
                    scene_routes: Vec::new(),
                    file_tree: Vec::new(),
                    scene_contract: None,
                    scene_local_nav_by_target: Default::default(),
                    scene_bindings_by_id: Default::default(),
                    scene_examples_by_id: Default::default(),
                    scene_projection_assembly_by_id: Default::default(),
                    resources: Vec::new(),
                    world_metrics: Default::default(),
                    world_semantic_by_file: Default::default(),
                    component_assets: Vec::new(),
                    diagnostics: Vec::new(),
                    build_experience_index: Default::default(),
                    build_t2_page_index: Default::default(),
                    build_template_index: Default::default(),
        ui_layout_index: Default::default(),
                };
                super::app_skeleton::merge_app_skeleton_into_compiled(&mut donor, &existing_sk);
                crate::graph::integration::merge_compiled_runtime_catalog(
                    &mut skeleton_compiled,
                    &donor,
                );
            }
        }
    }
    if let Ok(persisted) =
        persist_app_skeleton_artifact(app_root.as_path(), sk_rev.as_str(), &skeleton_compiled)
    {
        registry.upsert_node(McgNodeRecord {
            id: GraphNodeId::new(GraphNodeKind::AppSkeleton, app_id.to_string()),
            revision: sk_rev.clone(),
            state: MaterialState::Ready,
            layer: "compile".to_string(),
            payload_ref: Some(PayloadRef::new(
                APP_SKELETON,
                persisted.content_hash,
                super::app_skeleton::APP_SKELETON_ARTIFACT_SCHEMA,
            )),
            deps: Vec::new(),
            defs_fingerprint: None,
            owner_resource_id: None,
            assembly_inputs: Vec::new(),
            stats: None,
        });
    }

    let mut bundle_inputs = Vec::new();
    for (owner_id, bundle) in &bundles {
        if previous_bundles
            .get(owner_id)
            .is_some_and(|prev| prev == &bundle.revision)
        {
            outcome.bundles_unchanged.push(owner_id.clone());
        }
        outcome
            .bundle_revisions
            .insert(owner_id.clone(), bundle.revision.clone());
        bundle_inputs.push(AssemblyInputRecord {
            kind: "metric_def_bundle".to_string(),
            key: owner_id.clone(),
            revision: bundle.revision.clone(),
        });
        registry.upsert_node(bundle_node_record(app_root.as_path(), bundle));
        registry.edges.push(McgEdgeRecord {
            from: format!("metric_def_bundle:{owner_id}"),
            to: format!("mrg:eval_plan:{owner_id}"),
            kind: "exports_to_mrg".to_string(),
        });
    }

    let mut content_panels = extract_content_panels(compiled);
    let mut panel_persisted = BTreeMap::new();
    if graph_registry_dedup_enabled() {
        if let Ok(paths) = persist_content_panels(app_root.as_path(), content_panels.as_slice()) {
            for panel in &mut content_panels {
                if let Some(persisted) = paths.get(&panel.panel_key) {
                    panel_persisted.insert(panel.panel_key.clone(), persisted.content_hash.clone());
                }
            }
        }
    }
    for panel in &content_panels {
        registry.upsert_node(McgNodeRecord {
            id: GraphNodeId::new(GraphNodeKind::ContentPanel, panel.panel_key.clone()),
            revision: panel.revision.clone(),
            state: MaterialState::Ready,
            layer: "assembly".to_string(),
            payload_ref: panel_persisted.get(&panel.panel_key).map(|hash| {
                PayloadRef::new(
                    CONTENT_PANEL,
                    hash.clone(),
                    "mei-panel-contract-artifact-v1",
                )
            }),
            deps: vec![format!("scene_payload:{target_file}")],
            defs_fingerprint: None,
            owner_resource_id: None,
            assembly_inputs: Vec::new(),
            stats: None,
        });
    }
    let panel_inputs = content_panels
        .iter()
        .map(|panel| AssemblyInputRecord {
            kind: "content_panel".to_string(),
            key: panel.panel_key.clone(),
            revision: panel.revision.clone(),
        })
        .collect::<Vec<_>>();

    let scene_input = AssemblyInputRecord {
        kind: "scene_payload".to_string(),
        key: target_file.clone(),
        revision: scene_revision.clone(),
    };
    let (_, assembly_inputs) = assemble_page_instance(
        compiled.clone(),
        PageInstanceInputs {
            scene_payload: Some(scene_input),
            metric_def_bundles: bundle_inputs,
            content_panels: panel_inputs,
        },
    );
    let av_revision = page_instance_revision(&assembly_inputs);
    outcome.page_instance_revision = Some(av_revision.clone());

    registry.upsert_node(McgNodeRecord {
        id: GraphNodeId::new(
            GraphNodeKind::PageInstance,
            page_instance_key(options, compile_revision),
        ),
        revision: av_revision.clone(),
        state: MaterialState::Ready,
        layer: "assembly".to_string(),
        payload_ref: Some(PayloadRef::new(
            "page_instance",
            av_revision,
            "mei-assembly-view-v1",
        )),
        deps: vec![format!("scene_payload:{target_file}")],
        defs_fingerprint: None,
        owner_resource_id: None,
        assembly_inputs: assembly_inputs
            .iter()
            .map(|input| AssemblyInputRef {
                kind: input.kind.clone(),
                key: input.key.clone(),
                revision: input.revision.clone(),
            })
            .collect(),
        stats: None,
    });

    registry.finalize();
    McgRegistryWriter::save(source_root, &registry)?;

    let bridge = export_bridge(app_id, &bundles);
    crate::graph::bridge::BridgeWriter::save(source_root, &bridge)?;

    let scene_changed = previous_scene_rev.as_deref() != Some(scene_revision.as_str());
    let changed_owners = changed_bundle_owners(&previous_bundles, &outcome.bundle_revisions);
    let scene_only = scene_changed && changed_owners.is_empty();
    let mut mrg = MrgRegistryWriter::load(source_root, app_id);
    let invalidation = apply_mcg_invalidation(&mut mrg, &bridge, scene_only, changed_owners.as_slice());
    if invalidation.scene_only_skip {
        tracing::debug!(app_id = %app_id, "MRG invalidation skipped (scene-only bump)");
    }
    mrg.finalize();
    MrgRegistryWriter::save(source_root, &mrg)?;

    let routes = compiled
        .scene_routes
        .iter()
        .map(|route| (route.scene_id.clone(), route.target_file.clone()))
        .collect::<Vec<_>>();
    if let Err(error) =
        crate::graph::mrg::navigation::sync_navigation_registry(source_root, app_id, &routes)
    {
        tracing::warn!(app_id = %app_id, error = %error, "failed to sync MRG navigation registry");
    }

    Ok(outcome)
}

fn bundle_node_record(app_root: &Path, bundle: &MetricDefBundleRecord) -> McgNodeRecord {
    let content_hash = persist_metric_def_bundle(app_root, bundle).unwrap_or_else(|error| {
        tracing::warn!(
            owner = %bundle.owner_resource_id,
            error = %error,
            "failed to persist metric def bundle to content store"
        );
        String::new()
    });
    McgNodeRecord {
        id: GraphNodeId::new(
            GraphNodeKind::MetricDefBundle,
            bundle.owner_resource_id.clone(),
        ),
        revision: bundle.revision.clone(),
        state: MaterialState::Ready,
        layer: "eval_export".to_string(),
        payload_ref: if content_hash.is_empty() {
            None
        } else {
            Some(PayloadRef::new(
                METRIC_DEF_BUNDLE,
                content_hash,
                METRIC_DEF_BUNDLE_ARTIFACT_SCHEMA,
            ))
        },
        deps: Vec::new(),
        defs_fingerprint: Some(bundle.defs_fingerprint.clone()),
        owner_resource_id: Some(bundle.owner_resource_id.clone()),
        assembly_inputs: Vec::new(),
        stats: None,
    }
}

fn page_instance_key(options: &mei_lang_kernel::CompileOptions, compile_revision: &str) -> String {
    let scene = options
        .scene
        .as_deref()
        .unwrap_or("default")
        .trim()
        .to_string();
    let target = options
        .preview_target
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("")
        .to_string();
    if target.is_empty() {
        format!("{scene}@{compile_revision}")
    } else {
        format!("{scene}:{target}@{compile_revision}")
    }
}

fn scene_payload_value(compiled: &CompiledApp) -> Value {
    super::scene_payload::scene_payload_value_for_persist(compiled)
}

fn panel_only_scene_payload_compiled(
    app_root: &Path,
    target_file: &str,
    previous_scene_rev: Option<&str>,
    compiled: &CompiledApp,
) -> CompiledApp {
    let Some(previous_scene_rev) = previous_scene_rev else {
        return compiled.clone();
    };
    let Ok(Some(previous)) =
        load_scene_payload_artifact(app_root, target_file, Some(previous_scene_rev), None)
    else {
        return compiled.clone();
    };
    let app_root_str = app_root.display().to_string();
    let Some(base) = super::scene_payload::compiled_from_scene_payload_artifact(
        &previous,
        None,
        compiled.app_id.as_str(),
        app_root_str.as_str(),
    ) else {
        return compiled.clone();
    };
    let scene_id = compiled
        .active_scene
        .as_deref()
        .unwrap_or("default")
        .to_string();
    let Some(contract) = compiled.scene_contract.as_ref() else {
        return compiled.clone();
    };
    let mut changed_panels = BTreeMap::new();
    for panel in &contract.panels {
        let key = format!("{scene_id}:{}", panel.id);
        if let Ok(value) = serde_json::to_value(panel) {
            changed_panels.insert(key, value);
        }
    }
    if changed_panels.is_empty() {
        compiled.clone()
    } else {
        partial_assemble_panel_merge(&base, &changed_panels)
    }
}
