use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::time::Instant;

use super::super::super::compile_cache::compile_app_with_cache_shared;
use super::super::components::resolve_components_root;
use super::super::scene_qualified::{
    compile_options_from_coords, locate_dataset_resource, resolved_scene_context,
    strict_runtime_query_contract, strict_scene_query_coords, SceneQueryCoords,
};
use super::super::util::elapsed_ms;
use super::assembly::{
    hash_metric_response_cache_key, metric_eval_diagnostic_code, write_dag_perf,
    MetricQueryGroupRequest, MetricQueryGroupResponse, MetricQueryRequest, MetricQueryResponse,
};
use crate::http::observation::{CompileObservation, EvalObservation};
use crate::{AppError, AppState};
use axum::{
    extract::{Path as AxumPath, State},
    http::StatusCode,
    Json,
};
use mei_lang_datasets::{
    collect_all_query_options, evaluate_runtime_metrics_from_plan,
    metric_request_revision_fingerprint_for_compiled, metric_response_cache_scope_key,
    normalize_query_filters, normalize_query_search, plan_access_metric_eval_for_ids,
    project_requested_metrics, query_state_from_request, runtime_metric_scope_requested,
    runtime_metric_workset, store_cached_metric_response, take_cached_metric_response,
    RuntimeMetricEvalMode,
};
use mei_lang_kernel::{resolve_app_root, FilterIntent, QueryState};

#[derive(Debug, Clone)]
struct MetricQueryExecutionContext<'a> {
    app_id: &'a str,
    app_root: &'a Path,
    compiled: &'a mei_lang_kernel::CompiledApp,
    coords: &'a SceneQueryCoords,
    scene_id: &'a str,
    scene_path: Option<&'a str>,
    compile_observation: CompileObservation,
    compile_revision: &'a str,
    effective_query_state: &'a QueryState,
    filter_intents: &'a [FilterIntent],
}

