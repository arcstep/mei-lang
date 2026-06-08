use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{anyhow, Result};
use mei_lang_kernel::{
    build_runtime_eval_plan, locate_dataset_resource, runtime_analysis_closure_metric_ids,
    RuntimeMetricEvalScope,
};
use serde_json::json;

use crate::types::WorldScope;
use crate::world::{build_world_context_snapshot, load_world_runtime_bundle};

use super::dataset_binding::{
    collect_dataset_views, dimension_bindings, filter_intents, query_state, resolve_metric_ids,
};
use super::types::{
    export_context, finalize_envelope, HeadlessArtifactKind, HeadlessExportOptions,
};

pub fn export_inventory_snapshot(
    source_root: &Path,
    app_id: &str,
    scope: &WorldScope,
    options: HeadlessExportOptions,
) -> Result<super::types::HeadlessArtifactEnvelope> {
    let (app_root, revision) = export_context(source_root, app_id, scope)?;
    let snapshot = build_world_context_snapshot(source_root, app_id, Some(scope))?;
    let active_scene_id = Some(snapshot.world_snapshot.scene_id.clone());
    let active_target_file = snapshot.active_target_file.clone();
    let artifact = serde_json::to_value(&snapshot)?;
    finalize_envelope(
        &app_root,
        &revision,
        options,
        HeadlessArtifactKind::InventorySnapshot,
        "inventory".to_string(),
        app_id,
        scope,
        active_scene_id,
        active_target_file,
        artifact,
    )
}

pub fn export_semantic_dag(
    source_root: &Path,
    app_id: &str,
    scope: &WorldScope,
    dataset_id: &str,
    metric_ids: &[String],
    options: HeadlessExportOptions,
) -> Result<super::types::HeadlessArtifactEnvelope> {
    let (app_root, revision) = export_context(source_root, app_id, scope)?;
    let bundle = load_world_runtime_bundle(source_root, app_id, Some(scope))?;
    let loaded = locate_dataset_resource(&bundle.compiled, dataset_id)
        .map_err(|error| anyhow!(error.to_string()))?;
    let dataset = loaded
        .dataset
        .as_ref()
        .ok_or_else(|| anyhow!("resource `{dataset_id}` is not a dataset"))?;
    let selected_metric_ids = resolve_metric_ids(dataset, metric_ids)?;
    let closure_metric_ids = if selected_metric_ids.is_empty() {
        dataset
            .runtime_metric_defs
            .keys()
            .cloned()
            .collect::<Vec<_>>()
    } else {
        runtime_analysis_closure_metric_ids(&dataset.runtime_analysis_graph, &selected_metric_ids)
    };
    let artifact = json!({
        "dataset_id": dataset.id,
        "requested_metric_ids": metric_ids,
        "resolved_metric_ids": selected_metric_ids,
        "closure_metric_ids": closure_metric_ids,
        "runtime_metric_ids": dataset.runtime_metric_defs.keys().cloned().collect::<Vec<_>>(),
        "runtime_metric_defs": dataset.runtime_metric_defs,
        "analysis_graph": dataset.runtime_analysis_graph,
    });
    finalize_envelope(
        &app_root,
        &revision,
        options,
        HeadlessArtifactKind::SemanticDag,
        format!("semantic-{}", dataset.id),
        app_id,
        scope,
        bundle.compiled.active_scene.clone(),
        bundle.active_target_file.clone(),
        artifact,
    )
}

pub fn export_analysis_contracts(
    source_root: &Path,
    app_id: &str,
    scope: &WorldScope,
    dataset_id: &str,
    metric_ids: &[String],
    options: HeadlessExportOptions,
) -> Result<super::types::HeadlessArtifactEnvelope> {
    let (app_root, revision) = export_context(source_root, app_id, scope)?;
    let bundle = load_world_runtime_bundle(source_root, app_id, Some(scope))?;
    let loaded = locate_dataset_resource(&bundle.compiled, dataset_id)
        .map_err(|error| anyhow!(error.to_string()))?;
    let dataset = loaded
        .dataset
        .as_ref()
        .ok_or_else(|| anyhow!("resource `{dataset_id}` is not a dataset"))?;
    let selected_metric_ids = resolve_metric_ids(dataset, metric_ids)?;
    let contracts = if selected_metric_ids.is_empty() {
        dataset.runtime_analysis_contracts.clone()
    } else {
        let selected = selected_metric_ids.into_iter().collect::<BTreeSet<_>>();
        dataset
            .runtime_analysis_contracts
            .iter()
            .filter(|(metric_id, _)| selected.contains(*metric_id))
            .map(|(metric_id, value)| (metric_id.clone(), value.clone()))
            .collect()
    };
    let artifact = json!({
        "dataset_id": dataset.id,
        "metric_ids": metric_ids,
        "analysis_contracts": contracts,
    });
    finalize_envelope(
        &app_root,
        &revision,
        options,
        HeadlessArtifactKind::AnalysisContracts,
        format!("contracts-{}", dataset.id),
        app_id,
        scope,
        bundle.compiled.active_scene.clone(),
        bundle.active_target_file.clone(),
        artifact,
    )
}

