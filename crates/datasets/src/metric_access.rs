use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::time::Instant;

use anyhow::Result;
use mei_lang_kernel::{
    capsule_path_from_namespaced_resource_id, evaluate_runtime_metric_defs_with_scope,
    imported_capsule_path_from_world_metrics_resource_id, local_dataset_id_from_namespaced_token,
    CompiledApp, DatasetView, FilterIntent, MetricContract, QueryState, RuntimeMetricEvalReport,
    RuntimeMetricEvalScope,
};

use super::agg_result_cache::{agg_result_cache_key, lookup_agg_result_cache, store_agg_result_cache};
use super::eval_artifact::{
    eval_artifact_hydrate_dataset_ids, load_or_build_runtime_metric_workset_artifact,
};
use super::eval_execute::execute_runtime_eval_plan_artifacts;
use super::metric_hydrate::{resolve_dataset_query_bindings_from_state, unique_dataset_views};
use super::metric_locate::{plan_access_metric_eval_for_ids, AccessMetricEvalPlan};
use super::result_artifact::default_result_artifact_scope;
use super::types::DatasetQueryOptions;
use super::util::elapsed_ms;
use super::{
    hydrate_file_backed_datasets_for_metric_defs, metric_request_revision_fingerprint_for_compiled,
    project_requested_metrics, query_dataset_rows, runtime_metric_eval_scope,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeMetricEvalMode {
    WithDag,
    WithoutDag,
}

#[derive(Debug, Clone)]
pub struct RuntimeMetricEvalOutcome {
    pub primary_resource_id: String,
    pub owner_resource_id: String,
    pub request_metric_ids: Vec<String>,
    pub closure_metric_ids: Vec<String>,
    pub covered_eval_metric_ids: Vec<String>,
    pub dependency_revision_key: String,
    pub workset_artifact_hit: bool,
    pub eval_artifact_hit: bool,
    pub total_rows: usize,
    pub metrics_map: BTreeMap<String, MetricContract>,
    pub metrics: Vec<MetricContract>,
    pub query_perf: BTreeMap<String, u64>,
    pub hydrate_perf: BTreeMap<String, u64>,
    pub base_rowset_materialize_ms: u64,
    pub query_ms: u64,
    pub hydrate_ms: u64,
    pub eval_scope_ms: u64,
    pub workset_artifact_load_ms: u64,
    pub eval_artifact_load_ms: u64,
    pub eval_node_artifact_load_ms: u64,
    pub eval_node_artifact_hits: u64,
    pub eval_node_artifact_stores: u64,
    pub metric_eval_ms: u64,
    pub eval_scope: RuntimeMetricEvalScope,
    pub eval_report: Option<RuntimeMetricEvalReport>,
}

fn capsule_path_aliases(capsule_path: &str) -> Vec<String> {
    let capsule_path = capsule_path.trim();
    if capsule_path.is_empty() {
        return Vec::new();
    }
    let mut out = vec![capsule_path.to_string()];
    if let Some(stripped) = capsule_path.strip_prefix("src/") {
        out.push(stripped.to_string());
    } else {
        out.push(format!("src/{capsule_path}"));
    }
    out
}

fn capsule_paths_for_dataset_binding(
    primary_resource_id: &str,
    referenced_dataset_ids: &BTreeSet<String>,
    active_target_file: &str,
) -> BTreeSet<String> {
    let mut paths = BTreeSet::new();
    if let Some(path) = imported_capsule_path_from_world_metrics_resource_id(primary_resource_id) {
        paths.insert(path);
    }
    if let Some(path) = capsule_path_from_namespaced_resource_id(primary_resource_id) {
        paths.insert(path.to_string());
    }
    for dataset_id in referenced_dataset_ids {
        if let Some(path) = capsule_path_from_namespaced_resource_id(dataset_id) {
            paths.insert(path.to_string());
        }
    }
    let target = active_target_file.trim();
    if !target.is_empty() {
        paths.insert(target.to_string());
        if let Some(stripped) = target.strip_prefix("src/") {
            paths.insert(stripped.to_string());
        } else {
            paths.insert(format!("src/{target}"));
        }
    }
    paths
}

fn resource_matches_capsule_paths(
    resource_id: &str,
    dataset_id: &str,
    capsule_paths: &BTreeSet<String>,
) -> bool {
    capsule_paths.iter().any(|capsule_path| {
        capsule_path_aliases(capsule_path).into_iter().any(|alias| {
            resource_id == alias
                || dataset_id == alias
                || resource_id.starts_with(&format!("{alias}::"))
                || dataset_id.starts_with(&format!("{alias}::"))
        })
    })
}

fn standalone_capsule_local_dataset(
    resource_id: &str,
    dataset_id: &str,
    capsule_paths: &BTreeSet<String>,
    active_target_file: &str,
) -> bool {
    if resource_id.contains("::") || dataset_id.contains("::") || resource_id != dataset_id {
        return false;
    }
    let target = active_target_file.trim();
    if target.is_empty() || !target.ends_with(".mei") {
        return false;
    }
    capsule_paths.iter().any(|capsule_path| {
        capsule_path_aliases(capsule_path).into_iter().any(|alias| alias == target)
    })
}

fn insert_dataset_aliases(
    datasets: &mut BTreeMap<String, DatasetView>,
    resource_id: &str,
    dataset: DatasetView,
) {
    let primary_alias_missing = !datasets.contains_key(&dataset.id);
    datasets.insert(resource_id.to_string(), dataset.clone());
    if primary_alias_missing {
        datasets.insert(dataset.id.clone(), dataset.clone());
    }
    for token in [resource_id, dataset.id.as_str()] {
        if let Some(local) = local_dataset_id_from_namespaced_token(token) {
            datasets.entry(local.to_string()).or_insert_with(|| dataset.clone());
        }
    }
}

pub fn build_compiled_datasets_map(
    compiled: &CompiledApp,
    primary_resource_id: &str,
    runtime_dataset: DatasetView,
    referenced_dataset_ids: &BTreeSet<String>,
) -> BTreeMap<String, DatasetView> {
    let runtime_dataset_id = runtime_dataset.id.clone();
    let capsule_paths = capsule_paths_for_dataset_binding(
        primary_resource_id,
        referenced_dataset_ids,
        compiled.active_target_file.as_str(),
    );
    let mut datasets = BTreeMap::new();
    for resource in &compiled.resources {
        let Some(dataset) = resource.dataset.as_ref() else {
            continue;
        };
        let include = resource.id == primary_resource_id
            || dataset.id == runtime_dataset_id
            || referenced_dataset_ids.contains(&resource.id)
            || referenced_dataset_ids.contains(&dataset.id)
            || resource_matches_capsule_paths(&resource.id, &dataset.id, &capsule_paths)
            || standalone_capsule_local_dataset(
                &resource.id,
                &dataset.id,
                &capsule_paths,
                compiled.active_target_file.as_str(),
            );
        if !include {
            continue;
        }
        insert_dataset_aliases(&mut datasets, resource.id.as_str(), dataset.clone());
    }
    for dataset_id in referenced_dataset_ids {
        if datasets.contains_key(dataset_id) {
            continue;
        }
        if let Some(local) = local_dataset_id_from_namespaced_token(dataset_id) {
            if let Some(view) = datasets.get(local).cloned() {
                datasets.insert(dataset_id.clone(), view);
            }
        }
    }
    datasets.insert(primary_resource_id.to_string(), runtime_dataset.clone());
    datasets.insert(runtime_dataset_id, runtime_dataset);
    datasets
}

pub fn runtime_metric_scope_requested(
    query_state: &QueryState,
    filter_intents: &[FilterIntent],
) -> bool {
    !query_state.filters.is_empty()
        || query_state
            .search
            .as_deref()
            .map(str::trim)
            .is_some_and(|value| !value.is_empty())
        || !query_state.group.is_empty()
        || query_state.time_range.is_some()
        || !filter_intents.is_empty()
}

pub fn collect_all_query_options(query_state: &QueryState) -> DatasetQueryOptions {
    DatasetQueryOptions {
        page: 1,
        page_size: 0,
        search: query_state.search.clone(),
        filters: query_state.filters.clone(),
        group: query_state.group.clone(),
        time_range: query_state.time_range.clone(),
        collect_all: true,
        ..DatasetQueryOptions::default()
    }
}

pub fn evaluate_runtime_metrics(
    compiled: &CompiledApp,
    app_root: &Path,
    dataset_selector: &str,
    request_metric_ids: &[String],
    scene_id: &str,
    scene_path: Option<&str>,
    query_state: &QueryState,
    filter_intents: &[FilterIntent],
    mode: RuntimeMetricEvalMode,
) -> Result<RuntimeMetricEvalOutcome> {
    let request_all_metrics = request_metric_ids.is_empty();
    let eval_plan =
        plan_access_metric_eval_for_ids(compiled, dataset_selector, request_metric_ids)?;
    evaluate_runtime_metrics_from_plan(
        compiled,
        app_root,
        &eval_plan,
        scene_id,
        scene_path,
        query_state,
        filter_intents,
        mode,
        request_all_metrics,
    )
}

pub fn evaluate_runtime_metrics_from_plan<'a>(
    compiled: &'a CompiledApp,
    app_root: &Path,
    eval_plan: &AccessMetricEvalPlan<'a>,
    scene_id: &str,
    scene_path: Option<&str>,
    query_state: &QueryState,
    filter_intents: &[FilterIntent],
    mode: RuntimeMetricEvalMode,
    request_all_metrics: bool,
) -> Result<RuntimeMetricEvalOutcome> {
    let primary_dataset = eval_plan.primary_dataset;
    let owner_dataset = eval_plan.owner_dataset;
    let query_options = collect_all_query_options(query_state);
    let (workset, workset_artifact_load_ms, workset_artifact_hit) =
        load_or_build_runtime_metric_workset_artifact(
            app_root,
            &eval_plan.owner.id,
            &eval_plan.request_metric_ids,
            owner_dataset,
        )?;
    let closure_metric_ids = workset.closure_metric_ids.clone();
    let covered_eval_metric_ids = workset.eval_metric_ids.clone().unwrap_or_default();
    let metric_filter = workset.eval_metric_ids.as_deref();
    let defs_for_hydrate = workset.defs_for_hydrate.clone();
    let referenced_dataset_ids = eval_artifact_hydrate_dataset_ids(&defs_for_hydrate);
    let dependency_revision_key = metric_request_revision_fingerprint_for_compiled(
        app_root,
        compiled,
        &eval_plan.owner.id,
        &defs_for_hydrate,
    );

    if matches!(mode, RuntimeMetricEvalMode::WithDag) {
        let agg_key = agg_result_cache_key(
            compiled.app_id.as_str(),
            &eval_plan.owner.id,
            &covered_eval_metric_ids,
            query_state,
            filter_intents,
            &dependency_revision_key,
        );
        if let Some((cached_metrics_map, cached_total_rows)) = lookup_agg_result_cache(&agg_key) {
            let metrics = if request_all_metrics {
                cached_metrics_map.values().cloned().collect()
            } else {
                project_requested_metrics(
                    &eval_plan.owner.id,
                    &eval_plan.request_metric_ids,
                    &owner_dataset.runtime_metric_defs,
                    &cached_metrics_map,
                )
            };
            let mut query_perf = BTreeMap::new();
            query_perf.insert("agg_cache_hit".to_string(), 1);
            return Ok(RuntimeMetricEvalOutcome {
                primary_resource_id: eval_plan.primary.id.clone(),
                owner_resource_id: eval_plan.owner.id.clone(),
                request_metric_ids: eval_plan.request_metric_ids.clone(),
                closure_metric_ids,
                covered_eval_metric_ids,
                dependency_revision_key: dependency_revision_key.clone(),
                workset_artifact_hit,
                eval_artifact_hit: false,
                total_rows: cached_total_rows,
                metrics_map: cached_metrics_map,
                metrics,
                query_perf,
                hydrate_perf: BTreeMap::new(),
                base_rowset_materialize_ms: 0,
                query_ms: 0,
                hydrate_ms: 0,
                eval_scope_ms: 0,
                workset_artifact_load_ms,
                eval_artifact_load_ms: 0,
                eval_node_artifact_load_ms: 0,
                eval_node_artifact_hits: 0,
                eval_node_artifact_stores: 0,
                metric_eval_ms: 0,
                eval_scope: runtime_metric_eval_scope(
                    Some(primary_dataset),
                    &eval_plan.primary.id,
                    scene_id,
                    scene_path,
                    query_state.search.as_deref(),
                    &query_state.filters,
                    Some(query_state),
                    filter_intents,
                    &dependency_revision_key,
                    &[],
                )?,
                eval_report: None,
            });
        }
    }

    let primary_filters =
        resolve_dataset_query_bindings_from_state(query_state, primary_dataset).mapped_filters;
    let primary_query_options = DatasetQueryOptions {
        filters: primary_filters,
        ..query_options.clone()
    };
    let query_started = Instant::now();
    let filtered_rows = query_dataset_rows(app_root, primary_dataset, primary_query_options)?;
    let query_ms = elapsed_ms(query_started);
    let base_rowset_materialize_ms = query_ms;
    let total_rows = filtered_rows.rows.len();
    let mut query_perf = filtered_rows.perf.clone();

    let mut runtime_dataset = primary_dataset.clone();
    runtime_dataset.rows = filtered_rows.rows;
    if !filtered_rows.columns.is_empty() {
        runtime_dataset.columns = filtered_rows.columns;
    }

    let mut datasets = build_compiled_datasets_map(
        compiled,
        &eval_plan.primary.id,
        runtime_dataset.clone(),
        &referenced_dataset_ids,
    );

    let hydrate_started = Instant::now();
    let hydrate_perf = hydrate_file_backed_datasets_for_metric_defs(
        app_root,
        &mut datasets,
        &defs_for_hydrate,
        &query_options,
    )?;
    let hydrate_ms = elapsed_ms(hydrate_started);

    let binding_datasets = unique_dataset_views(primary_dataset, datasets.values());
    let supplementary_binding_datasets: Vec<&DatasetView> = binding_datasets
        .into_iter()
        .filter(|view| view.id != primary_dataset.id)
        .collect();
    let eval_scope_started = Instant::now();
    let eval_scope = runtime_metric_eval_scope(
        Some(primary_dataset),
        &eval_plan.primary.id,
        scene_id,
        scene_path,
        query_state.search.as_deref(),
        &query_state.filters,
        Some(query_state),
        filter_intents,
        &dependency_revision_key,
        &supplementary_binding_datasets,
    )?;
    let eval_scope_ms = elapsed_ms(eval_scope_started);
    let metric_eval_started = Instant::now();
    let (
        metrics_map,
        eval_report,
        eval_artifact_load_ms,
        eval_artifact_hit,
        eval_node_artifact_load_ms,
        eval_node_artifact_hits,
        eval_node_artifact_stores,
    ) = match mode {
        RuntimeMetricEvalMode::WithDag => {
            let eval_outcome = execute_runtime_eval_plan_artifacts(
                app_root,
                &eval_plan.owner.id,
                &covered_eval_metric_ids,
                &owner_dataset.runtime_metric_defs,
                &datasets,
                &runtime_dataset.rows,
                &eval_scope,
                default_result_artifact_scope(query_state, filter_intents),
            )?;
            (
                eval_outcome.metrics_map,
                Some(eval_outcome.eval_report),
                eval_outcome.eval_artifact_load_ms,
                eval_outcome.eval_artifact_hit,
                eval_outcome.eval_node_artifact_load_ms,
                eval_outcome.eval_node_artifact_hits,
                eval_outcome.eval_node_artifact_stores,
            )
        }
        RuntimeMetricEvalMode::WithoutDag => {
            let map = evaluate_runtime_metric_defs_with_scope(
                &owner_dataset.runtime_metric_defs,
                &runtime_dataset.rows,
                &datasets,
                metric_filter,
                &eval_scope,
            )?;
            (map, None, 0, false, 0, 0, 0)
        }
    };
    let metric_eval_ms = elapsed_ms(metric_eval_started);

    if matches!(mode, RuntimeMetricEvalMode::WithDag) {
        let agg_key = agg_result_cache_key(
            compiled.app_id.as_str(),
            &eval_plan.owner.id,
            &covered_eval_metric_ids,
            query_state,
            filter_intents,
            &dependency_revision_key,
        );
        store_agg_result_cache(agg_key, metrics_map.clone(), total_rows);
        query_perf.insert("agg_cache_hit".to_string(), 0);
    }

    let metrics = if request_all_metrics {
        metrics_map.values().cloned().collect()
    } else {
        project_requested_metrics(
            &eval_plan.owner.id,
            &eval_plan.request_metric_ids,
            &owner_dataset.runtime_metric_defs,
            &metrics_map,
        )
    };

    Ok(RuntimeMetricEvalOutcome {
        primary_resource_id: eval_plan.primary.id.clone(),
        owner_resource_id: eval_plan.owner.id.clone(),
        request_metric_ids: eval_plan.request_metric_ids.clone(),
        closure_metric_ids,
        covered_eval_metric_ids,
        dependency_revision_key,
        workset_artifact_hit,
        eval_artifact_hit,
        total_rows,
        metrics_map,
        metrics,
        query_perf,
        hydrate_perf,
        base_rowset_materialize_ms,
        query_ms,
        hydrate_ms,
        eval_scope_ms,
        workset_artifact_load_ms,
        eval_artifact_load_ms,
        eval_node_artifact_load_ms,
        eval_node_artifact_hits,
        eval_node_artifact_stores,
        metric_eval_ms,
        eval_scope,
        eval_report,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use mei_lang_kernel::{LoadedResource, SourceDecl};

    use super::*;

    fn minimal_dataset(id: &str) -> DatasetView {
        DatasetView {
            id: id.to_string(),
            title: None,
            purpose: None,
            schema: Vec::new(),
            stage_schema: Vec::new(),
            columns: Vec::new(),
            rows: Vec::new(),
            source: SourceDecl {
                kind: "inline".to_string(),
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
            runtime_metric_defs: BTreeMap::new(),
            runtime_analysis_graph: Default::default(),
            runtime_analysis_contracts: BTreeMap::new(),
        }
    }

    #[test]
    fn build_compiled_datasets_map_aliases_capsule_local_warning_list_for_world_metrics() {
        let owner = "__world_metrics__::scenes/05-监督预警.mei::metrics".to_string();
        let compiled = CompiledApp {
            app_id: "data-demo".to_string(),
            title: String::new(),
            app_root: String::new(),
            scene_routes: Vec::new(),
            active_scene: None,
            active_target_file: "src/scenes/05-监督预警.mei".to_string(),
            file_tree: Vec::new(),
            scene_contract: None,
            scene_local_nav_by_target: BTreeMap::new(),
            scene_bindings_by_id: BTreeMap::new(),
            scene_examples_by_id: BTreeMap::new(),
            scene_projection_assembly_by_id: BTreeMap::new(),
            resources: vec![
                LoadedResource {
                    id: owner.clone(),
                    kind: "dataset".to_string(),
                    title: None,
                    document: None,
                    dataset: Some(minimal_dataset(owner.as_str())),
                },
                LoadedResource {
                    id: "warning_list".to_string(),
                    kind: "dataset".to_string(),
                    title: None,
                    document: None,
                    dataset: Some(minimal_dataset("warning_list")),
                },
            ],
            world_metrics: BTreeMap::new(),
            world_semantic_by_file: BTreeMap::new(),
            component_assets: Vec::new(),
            diagnostics: Vec::new(),
            build_experience_index: Default::default(),
            build_t2_page_index: Default::default(),
            build_template_index: Default::default(),
        };
        let referenced = BTreeSet::from(["scenes/05-监督预警.mei::warning_list".to_string()]);
        let runtime = minimal_dataset(owner.as_str());
        let datasets = build_compiled_datasets_map(&compiled, owner.as_str(), runtime, &referenced);
        assert!(datasets.contains_key("warning_list"));
        assert!(datasets.contains_key("scenes/05-监督预警.mei::warning_list"));
    }
}