pub async fn dataset_metric_api(
    State(state): State<AppState>,
    AxumPath(app_id_raw): AxumPath<String>,
    Json(request): Json<MetricQueryRequest>,
) -> Result<Json<MetricQueryResponse>, crate::AppError> {
    let request_started = Instant::now();
    let app_id = app_id_raw.trim_start_matches('/').to_string();
    if app_id.is_empty() {
        return Err(AppError::status(
            StatusCode::NOT_FOUND,
            "missing app id in route",
        ));
    }
    strict_runtime_query_contract(
        &request.filters,
        request.search.as_deref(),
        request.query_state.as_ref(),
        &request.filter_intents,
        "metric query",
    )?;
    let requested_scene_id = request
        .scene_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("-");
    let requested_target = request
        .target
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("-");
    let request_groups = normalize_metric_query_groups(&request)?;
    let request_group_count = request_groups.len();
    let requested_dataset_id = if request_group_count > 1 {
        format!("batch:{request_group_count}")
    } else {
        request_groups
            .first()
            .map(|group| group.dataset_id.clone())
            .unwrap_or_else(|| "-".to_string())
    };
    let requested_metric_ids = if request_group_count > 1 {
        format!("batch_groups={request_group_count}")
    } else {
        request_groups
            .first()
            .map(|group| requested_metric_ids_label(&group.metric_ids))
            .unwrap_or_else(|| "-".to_string())
    };
    let request_span = tracing::info_span!(
        "dataset_metric_api",
        app_id = %app_id,
        scene_id = %requested_scene_id,
        target = %requested_target,
        dataset_id = %requested_dataset_id,
        metric_ids = %requested_metric_ids
    );
    let _request_span_guard = request_span.enter();
    tracing::info!(metric_group_count = request_group_count, "metric query started");

    let coords = strict_scene_query_coords(
        request.scene_id.clone(),
        request.target.clone(),
        "metric query",
    )?;
    let compile_options = compile_options_from_coords(&coords);
    let components_root = resolve_components_root(&state.source_root);
    let compile_outcome =
        compile_app_with_cache_shared(&state, &app_id, compile_options, components_root.as_path())
            .map_err(|failure| {
                tracing::warn!(
                    app_id = %app_id,
                    scene_id = %requested_scene_id,
                    target = %requested_target,
                    dataset_id = %requested_dataset_id,
                    metric_ids = %requested_metric_ids,
                    phase = "compile",
                    error = %failure.error,
                    cache_lookup_ms = failure.cache_lookup_ms,
                    compile_cache_lock_wait_ms = failure.compile_cache_lock_wait_ms,
                    compile_ms = failure.compile_ms,
                    "metric query compile failed"
                );
                AppError::from(failure.error)
            })?;
    let scene_ctx = resolved_scene_context(&compile_outcome.compiled);
    let compile_observation = CompileObservation::from_compile_outcome_shared(
        &app_id,
        &scene_ctx.scene_id,
        scene_ctx.scene_path.as_deref(),
        &compile_outcome,
    );
    let normalized_search = normalize_query_search(request.search.as_deref());
    let normalized_filters = normalize_query_filters(&request.filters);
    let effective_query_state = query_state_from_request(
        &normalized_filters,
        normalized_search.as_deref(),
        request.query_state.as_ref(),
    );
    let app_root = resolve_app_root(state.source_root.as_path(), &app_id);
    let execution_ctx = MetricQueryExecutionContext {
        app_id: &app_id,
        app_root: app_root.as_path(),
        compiled: &compile_outcome.compiled,
        coords: &coords,
        scene_id: &scene_ctx.scene_id,
        scene_path: scene_ctx.scene_path.as_deref(),
        compile_observation,
        compile_revision: &compile_outcome.compile_revision,
        effective_query_state: &effective_query_state,
        filter_intents: &request.filter_intents,
    };

    if request_group_count == 1 {
        let group = execute_metric_query_group(&execution_ctx, &request_groups[0])?;
        return Ok(Json(MetricQueryResponse {
            scene_id: scene_ctx.scene_id,
            scene_path: scene_ctx.scene_path,
            dataset_id: group.dataset_id.clone(),
            total_rows: group.total_rows,
            metrics: group.metrics,
            perf: group.perf,
            groups: Vec::new(),
        }));
    }

    let mut groups = Vec::with_capacity(request_groups.len());
    let batch_started = Instant::now();
    for group in &request_groups {
        groups.push(execute_metric_query_group(&execution_ctx, group)?);
    }
    let mut perf = BTreeMap::new();
    execution_ctx.compile_observation.write_perf(&mut perf);
    perf.insert("metric_group_count".to_string(), groups.len() as u64);
    perf.insert("metric_group_dataset_count".to_string(), groups.len() as u64);
    perf.insert("response_group_count".to_string(), groups.len() as u64);
    perf.insert("batch_eval_ms".to_string(), elapsed_ms(batch_started));
    perf.insert("total_ms".to_string(), elapsed_ms(request_started));
    Ok(Json(MetricQueryResponse {
        scene_id: scene_ctx.scene_id,
        scene_path: scene_ctx.scene_path,
        dataset_id: "__scene_batch__".to_string(),
        total_rows: groups.iter().map(|group| group.total_rows).max().unwrap_or(0),
        metrics: Vec::new(),
        perf,
        groups,
    }))
}

fn normalize_metric_query_groups(
    request: &MetricQueryRequest,
) -> Result<Vec<MetricQueryGroupRequest>, AppError> {
    let mut groups = if request.metric_groups.is_empty() {
        vec![MetricQueryGroupRequest {
            dataset_id: request.dataset_id.trim().to_string(),
            metric_ids: request.metric_ids.clone(),
        }]
    } else {
        request.metric_groups.clone()
    };
    if groups.is_empty() {
        return Err(AppError::status(
            StatusCode::BAD_REQUEST,
            "metric query requires at least one dataset binding",
        ));
    }
    for group in &mut groups {
        group.dataset_id = group.dataset_id.trim().to_string();
        if group.dataset_id.is_empty() {
            return Err(AppError::status(
                StatusCode::BAD_REQUEST,
                "metric query batch requires non-empty `dataset_id`",
            ));
        }
        group.metric_ids = group
            .metric_ids
            .iter()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>();
        group.metric_ids.sort();
        group.metric_ids.dedup();
    }
    Ok(groups)
}

fn requested_metric_ids_label(metric_ids: &[String]) -> String {
    if metric_ids.is_empty() {
        "-".to_string()
    } else {
        metric_ids.join(",")
    }
}