pub fn export_eval_plan(
    source_root: &Path,
    app_id: &str,
    scope: &WorldScope,
    dataset_id: &str,
    metric_ids: &[String],
    search: Option<&str>,
    filters: &BTreeMap<String, String>,
    options: HeadlessExportOptions,
) -> Result<super::types::HeadlessArtifactEnvelope> {
    let (app_root, revision) = export_context(source_root, app_id, scope)?;
    let bundle = load_world_runtime_bundle(source_root, app_id, Some(scope))?;
    let loaded = locate_dataset_resource(&bundle.compiled, dataset_id)
        .map_err(|error| anyhow!(error.to_string()))?;
    let dataset = loaded
        .dataset
        .as_ref()
        .ok_or_else(|| anyhow!("resource `{dataset_id}` is not a dataset"))?;
    let selected_metric_ids = resolve_metric_ids(dataset, metric_ids)?;
    let state = query_state(filters, search);
    let bindings = dimension_bindings(dataset, &state)?;
    let eval_scope = RuntimeMetricEvalScope {
        base_dataset_id: dataset.id.clone(),
        scene_id: bundle.contract.scene.id.clone(),
        target: bundle.active_target_file.clone(),
        search: state.search.clone().unwrap_or_default(),
        query_state: state.clone(),
        filter_intents: filter_intents(&state),
        dimension_bindings: bindings,
        filters_fingerprint: serde_json::to_string(&state.filters).unwrap_or_default(),
        dependency_revision_key: revision.token.clone(),
    };
    let datasets = collect_dataset_views(&bundle.compiled);
    let eval_plan = build_runtime_eval_plan(
        &dataset.runtime_metric_defs,
        Some(&selected_metric_ids),
        &datasets,
        &eval_scope,
    );
    let artifact = json!({
        "dataset_id": dataset.id,
        "selected_metric_ids": selected_metric_ids,
        "eval_scope": {
            "base_dataset_id": eval_scope.base_dataset_id,
            "scene_id": eval_scope.scene_id,
            "target": eval_scope.target,
            "search": eval_scope.search,
            "query_state": eval_scope.query_state,
            "filter_intents": eval_scope.filter_intents,
            "dimension_bindings": eval_scope.dimension_bindings,
            "filters_fingerprint": eval_scope.filters_fingerprint,
            "dependency_revision_key": eval_scope.dependency_revision_key,
        },
        "eval_plan": eval_plan,
    });
    finalize_envelope(
        &app_root,
        &revision,
        options,
        HeadlessArtifactKind::EvalPlan,
        format!("eval-plan-{}", dataset.id),
        app_id,
        scope,
        bundle.compiled.active_scene.clone(),
        bundle.active_target_file.clone(),
        artifact,
    )
}

pub fn export_runtime_trace(
    source_root: &Path,
    app_id: &str,
    scope: &WorldScope,
    trace_limit: Option<usize>,
    options: HeadlessExportOptions,
) -> Result<super::types::HeadlessArtifactEnvelope> {
    let (app_root, revision) = export_context(source_root, app_id, scope)?;
    let bundle = load_world_runtime_bundle(source_root, app_id, Some(scope))?;
    let trace_limit = trace_limit.unwrap_or(20).clamp(1, 200);
    let trace_events = bundle
        .state
        .trace_events
        .iter()
        .rev()
        .take(trace_limit)
        .cloned()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>();
    let artifact = json!({
        "scene_id": bundle.contract.scene.id,
        "phase": bundle.state.phase,
        "result": bundle.state.result,
        "countdown": bundle.state.countdown,
        "available_actions": bundle.scene_view.available_actions,
        "trace_events": trace_events,
    });
    finalize_envelope(
        &app_root,
        &revision,
        options,
        HeadlessArtifactKind::RuntimeTrace,
        "runtime-trace".to_string(),
        app_id,
        scope,
        bundle.compiled.active_scene.clone(),
        bundle.active_target_file.clone(),
        artifact,
    )
}
