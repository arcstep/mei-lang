use std::collections::BTreeMap;
use std::path::Path;
use std::time::Instant;

use anyhow::Result;
use mei_lang_kernel::{
    evaluate_runtime_metric_defs_with_scope, evaluate_runtime_metric_defs_with_scope_and_dag,
    CompiledApp, DatasetView, FilterIntent, MetricContract, QueryState, RuntimeMetricEvalReport,
    RuntimeMetricEvalScope,
};

use super::metric_locate::{plan_access_metric_eval_for_ids, AccessMetricEvalPlan};
use super::types::DatasetQueryOptions;
use super::util::elapsed_ms;
use super::{
    hydrate_file_backed_datasets_for_metric_defs, metric_request_revision_fingerprint,
    project_requested_metrics, query_dataset_rows, runtime_metric_eval_scope, runtime_metric_workset,
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
    pub total_rows: usize,
    pub metrics_map: BTreeMap<String, MetricContract>,
    pub metrics: Vec<MetricContract>,
    pub query_perf: BTreeMap<String, u64>,
    pub hydrate_perf: BTreeMap<String, u64>,
    pub query_ms: u64,
    pub hydrate_ms: u64,
    pub eval_scope_ms: u64,
    pub metric_eval_ms: u64,
    pub eval_scope: RuntimeMetricEvalScope,
    pub eval_report: Option<RuntimeMetricEvalReport>,
}

pub fn build_compiled_datasets_map(
    compiled: &CompiledApp,
    primary_resource_id: &str,
    runtime_dataset: DatasetView,
) -> BTreeMap<String, DatasetView> {
    let mut datasets = compiled
        .resources
        .iter()
        .filter_map(|resource| {
            resource
                .dataset
                .clone()
                .map(|dataset| (resource.id.clone(), dataset))
        })
        .fold(BTreeMap::new(), |mut acc, (resource_id, dataset)| {
            acc.insert(resource_id, dataset.clone());
            acc.entry(dataset.id.clone()).or_insert(dataset);
            acc
        });
    datasets.insert(primary_resource_id.to_string(), runtime_dataset);
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
    let eval_plan = plan_access_metric_eval_for_ids(compiled, dataset_selector, request_metric_ids)?;
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

    let query_started = Instant::now();
    let filtered_rows = query_dataset_rows(app_root, primary_dataset, query_options.clone())?;
    let query_ms = elapsed_ms(query_started);

    let mut runtime_dataset = primary_dataset.clone();
    runtime_dataset.rows = filtered_rows.rows.clone();
    if !filtered_rows.columns.is_empty() {
        runtime_dataset.columns = filtered_rows.columns.clone();
    }

    let mut datasets = build_compiled_datasets_map(compiled, &eval_plan.primary.id, runtime_dataset.clone());
    let workset = runtime_metric_workset(
        &eval_plan.owner.id,
        &eval_plan.request_metric_ids,
        owner_dataset,
    );
    let closure_metric_ids = workset.closure_metric_ids.clone();
    let covered_eval_metric_ids = workset
        .eval_metric_ids
        .clone()
        .unwrap_or_default();
    let metric_filter = workset.eval_metric_ids.as_deref();
    let defs_for_hydrate = workset.defs_for_hydrate.clone();
    let dependency_revision_key = metric_request_revision_fingerprint(
        app_root,
        &datasets,
        &eval_plan.owner.id,
        &defs_for_hydrate,
    );

    let hydrate_started = Instant::now();
    let hydrate_perf = hydrate_file_backed_datasets_for_metric_defs(
        app_root,
        &mut datasets,
        &defs_for_hydrate,
        &query_options,
    )?;
    let hydrate_ms = elapsed_ms(hydrate_started);

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
    )?;
    let eval_scope_ms = elapsed_ms(eval_scope_started);

    let metric_eval_started = Instant::now();
    let (metrics_map, eval_report) = match mode {
        RuntimeMetricEvalMode::WithDag => {
            let (map, report) = evaluate_runtime_metric_defs_with_scope_and_dag(
                &owner_dataset.runtime_metric_defs,
                &runtime_dataset.rows,
                &datasets,
                metric_filter,
                &eval_scope,
            )?;
            (map, Some(report))
        }
        RuntimeMetricEvalMode::WithoutDag => {
            let map = evaluate_runtime_metric_defs_with_scope(
                &owner_dataset.runtime_metric_defs,
                &runtime_dataset.rows,
                &datasets,
                metric_filter,
                &eval_scope,
            )?;
            (map, None)
        }
    };
    let metric_eval_ms = elapsed_ms(metric_eval_started);

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
        total_rows: runtime_dataset.rows.len(),
        metrics_map,
        metrics,
        query_perf: filtered_rows.perf,
        hydrate_perf,
        query_ms,
        hydrate_ms,
        eval_scope_ms,
        metric_eval_ms,
        eval_scope,
        eval_report,
    })
}
