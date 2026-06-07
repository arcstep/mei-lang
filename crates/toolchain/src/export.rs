use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use mei_lang_kernel::{
    build_runtime_eval_plan, compile_revision_plan_from_root_with_options, locate_dataset_resource,
    resolve_runtime_metric_def_key, runtime_analysis_closure_metric_ids, CompileOptions,
    CompileRevisionPlan, DatasetView, DimensionBinding, FilterIntent, FilterIntentSource,
    FilterOperator, QueryState, RuntimeMetricEvalScope,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::artifact_store::{
    write_json_artifact, ArtifactStoreWriteResult, ArtifactWatchedFile, ArtifactWriteContext,
};
use crate::types::WorldScope;
use crate::world::{build_world_context_snapshot, load_world_runtime_bundle};

pub const HEADLESS_EXPORT_SCHEMA_VERSION: &str = "mei-headless-export-v1";

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HeadlessArtifactKind {
    InventorySnapshot,
    SemanticDag,
    AnalysisContracts,
    EvalPlan,
    RuntimeTrace,
}

impl HeadlessArtifactKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::InventorySnapshot => "inventory_snapshot",
            Self::SemanticDag => "semantic_dag",
            Self::AnalysisContracts => "analysis_contracts",
            Self::EvalPlan => "eval_plan",
            Self::RuntimeTrace => "runtime_trace",
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct HeadlessExportOptions {
    pub write_store: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeadlessArtifactEnvelope {
    pub schema_version: String,
    pub artifact_kind: HeadlessArtifactKind,
    pub app_id: String,
    pub scope: WorldScope,
    pub revision_token: String,
    pub components_revision: u128,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_scene_id: Option<String>,
    pub active_target_file: String,
    pub artifact: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub store: Option<ArtifactStoreWriteResult>,
}

fn compile_options_from_scope(scope: &WorldScope) -> CompileOptions {
    CompileOptions {
        scene: scope.scene_id.clone(),
        preview_target: scope.target_file.clone(),
    }
}

fn app_root(source_root: &Path, app_id: &str) -> Result<PathBuf> {
    let trimmed = app_id.trim();
    if trimmed.is_empty() {
        anyhow::bail!("--app is required");
    }
    Ok(source_root.join(trimmed))
}

fn export_context(
    source_root: &Path,
    app_id: &str,
    scope: &WorldScope,
) -> Result<(PathBuf, CompileRevisionPlan)> {
    let app_root = app_root(source_root, app_id)?;
    let revision = compile_revision_plan_from_root_with_options(
        source_root,
        &app_root,
        &compile_options_from_scope(scope),
    )?;
    Ok((app_root, revision))
}

fn finalize_envelope(
    app_root: &Path,
    revision: &CompileRevisionPlan,
    options: HeadlessExportOptions,
    artifact_kind: HeadlessArtifactKind,
    artifact_name: String,
    app_id: &str,
    scope: &WorldScope,
    active_scene_id: Option<String>,
    active_target_file: String,
    artifact: Value,
) -> Result<HeadlessArtifactEnvelope> {
    let store = if options.write_store {
        Some(write_json_artifact(
            app_root,
            &ArtifactWriteContext {
                app_id: app_id.to_string(),
                artifact_kind: artifact_kind.as_str().to_string(),
                artifact_name,
                scope: scope.clone(),
                active_scene_id: active_scene_id.clone(),
                active_target_file: active_target_file.clone(),
                revision_token: revision.token.clone(),
                components_revision: revision.components_revision,
                watched_files: revision
                    .watched_files
                    .iter()
                    .map(ArtifactWatchedFile::from)
                    .collect(),
            },
            &artifact,
        )?)
    } else {
        None
    };
    Ok(HeadlessArtifactEnvelope {
        schema_version: HEADLESS_EXPORT_SCHEMA_VERSION.to_string(),
        artifact_kind,
        app_id: app_id.to_string(),
        scope: scope.clone(),
        revision_token: revision.token.clone(),
        components_revision: revision.components_revision,
        active_scene_id,
        active_target_file,
        artifact,
        store,
    })
}

fn collect_dataset_views(compiled: &mei_lang_kernel::CompiledApp) -> BTreeMap<String, DatasetView> {
    compiled
        .resources
        .iter()
        .filter_map(|resource| resource.dataset.as_ref().map(|dataset| (dataset.id.clone(), dataset.clone())))
        .collect()
}

fn normalize_filters(filters: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    filters
        .iter()
        .filter_map(|(key, value)| {
            let key = key.trim();
            let value = value.trim();
            if key.is_empty() || value.is_empty() {
                None
            } else {
                Some((key.to_string(), value.to_string()))
            }
        })
        .collect()
}

fn normalize_search(search: Option<&str>) -> Option<String> {
    search
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn query_state(filters: &BTreeMap<String, String>, search: Option<&str>) -> QueryState {
    QueryState {
        filters: normalize_filters(filters),
        search: normalize_search(search),
        group: Vec::new(),
        time_range: None,
    }
}

fn filter_intents(state: &QueryState) -> Vec<FilterIntent> {
    state
        .filters
        .iter()
        .map(|(dimension, value)| FilterIntent {
            dimension: dimension.clone(),
            operator: FilterOperator::Eq,
            value: value.clone(),
            source: FilterIntentSource::QueryState,
        })
        .collect()
}

fn dataset_field_names(dataset: &DatasetView) -> BTreeSet<String> {
    let mut fields = dataset.columns.iter().cloned().collect::<BTreeSet<_>>();
    for column in &dataset.schema {
        fields.insert(column.name.clone());
        if let Some(source) = column.source.as_ref() {
            fields.insert(source.clone());
        }
    }
    fields
}

fn dimension_bindings(dataset: &DatasetView, state: &QueryState) -> Result<Vec<DimensionBinding>> {
    let fields = dataset_field_names(dataset);
    let mut bindings = Vec::new();
    for dimension in state.filters.keys() {
        if fields.contains(dimension) {
            bindings.push(DimensionBinding {
                dimension: dimension.clone(),
                field: dimension.clone(),
            });
            continue;
        }
        let fallback = fields
            .iter()
            .find(|field| field.eq_ignore_ascii_case(dimension))
            .cloned();
        if let Some(field) = fallback {
            bindings.push(DimensionBinding {
                dimension: dimension.clone(),
                field,
            });
        } else {
            anyhow::bail!(
                "filter dimension `{dimension}` is not available on dataset `{}`",
                dataset.id
            );
        }
    }
    Ok(bindings)
}

fn resolve_metric_ids(dataset: &DatasetView, requested_metric_ids: &[String]) -> Result<Vec<String>> {
    if requested_metric_ids.is_empty() {
        return Ok(dataset.runtime_metric_defs.keys().cloned().collect());
    }
    requested_metric_ids
        .iter()
        .map(|metric_id| {
            resolve_runtime_metric_def_key(&dataset.id, metric_id, &dataset.runtime_metric_defs)
                .with_context(|| {
                    format!(
                        "failed to resolve runtime metric `{metric_id}` for dataset `{}`",
                        dataset.id
                    )
                })
        })
        .collect()
}

pub fn export_inventory_snapshot(
    source_root: &Path,
    app_id: &str,
    scope: &WorldScope,
    options: HeadlessExportOptions,
) -> Result<HeadlessArtifactEnvelope> {
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
) -> Result<HeadlessArtifactEnvelope> {
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
        dataset.runtime_metric_defs.keys().cloned().collect::<Vec<_>>()
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
) -> Result<HeadlessArtifactEnvelope> {
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
        let selected = selected_metric_ids
            .into_iter()
            .collect::<BTreeSet<_>>();
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
) -> Result<HeadlessArtifactEnvelope> {
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
) -> Result<HeadlessArtifactEnvelope> {
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

#[cfg(test)]
mod tests {
    use super::{normalize_filters, normalize_search, query_state};
    use std::collections::BTreeMap;

    #[test]
    fn normalize_filters_trims_and_drops_empty_values() {
        let mut filters = BTreeMap::new();
        filters.insert(" dept ".to_string(), " one ".to_string());
        filters.insert("empty".to_string(), "   ".to_string());
        let normalized = normalize_filters(&filters);
        assert_eq!(normalized.get("dept").map(String::as_str), Some("one"));
        assert!(!normalized.contains_key("empty"));
    }

    #[test]
    fn query_state_normalizes_top_level_search() {
        let state = query_state(&BTreeMap::new(), Some("  foo  "));
        assert_eq!(state.search.as_deref(), Some("foo"));
        assert_eq!(normalize_search(Some("   ")), None);
    }
}
