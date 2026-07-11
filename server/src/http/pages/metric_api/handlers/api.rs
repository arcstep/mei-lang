use std::collections::BTreeMap;
use std::time::Instant;

use super::execute::execute_metric_query_group;
use super::helpers::{
    merge_metric_query_groups, normalize_metric_query_groups, project_metric_group_response,
};
use super::types::*;
use crate::http::compile_cache::{RuntimeAccessPolicies, RuntimeArtifactPolicy};
use crate::http::observation::CompileObservation;
use crate::http::pages::components::resolve_components_root;
use crate::http::pages::metric_api::assembly::{
    MetricQueryGroupResponse, MetricQueryRequest, MetricQueryResponse,
};
use crate::http::pages::scene_qualified::{
    compile_options_from_coords, resolved_scene_context, strict_runtime_query_contract,
    strict_scene_query_coords,
};
use crate::http::pages::util::elapsed_ms;
use crate::{AppError, AppState};
use axum::{
    extract::{Path as AxumPath, State},
    http::StatusCode,
    Json,
};
use mei_lang_datasets::{
    normalize_query_filters, normalize_query_search, query_state_from_request,
};
use mei_lang_kernel::resolve_app_root;

fn access_artifact_unavailable_error(
    request_kind: &str,
    app_id: &str,
    scene_id: &str,
    target: &str,
) -> AppError {
    let scene_label = if scene_id.trim().is_empty() || scene_id == "-" {
        "scene=<unspecified>"
    } else {
        scene_id
    };
    let target_label = if target.trim().is_empty() || target == "-" {
        "target=<unspecified>"
    } else {
        target
    };
    AppError::status(
        StatusCode::SERVICE_UNAVAILABLE,
        format!(
            "{request_kind} requires prebuilt access artifacts on access-only host: app={app_id} {scene_label} {target_label}; wait for startup warmup or prebuild artifacts before serving access traffic"
        ),
    )
}

