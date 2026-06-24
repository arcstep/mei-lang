use std::collections::BTreeMap;
use std::path::Path;

use mei_lang_kernel::{resolve_app_root, CompiledApp};
use serde_json::Value;

use crate::graph::bridge::export_bridge;
use crate::graph::feature::graph_registry_enabled;
use crate::graph::mcg::assemble::{
    assemble_assembly_view, assembly_view_revision, AssemblyInputRecord, AssemblyViewInputs,
};
use crate::graph::mcg::metric_def_bundle::{
    extract_metric_def_bundles, DatasetRuntimePayloadView, MetricDefBundleRecord,
};
use crate::graph::mcg::panel_contract::extract_panel_contracts;
use crate::graph::mcg::registry::{AssemblyInputRef, McgEdgeRecord, McgNodeRecord, McgRegistryWriter};
use crate::graph::mcg::scene_payload::{persist_scene_payload_artifact, scene_payload_revision};
use crate::graph::mrg::invalidation::{apply_mcg_invalidation, changed_bundle_owners};
use crate::graph::mrg::registry::MrgRegistryWriter;
use crate::graph::types::{GraphNodeId, GraphNodeKind, MaterialState, PayloadRef};

#[derive(Debug, Clone, Default)]
pub struct McgUpdateOutcome {
    pub scene_payload_revision: Option<String>,
    pub assembly_view_revision: Option<String>,
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
    if !graph_registry_enabled() {
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

    if let Ok(rel) = persist_scene_payload_artifact(
        app_root.as_path(),
        target_file.as_str(),
        scene_revision.as_str(),
        &scene_payload_value(compiled),
    ) {
        registry.upsert_node(McgNodeRecord {
            id: GraphNodeId::new(GraphNodeKind::ScenePayload, target_file.clone()),
            revision: scene_revision.clone(),
            state: MaterialState::Ready,
            layer: "compile".to_string(),
            payload_ref: Some(PayloadRef {
                kind: "scene_payload".to_string(),
                relative_path: rel,
                schema_version: super::scene_payload::SCENE_PAYLOAD_ARTIFACT_SCHEMA.to_string(),
                content_hash: None,
            }),
            deps: Vec::new(),
            defs_fingerprint: None,
            owner_resource_id: None,
            assembly_inputs: Vec::new(),
            stats: None,
        });
    }

    let bundles = extract_metric_def_bundles(compiled, dataset_runtime_payloads);
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
        registry.upsert_node(bundle_node_record(bundle));
        registry.edges.push(McgEdgeRecord {
            from: format!("metric_def_bundle:{owner_id}"),
            to: format!("mrg:eval_plan:{owner_id}"),
            kind: "exports_to_mrg".to_string(),
        });
    }

    let panel_contracts = extract_panel_contracts(compiled);
    let panel_inputs = panel_contracts
        .iter()
        .map(|panel| AssemblyInputRecord {
            kind: "panel_contract".to_string(),
            key: panel.panel_key.clone(),
            revision: panel.revision.clone(),
        })
        .collect::<Vec<_>>();

    let scene_input = AssemblyInputRecord {
        kind: "scene_payload".to_string(),
        key: target_file.clone(),
        revision: scene_revision.clone(),
    };
    let (_, assembly_inputs) = assemble_assembly_view(
        compiled.clone(),
        AssemblyViewInputs {
            scene_payload: Some(scene_input),
            metric_def_bundles: bundle_inputs,
            panel_contracts: panel_inputs,
        },
    );
    let av_revision = assembly_view_revision(&assembly_inputs);
    outcome.assembly_view_revision = Some(av_revision.clone());

    registry.upsert_node(McgNodeRecord {
        id: GraphNodeId::new(
            GraphNodeKind::AssemblyView,
            assembly_view_key(options, compile_revision),
        ),
        revision: av_revision,
        state: MaterialState::Ready,
        layer: "assembly".to_string(),
        payload_ref: Some(PayloadRef {
            kind: "compiled_app".to_string(),
            relative_path: ".mei/artifacts/compiled_app/".to_string(),
            schema_version: "mei-compiled-app-artifact-v3".to_string(),
            content_hash: Some(compile_revision.to_string()),
        }),
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
    let invalidation = apply_mcg_invalidation(&mut mrg, scene_only, changed_owners.as_slice());
    if invalidation.scene_only_skip {
        tracing::debug!(app_id = %app_id, "MRG invalidation skipped (scene-only bump)");
    }
    mrg.finalize();
    MrgRegistryWriter::save(source_root, &mrg)?;

    Ok(outcome)
}

fn bundle_node_record(bundle: &MetricDefBundleRecord) -> McgNodeRecord {
    McgNodeRecord {
        id: GraphNodeId::new(
            GraphNodeKind::MetricDefBundle,
            bundle.owner_resource_id.clone(),
        ),
        revision: bundle.revision.clone(),
        state: MaterialState::Ready,
        layer: "eval_export".to_string(),
        payload_ref: None,
        deps: Vec::new(),
        defs_fingerprint: Some(bundle.defs_fingerprint.clone()),
        owner_resource_id: Some(bundle.owner_resource_id.clone()),
        assembly_inputs: Vec::new(),
        stats: None,
    }
}

fn assembly_view_key(options: &mei_lang_kernel::CompileOptions, compile_revision: &str) -> String {
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
    serde_json::json!({
        "activeTargetFile": compiled.active_target_file,
        "activeScene": compiled.active_scene,
        "sceneRoutes": compiled.scene_routes,
    })
}