fn execute_metric_query_group(
    ctx: &MetricQueryExecutionContext<'_>,
    request: &MetricQueryGroupRequest,
) -> Result<MetricQueryGroupResponse, AppError> {
    let request_started = Instant::now();
    let requested_metric_ids = requested_metric_ids_label(&request.metric_ids);
    let locate_started = Instant::now();
    let resource = locate_dataset_resource(ctx.compiled, request.dataset_id.trim(), Some(ctx.coords))
        .map_err(|error| {
            tracing::warn!(
                app_id = %ctx.app_id,
                scene_id = %ctx.scene_id,
                target = %ctx.scene_path.unwrap_or("-"),
                dataset_id = %request.dataset_id,
                metric_ids = %requested_metric_ids,
                phase = "locate_dataset",
                error = ?error,
                "metric query locate failed"
            );
            AppError::from(error)
        })?;
    let locate_dataset_ms = elapsed_ms(locate_started);
    let dataset = resource.dataset.as_ref().ok_or_else(|| {
        AppError::status(
            StatusCode::BAD_REQUEST,
            format!("resource `{}` is not a dataset", resource.id),
        )
    })?;
    if !dataset.has_runtime_metric_defs() && dataset.uses_compiled_metric_snapshot_only() {
        if runtime_metric_scope_requested(ctx.effective_query_state, ctx.filter_intents) {
            tracing::warn!(
                app_id = %ctx.app_id,
                scene_id = %ctx.scene_id,
                target = %ctx.scene_path.unwrap_or("-"),
                dataset_id = %resource.id,
                metric_ids = %requested_metric_ids,
                phase = "metric_defs",
                "metric query refused compile-time snapshot fallback for scoped request"
            );
            return Err(AppError::status(
                StatusCode::BAD_REQUEST,
                format!(
                    "dataset `{}` only exposes compile-time metric snapshots; scoped runtime metric queries require runtime_metric_defs",
                    resource.id
                ),
            ));
        }
        let metrics = if request.metric_ids.is_empty() {
            dataset.metrics.values().cloned().collect::<Vec<_>>()
        } else {
            request
                .metric_ids
                .iter()
                .filter_map(|metric_id| dataset.metrics.get(metric_id).cloned())
                .collect::<Vec<_>>()
        };
        let mut perf = BTreeMap::new();
        ctx.compile_observation.write_perf(&mut perf);
        perf.insert("locate_dataset_ms".to_string(), locate_dataset_ms);
        perf.insert("compat_compiled_metric_snapshot_fallback".to_string(), 1);
        perf.insert("total_ms".to_string(), elapsed_ms(request_started));
        return Ok(MetricQueryGroupResponse {
            dataset_id: resource.id.clone(),
            total_rows: 0,
            metrics,
            perf,
        });
    }

    let request_all_metrics = request.metric_ids.is_empty();
    let access_plan =
        plan_access_metric_eval_for_ids(ctx.compiled, request.dataset_id.trim(), &request.metric_ids)
            .map_err(|error| {
                tracing::warn!(
                    app_id = %ctx.app_id,
                    scene_id = %ctx.scene_id,
                    target = %ctx.scene_path.unwrap_or("-"),
                    dataset_id = %request.dataset_id,
                    metric_ids = %requested_metric_ids,
                    phase = "metric_defs",
                    error = %error,
                    "metric query runtime metric plan failed"
                );
                AppError::status(StatusCode::BAD_REQUEST, error.to_string())
            })?;
    let owner_dataset = access_plan.owner_dataset;
    let runtime_workset = runtime_metric_workset(
        &access_plan.owner.id,
        &access_plan.request_metric_ids,
        owner_dataset,
    );
    let requested_eval_metric_ids = runtime_workset
        .eval_metric_ids
        .clone()
        .unwrap_or_default()
        .into_iter()
        .collect::<BTreeSet<_>>();
    let dependency_revision_key = metric_request_revision_fingerprint_for_compiled(
        ctx.app_root,
        ctx.compiled,
        &access_plan.owner.id,
        &runtime_workset.defs_for_hydrate,
    );
    let query = collect_all_query_options(ctx.effective_query_state);
    let response_cache_key = metric_response_cache_scope_key(
        ctx.app_id,
        ctx.scene_id,
        ctx.scene_path,
        &access_plan.owner.id,
        &query,
        ctx.compile_revision,
        &dependency_revision_key,
    );
    let response_cache_lookup_started = Instant::now();
    if let Some(cached) = take_cached_metric_response(
        &response_cache_key,
        &requested_eval_metric_ids,
        request_all_metrics,
    ) {
        let mut perf = BTreeMap::new();
        ctx.compile_observation.write_perf(&mut perf);
        perf.insert("locate_dataset_ms".to_string(), locate_dataset_ms);
        let mut eval_observation = EvalObservation::new(true)
            .with_response_cache_key_hash(hash_metric_response_cache_key(&response_cache_key));
        eval_observation.insert_counter("request_dag_observed", 0);
        eval_observation.insert_counter("eval_memo_hits", 0);
        eval_observation.insert_counter("eval_memo_eval_node_cache_hits", 0);
        eval_observation.insert_counter("eval_memo_eval_node_cache_misses", 0);
        perf.insert(
            "response_cache_lookup_ms".to_string(),
            elapsed_ms(response_cache_lookup_started),
        );
        eval_observation.insert_counter(
            "response_cache_metric_coverage".to_string(),
            cached.covered_metric_ids.len() as u64,
        );
        eval_observation.insert_counter(
            "response_cache_complete".to_string(),
            u64::from(cached.complete),
        );
        perf.insert("total_ms".to_string(), elapsed_ms(request_started));
        eval_observation.write_perf(&mut perf);
        let metrics = project_requested_metrics(
            &access_plan.owner.id,
            &access_plan.request_metric_ids,
            &owner_dataset.runtime_metric_defs,
            &cached.metrics_map,
        );
        return Ok(MetricQueryGroupResponse {
            dataset_id: resource.id.clone(),
            total_rows: cached.total_rows,
            metrics,
            perf,
        });
    }

    let eval_outcome = evaluate_runtime_metrics_from_plan(
        ctx.compiled,
        ctx.app_root,
        &access_plan,
        ctx.scene_id,
        ctx.scene_path,
        ctx.effective_query_state,
        ctx.filter_intents,
        RuntimeMetricEvalMode::WithDag,
        request_all_metrics,
    )
    .map_err(|error| {
        let error_text = error.to_string();
        let diagnostic_code = metric_eval_diagnostic_code(&error_text);
        tracing::warn!(
            app_id = %ctx.app_id,
            scene_id = %ctx.scene_id,
            target = %ctx.scene_path.unwrap_or("-"),
            dataset_id = %resource.id,
            metric_ids = %requested_metric_ids,
            diagnostic_code,
            phase = "metric_eval",
            error = %error,
            "metric query evaluate runtime metric defs failed"
        );
        AppError::from(error)
    })?;
    let metrics = eval_outcome.metrics;
    let metrics_map = eval_outcome.metrics_map;
    let mut perf = eval_outcome.query_perf;
    ctx.compile_observation.write_perf(&mut perf);
    perf.insert("locate_dataset_ms".to_string(), locate_dataset_ms);
    let mut eval_observation = EvalObservation::new(false)
        .with_response_cache_key_hash(hash_metric_response_cache_key(&response_cache_key));
    if let Some(eval_report) = eval_outcome.eval_report.as_ref() {
        eval_observation.insert_counter("request_dag_observed", 1);
        eval_observation.insert_counter(
            "eval_memo_hits",
            eval_report.request_dag_metrics.request_cache_hits,
        );
        eval_observation.insert_counter(
            "eval_memo_eval_node_cache_hits",
            eval_report.request_dag_metrics.eval_node_cache_hits,
        );
        eval_observation.insert_counter(
            "eval_memo_eval_node_cache_misses",
            eval_report.request_dag_metrics.eval_node_cache_misses,
        );
        write_dag_perf(
            &mut perf,
            eval_report,
            &eval_outcome.eval_scope,
            &eval_outcome.closure_metric_ids,
            dataset,
        );
    }
    eval_observation.write_perf(&mut perf);
    perf.insert(
        "response_cache_lookup_ms".to_string(),
        elapsed_ms(response_cache_lookup_started),
    );
    perf.insert("query_api_ms".to_string(), eval_outcome.query_ms);
    perf.insert("hydrate_datasets_ms".to_string(), eval_outcome.hydrate_ms);
    perf.insert("compat_compiled_metric_snapshot_fallback".to_string(), 0);
    for (key, value) in eval_outcome.hydrate_perf {
        perf.insert(key, value);
    }
    perf.insert("metric_eval_ms".to_string(), eval_outcome.metric_eval_ms);
    perf.insert("total_ms".to_string(), elapsed_ms(request_started));
    store_cached_metric_response(
        response_cache_key,
        eval_outcome.total_rows,
        &metrics_map,
        &requested_eval_metric_ids,
        request_all_metrics,
    );
    Ok(MetricQueryGroupResponse {
        dataset_id: resource.id.clone(),
        total_rows: eval_outcome.total_rows,
        metrics,
        perf,
    })
}