pub async fn dataset_metric_api(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
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
    let metric_count: usize = request_groups.iter().map(|g| g.metric_ids.len()).sum();
    let request_span = tracing::info_span!(
        "dataset_metric_api",
        app_id = %app_id,
        scene_id = %requested_scene_id,
        target = %requested_target,
        dataset_id = %requested_dataset_id,
        metric_count,
        metric_group_count = request_group_count
    );
    let _request_span_guard = request_span.enter();
    tracing::info!(
        metric_group_count = request_group_count,
        "metric query started"
    );

    let coords = strict_scene_query_coords(
        request.scene_id.clone(),
        request.target.clone(),
        "metric query",
    )?;
    let compile_options = compile_options_from_coords(&coords);
    let components_root = resolve_components_root(&state.source_root);
    let runtime_policy = RuntimeArtifactPolicy::from_headers(&headers);
    let access_policies = RuntimeAccessPolicies::from_headers(&headers);
    let access_artifact_only = true;
    let compile_resolution = crate::http::compile_cache::resolve_runtime_compile_shared(
        &state,
        &app_id,
        &compile_options,
        components_root.as_path(),
        access_policies,
        mei_lang_app::UiRouteMode::App,
    )
    .map_err(|failure| AppError::from(failure.error))?
    .ok_or_else(|| {
        let scene_label = if requested_scene_id.trim().is_empty() || requested_scene_id == "-" {
            "scene=<unspecified>".to_string()
        } else {
            requested_scene_id.to_string()
        };
        let target_label = if requested_target.trim().is_empty() || requested_target == "-" {
            "target=<unspecified>".to_string()
        } else {
            requested_target.to_string()
        };
        let error_message = format!(
            "metric query requires prebuilt access artifacts on access-only host: app={app_id} {scene_label} {target_label}; wait for startup warmup or prebuild artifacts before serving access traffic"
        );
        let error = access_artifact_unavailable_error(
            "metric query",
            &app_id,
            requested_scene_id,
            requested_target,
        );
        crate::http::host_api::mark_access_artifact_degraded(
            &app_id,
            Some(requested_scene_id),
            Some(requested_target),
            &error_message,
        );
        tracing::warn!(
            app_id = %app_id,
            scene_id = %requested_scene_id,
            target = %requested_target,
            dataset_id = %requested_dataset_id,
            metric_count,
            phase = "artifact_only_miss",
            "metric query rejected because host requires prebuilt artifacts"
        );
        error
    })?;
    let compile_correctness_fallback = compile_resolution.correctness_fallback;
    let compile_artifact_backfilled = compile_resolution.artifact_backfilled;
    let resolved_access_policies = compile_resolution.access_policies;
    let compile_outcome = compile_resolution.outcome;
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
        source_root: state.source_root.as_path(),
        app_root: app_root.as_path(),
        compiled: &compile_outcome.compiled,
        coords: &coords,
        scene_id: &scene_ctx.scene_id,
        scene_path: scene_ctx.scene_path.as_deref(),
        compile_observation,
        compile_revision: &compile_outcome.compile_revision,
        effective_query_state: &effective_query_state,
        filter_intents: &request.filter_intents,
        access_artifact_only,
        runtime_policy,
        access_policies: resolved_access_policies,
        compile_correctness_fallback,
        compile_artifact_backfilled,
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

    let batch_started = Instant::now();
    let shared_ctx = MetricQueryExecutionShared {
        app_id: app_id.clone(),
        source_root: state.source_root.to_path_buf(),
        app_root: app_root.clone(),
        compiled: compile_outcome.compiled.clone(),
        coords: coords.clone(),
        scene_id: scene_ctx.scene_id.clone(),
        scene_path: scene_ctx.scene_path.clone(),
        compile_observation: execution_ctx.compile_observation.clone(),
        compile_revision: compile_outcome.compile_revision.clone(),
        effective_query_state: effective_query_state.clone(),
        filter_intents: request.filter_intents.clone(),
        access_artifact_only,
        runtime_policy,
        access_policies: resolved_access_policies,
        compile_correctness_fallback,
        compile_artifact_backfilled,
    };
    let merged_groups = merge_metric_query_groups(&request_groups);
    let mut tasks = tokio::task::JoinSet::new();
    for merged in merged_groups.clone() {
        let shared_ctx = shared_ctx.clone();
        tasks.spawn_blocking(move || {
            let ctx = shared_ctx.as_borrowed();
            execute_metric_query_group(&ctx, &merged.request).map(|response| (merged, response))
        });
    }
    let mut merged_responses = Vec::new();
    while let Some(task) = tasks.join_next().await {
        let (merged, group) = task.map_err(|error| {
            AppError::msg(format!("metric batch worker join failed: {error}"))
        })??;
        tracing::info!(
            dataset_id = %group.dataset_id,
            metric_count = group.metrics.len(),
            group_query_api_ms = group.perf.get("query_api_ms").copied().unwrap_or(0),
            group_total_ms = group.perf.get("total_ms").copied().unwrap_or(0),
            group_base_rowset_materialize_ms = group
                .perf
                .get("base_rowset_materialize_ms")
                .copied()
                .unwrap_or(0),
            group_metric_eval_ms = group.perf.get("metric_eval_ms").copied().unwrap_or(0),
            group_response_cache_hit = group.perf.get("response_cache_hit").copied().unwrap_or(0),
            group_result_artifact_hit = group.perf.get("result_artifact_hit").copied().unwrap_or(0),
            group_workset_artifact_hit = group.perf.get("workset_artifact_hit").copied().unwrap_or(0),
            group_eval_artifact_hit = group.perf.get("eval_artifact_hit").copied().unwrap_or(0),
            "metric batch group finished"
        );
        merged_responses.push((merged, group));
    }
    let mut groups = vec![None; request_groups.len()];
    for (merged, group) in merged_responses {
        let bundle_hit = merged.original_indexes.len() > 1;
        for original_index in merged.original_indexes {
            let mut response =
                project_metric_group_response(&group, &request_groups[original_index]);
            response.perf.insert(
                "default_board_bundle_hit".to_string(),
                u64::from(bundle_hit),
            );
            groups[original_index] = Some(response);
        }
    }
    let groups = groups
        .into_iter()
        .flatten()
        .collect::<Vec<MetricQueryGroupResponse>>();
    let mut perf = BTreeMap::new();
    execution_ctx.compile_observation.write_perf(&mut perf);
    perf.insert(
        "access_artifact_only_mode".to_string(),
        u64::from(access_artifact_only),
    );
    perf.insert(
        "runtime_artifact_policy_sealed_strict".to_string(),
        u64::from(runtime_policy.is_sealed_strict()),
    );
    perf.insert(
        "runtime_artifact_policy_artifact_first_fallback".to_string(),
        u64::from(matches!(
            runtime_policy,
            RuntimeArtifactPolicy::ArtifactFirstFallback
        )),
    );
    perf.insert(
        "correctness_fallback".to_string(),
        u64::from(compile_correctness_fallback),
    );
    perf.insert(
        "artifact_backfilled".to_string(),
        u64::from(compile_artifact_backfilled),
    );
    perf.insert("metric_group_count".to_string(), groups.len() as u64);
    perf.insert(
        "metric_group_dataset_count".to_string(),
        merged_groups.len() as u64,
    );
    perf.insert("response_group_count".to_string(), groups.len() as u64);
    perf.insert(
        "default_board_bundle_hit".to_string(),
        u64::from(merged_groups.len() < request_group_count),
    );
    if let Some(slowest_group) = groups
        .iter()
        .max_by_key(|group| group.perf.get("total_ms").copied().unwrap_or(0))
    {
        perf.insert(
            "slowest_group_total_ms".to_string(),
            slowest_group.perf.get("total_ms").copied().unwrap_or(0),
        );
        perf.insert(
            "slowest_group_query_api_ms".to_string(),
            slowest_group.perf.get("query_api_ms").copied().unwrap_or(0),
        );
        perf.insert(
            "slowest_group_base_rowset_materialize_ms".to_string(),
            slowest_group
                .perf
                .get("base_rowset_materialize_ms")
                .copied()
                .unwrap_or(0),
        );
        perf.insert(
            "slowest_group_metric_eval_ms".to_string(),
            slowest_group
                .perf
                .get("metric_eval_ms")
                .copied()
                .unwrap_or(0),
        );
        perf.insert(
            "slowest_group_metric_count".to_string(),
            slowest_group.metrics.len() as u64,
        );
    }
    perf.insert("batch_eval_ms".to_string(), elapsed_ms(batch_started));
    perf.insert("total_ms".to_string(), elapsed_ms(request_started));
    Ok(Json(MetricQueryResponse {
        scene_id: scene_ctx.scene_id,
        scene_path: scene_ctx.scene_path,
        dataset_id: "__scene_batch__".to_string(),
        total_rows: groups
            .iter()
            .map(|group| group.total_rows)
            .max()
            .unwrap_or(0),
        metrics: Vec::new(),
        perf,
        groups,
    }))
}
