use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use super::super::super::compile_cache::compile_app_with_cache;
use super::super::components::resolve_components_root;
use super::super::scene_qualified::{
    compile_options_from_coords, locate_dataset_resource, resolved_scene_context,
    strict_runtime_query_contract, strict_scene_query_coords,
};
use super::super::util::elapsed_ms;
use super::assembly::{
    hash_metric_response_cache_key, metric_eval_diagnostic_code, write_dag_perf,
    MetricQueryRequest, MetricQueryResponse,
};
use crate::http::observation::{CompileObservation, EvalObservation};
use crate::{AppError, AppState};
use axum::{
    extract::{Path as AxumPath, State},
    http::StatusCode,
    Json,
};
use mei_lang_datasets::{
    collect_all_query_options, evaluate_runtime_metrics_from_plan, metric_response_cache_scope_key,
    normalize_query_filters, normalize_query_search, plan_access_metric_eval_for_ids,
    project_requested_metrics, query_state_from_request, runtime_metric_scope_requested,
    store_cached_metric_response, take_cached_metric_response, RuntimeMetricEvalMode,
};
use mei_lang_kernel::resolve_app_root;

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
    let requested_dataset_id = request.dataset_id.trim().to_string();
    let requested_metric_ids = if request.metric_ids.is_empty() {
        "-".to_string()
    } else {
        request
            .metric_ids
            .iter()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>()
            .join(",")
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
    tracing::info!("metric query started");
    let coords = strict_scene_query_coords(
        request.scene_id.clone(),
        request.target.clone(),
        "metric query",
    )?;
    let compile_options = compile_options_from_coords(&coords);
    let components_root = resolve_components_root(&state.source_root);
    let compile_outcome =
        compile_app_with_cache(&state, &app_id, compile_options, components_root.as_path())
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
    let compile_ms = compile_outcome.compile_ms;
    let scene_ctx = resolved_scene_context(&compile_outcome.compiled);
    let compile_observation = CompileObservation::from_compile_outcome(
        &app_id,
        &scene_ctx.scene_id,
        scene_ctx.scene_path.as_deref(),
        &compile_outcome,
    );
    let compiled = compile_outcome.compiled;
    let normalized_dataset_id = request.dataset_id.trim();
    let resource = locate_dataset_resource(&compiled, normalized_dataset_id, Some(&coords))
        .map_err(|error| {
            tracing::warn!(
                app_id = %app_id,
                scene_id = %requested_scene_id,
                target = %requested_target,
                dataset_id = %requested_dataset_id,
                metric_ids = %requested_metric_ids,
                phase = "locate_dataset",
                error = ?error,
                "metric query locate failed"
            );
            AppError::from(error)
        })?;
    let locate_started = Instant::now();
    let locate_dataset_ms = elapsed_ms(locate_started);
    let dataset = resource.dataset.as_ref().ok_or_else(|| {
        AppError::status(
            StatusCode::BAD_REQUEST,
            format!("resource `{}` is not a dataset", resource.id),
        )
    })?;
    let normalized_search = normalize_query_search(request.search.as_deref());
    let normalized_filters = normalize_query_filters(&request.filters);
    let effective_query_state = query_state_from_request(
        &normalized_filters,
        normalized_search.as_deref(),
        request.query_state.as_ref(),
    );
    if !dataset.has_runtime_metric_defs() && dataset.uses_compiled_metric_snapshot_only() {
        if runtime_metric_scope_requested(&effective_query_state, &request.filter_intents) {
            tracing::warn!(
                app_id = %app_id,
                scene_id = %scene_ctx.scene_id,
                target = %scene_ctx.scene_path.as_deref().unwrap_or("-"),
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
        compile_observation.write_perf(&mut perf);
        perf.insert("locate_dataset_ms".to_string(), locate_dataset_ms);
        perf.insert("compat_compiled_metric_snapshot_fallback".to_string(), 1);
        let total_ms = elapsed_ms(request_started);
        perf.insert("total_ms".to_string(), total_ms);
        tracing::info!(
            app_id = %app_id,
            scene_id = %scene_ctx.scene_id,
            target = %scene_ctx.scene_path.as_deref().unwrap_or("-"),
            dataset_id = %resource.id,
            metric_ids = %requested_metric_ids,
            compile_cache_hit = compile_outcome.cache_hit,
            compile_ms,
            total_rows = 0,
            total_ms,
            "metric query finished (dataset static metrics fallback)"
        );
        return Ok(Json(MetricQueryResponse {
            scene_id: scene_ctx.scene_id,
            scene_path: scene_ctx.scene_path,
            dataset_id: resource.id.clone(),
            total_rows: 0,
            metrics,
            perf,
        }));
    }
    let request_all_metrics = request.metric_ids.is_empty();
    let access_plan =
        plan_access_metric_eval_for_ids(&compiled, normalized_dataset_id, &request.metric_ids)
            .map_err(|error| {
                tracing::warn!(
                    app_id = %app_id,
                    scene_id = %scene_ctx.scene_id,
                    target = %scene_ctx.scene_path.as_deref().unwrap_or("-"),
                    dataset_id = %requested_dataset_id,
                    metric_ids = %requested_metric_ids,
                    phase = "metric_defs",
                    error = %error,
                    "metric query runtime metric plan failed"
                );
                AppError::status(StatusCode::BAD_REQUEST, error.to_string())
            })?;
    let owner_dataset = access_plan.owner_dataset;
    let app_root = resolve_app_root(state.source_root.as_path(), &app_id);
    let query = collect_all_query_options(&effective_query_state);
    let response_cache_key = metric_response_cache_scope_key(
        &app_id,
        &scene_ctx.scene_id,
        scene_ctx.scene_path.as_deref(),
        &access_plan.owner.id,
        &query,
        &compile_outcome.compile_revision,
        &mei_lang_datasets::metric_request_revision_fingerprint(
            &app_root,
            &mei_lang_datasets::build_compiled_datasets_map(
                &compiled,
                &access_plan.primary.id,
                access_plan.primary_dataset.clone(),
            ),
            &access_plan.owner.id,
            &mei_lang_datasets::runtime_metric_workset(
                &access_plan.owner.id,
                &access_plan.request_metric_ids,
                owner_dataset,
            )
            .defs_for_hydrate,
        ),
    );
    let requested_eval_metric_ids = mei_lang_datasets::runtime_metric_workset(
        &access_plan.owner.id,
        &access_plan.request_metric_ids,
        owner_dataset,
    )
    .eval_metric_ids
    .unwrap_or_default()
    .into_iter()
    .collect::<BTreeSet<_>>();
    let response_cache_lookup_started = Instant::now();
    if let Some(cached) = take_cached_metric_response(
        &response_cache_key,
        &requested_eval_metric_ids,
        request_all_metrics,
    ) {
        let mut perf = BTreeMap::new();
        compile_observation.write_perf(&mut perf);
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
        let total_ms = elapsed_ms(request_started);
        perf.insert("total_ms".to_string(), total_ms);
        eval_observation.write_perf(&mut perf);
        return Ok(Json(MetricQueryResponse {
            scene_id: scene_ctx.scene_id,
            scene_path: scene_ctx.scene_path,
            dataset_id: resource.id.clone(),
            total_rows: cached.total_rows,
            metrics: project_requested_metrics(
                &access_plan.owner.id,
                &access_plan.request_metric_ids,
                &owner_dataset.runtime_metric_defs,
                &cached.metrics_map,
            ),
            perf,
        }));
    }
    let eval_outcome = evaluate_runtime_metrics_from_plan(
        &compiled,
        &app_root,
        &access_plan,
        &scene_ctx.scene_id,
        scene_ctx.scene_path.as_deref(),
        &effective_query_state,
        &request.filter_intents,
        RuntimeMetricEvalMode::WithDag,
        request_all_metrics,
    )
    .map_err(|error| {
        let error_text = error.to_string();
        let diagnostic_code = metric_eval_diagnostic_code(&error_text);
        tracing::warn!(
            app_id = %app_id,
            scene_id = %scene_ctx.scene_id,
            target = %scene_ctx.scene_path.as_deref().unwrap_or("-"),
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
    compile_observation.write_perf(&mut perf);
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
    let total_ms = elapsed_ms(request_started);
    perf.insert("total_ms".to_string(), total_ms);
    tracing::info!(
        app_id = %app_id,
        scene_id = %scene_ctx.scene_id,
        target = %scene_ctx.scene_path.as_deref().unwrap_or("-"),
        dataset_id = %resource.id,
        metric_ids = %requested_metric_ids,
        compile_cache_hit = compile_outcome.cache_hit,
        compile_ms,
        query_api_ms = eval_outcome.query_ms,
        metric_eval_ms = eval_outcome.metric_eval_ms,
        total_rows = eval_outcome.total_rows,
        total_ms,
        "metric query finished"
    );
    store_cached_metric_response(
        response_cache_key,
        eval_outcome.total_rows,
        &metrics_map,
        &requested_eval_metric_ids,
        request_all_metrics,
    );
    Ok(Json(MetricQueryResponse {
        scene_id: scene_ctx.scene_id,
        scene_path: scene_ctx.scene_path,
        dataset_id: resource.id.clone(),
        total_rows: eval_outcome.total_rows,
        metrics,
        perf,
    }))
}
