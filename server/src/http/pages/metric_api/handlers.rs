use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

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
use crate::http::compile_cache::{
    RuntimeAccessPolicies, RuntimeArtifactPolicy, RuntimeAssemblyPolicy, RuntimeEvalPolicy,
};
use crate::http::observation::{CompileObservation, EvalObservation};
use crate::{AppError, AppState};
use axum::{
    extract::{Path as AxumPath, State},
    http::StatusCode,
    Json,
};
use mei_lang_datasets::{
    collect_all_query_options, default_result_artifact_scope, evaluate_runtime_metrics_from_plan,
    load_metric_response_result_artifact, load_prebuild_metric_response_artifact_dataset_fallback,
    metric_response_artifact_lookup_cache_keys, metric_response_cache_scope_key,
    normalize_query_filters, normalize_query_search, plan_access_metric_eval_for_ids,
    project_requested_metrics, query_state_from_request, runtime_metric_workset,
    store_cached_metric_response, store_metric_response_result_artifact,
    take_cached_metric_response, take_metric_response_index_stats, RuntimeMetricEvalMode,
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
    access_artifact_only: bool,
    runtime_policy: RuntimeArtifactPolicy,
    access_policies: RuntimeAccessPolicies,
    compile_correctness_fallback: bool,
    compile_artifact_backfilled: bool,
}

#[derive(Debug, Clone)]
struct MetricQueryExecutionShared {
    app_id: String,
    app_root: std::path::PathBuf,
    compiled: Arc<mei_lang_kernel::CompiledApp>,
    coords: SceneQueryCoords,
    scene_id: String,
    scene_path: Option<String>,
    compile_observation: CompileObservation,
    compile_revision: String,
    effective_query_state: QueryState,
    filter_intents: Vec<FilterIntent>,
    access_artifact_only: bool,
    runtime_policy: RuntimeArtifactPolicy,
    access_policies: RuntimeAccessPolicies,
    compile_correctness_fallback: bool,
    compile_artifact_backfilled: bool,
}

impl MetricQueryExecutionShared {
    fn as_borrowed(&self) -> MetricQueryExecutionContext<'_> {
        MetricQueryExecutionContext {
            app_id: &self.app_id,
            app_root: self.app_root.as_path(),
            compiled: self.compiled.as_ref(),
            coords: &self.coords,
            scene_id: &self.scene_id,
            scene_path: self.scene_path.as_deref(),
            compile_observation: self.compile_observation.clone(),
            compile_revision: &self.compile_revision,
            effective_query_state: &self.effective_query_state,
            filter_intents: &self.filter_intents,
            access_artifact_only: self.access_artifact_only,
            runtime_policy: self.runtime_policy,
            access_policies: self.access_policies,
            compile_correctness_fallback: self.compile_correctness_fallback,
            compile_artifact_backfilled: self.compile_artifact_backfilled,
        }
    }
}

#[derive(Debug, Clone)]
struct MergedMetricGroupRequest {
    request: MetricQueryGroupRequest,
    original_indexes: Vec<usize>,
}

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
    let access_artifact_only = !matches!(runtime_policy, RuntimeArtifactPolicy::BuildViewJit);
    let compile_resolution = crate::http::compile_cache::resolve_runtime_compile_shared(
        &state,
        &app_id,
        &compile_options,
        components_root.as_path(),
        access_policies,
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
            metric_ids = %requested_metric_ids,
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

fn merge_metric_query_groups(groups: &[MetricQueryGroupRequest]) -> Vec<MergedMetricGroupRequest> {
    let mut merged = BTreeMap::<String, MergedMetricGroupRequest>::new();
    for (index, group) in groups.iter().enumerate() {
        let entry =
            merged
                .entry(group.dataset_id.clone())
                .or_insert_with(|| MergedMetricGroupRequest {
                    request: MetricQueryGroupRequest {
                        dataset_id: group.dataset_id.clone(),
                        metric_ids: group.metric_ids.clone(),
                    },
                    original_indexes: Vec::new(),
                });
        entry.original_indexes.push(index);
        if group.metric_ids.is_empty() {
            entry.request.metric_ids.clear();
            continue;
        }
        if entry.request.metric_ids.is_empty() {
            continue;
        }
        entry
            .request
            .metric_ids
            .extend(group.metric_ids.iter().cloned());
        entry.request.metric_ids.sort();
        entry.request.metric_ids.dedup();
    }
    merged.into_values().collect()
}

fn project_metric_group_response(
    merged: &MetricQueryGroupResponse,
    request: &MetricQueryGroupRequest,
) -> MetricQueryGroupResponse {
    if request.metric_ids.is_empty() {
        return merged.clone();
    }
    let metrics = request
        .metric_ids
        .iter()
        .filter_map(|metric_id| {
            merged
                .metrics
                .iter()
                .find(|metric| metric.id == *metric_id)
                .cloned()
        })
        .collect::<Vec<_>>();
    MetricQueryGroupResponse {
        dataset_id: request.dataset_id.clone(),
        total_rows: merged.total_rows,
        metrics,
        perf: merged.perf.clone(),
    }
}

fn write_runtime_policy_perf(
    ctx: &MetricQueryExecutionContext<'_>,
    perf: &mut BTreeMap<String, u64>,
    result_artifact_backfilled: bool,
) {
    let correctness_fallback = ctx.compile_correctness_fallback
        || (ctx.runtime_policy.is_artifact_first_fallback() && result_artifact_backfilled);
    perf.insert(
        "runtime_artifact_policy_sealed_strict".to_string(),
        u64::from(ctx.runtime_policy.is_sealed_strict()),
    );
    perf.insert(
        "runtime_artifact_policy_artifact_first_fallback".to_string(),
        u64::from(matches!(
            ctx.runtime_policy,
            RuntimeArtifactPolicy::ArtifactFirstFallback
        )),
    );
    perf.insert(
        "correctness_fallback".to_string(),
        u64::from(correctness_fallback),
    );
    perf.insert(
        "artifact_backfilled".to_string(),
        u64::from(ctx.compile_artifact_backfilled || result_artifact_backfilled),
    );
    perf.insert(
        "metric_result_artifact_backfilled".to_string(),
        u64::from(result_artifact_backfilled),
    );
    perf.insert(
        "runtime_assembly_policy_sealed".to_string(),
        u64::from(matches!(
            ctx.access_policies.assembly,
            RuntimeAssemblyPolicy::Sealed
        )),
    );
    perf.insert(
        "runtime_eval_policy_artifact_first_thin".to_string(),
        u64::from(matches!(
            ctx.access_policies.eval,
            RuntimeEvalPolicy::ArtifactFirstThin
        )),
    );
}

fn execute_metric_query_group(
    ctx: &MetricQueryExecutionContext<'_>,
    request: &MetricQueryGroupRequest,
) -> Result<MetricQueryGroupResponse, AppError> {
    let request_started = Instant::now();
    let requested_metric_ids = requested_metric_ids_label(&request.metric_ids);
    let locate_started = Instant::now();
    let resource =
        locate_dataset_resource(ctx.compiled, request.dataset_id.trim(), Some(ctx.coords))
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

    let request_all_metrics = request.metric_ids.is_empty();
    let access_plan =
        plan_access_metric_eval_for_ids(ctx.compiled, resource.id.as_str(), &request.metric_ids)
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
    if !owner_dataset.has_runtime_metric_defs() {
        return Err(AppError::status(
            StatusCode::SERVICE_UNAVAILABLE,
            format!(
                "dataset `{}` missing runtime_metric_defs for strict AOT metric query; run `mei-toolchain prebuild` first",
                resource.id
            ),
        ));
    }
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
    let query = collect_all_query_options(ctx.effective_query_state);
    let result_artifact_candidate =
        default_result_artifact_scope(ctx.effective_query_state, ctx.filter_intents);
    let lookup_cache_keys = metric_response_artifact_lookup_cache_keys(
        ctx.app_id,
        ctx.app_root,
        ctx.compiled,
        ctx.scene_id,
        ctx.scene_path,
        access_plan.owner.id.as_str(),
        owner_dataset,
        &query,
        ctx.compile_revision,
        ctx.filter_intents,
        result_artifact_candidate,
    );
    let response_cache_key = lookup_cache_keys.first().cloned().unwrap_or_else(|| {
        metric_response_cache_scope_key(
            ctx.app_id,
            ctx.scene_id,
            ctx.scene_path,
            access_plan.owner.id.as_str(),
            &query,
            ctx.compile_revision,
            "",
            ctx.filter_intents,
        )
    });
    let response_cache_lookup_started = Instant::now();
    let mut cached_hit = None;
    for cache_key in &lookup_cache_keys {
        if let Some(cached) =
            take_cached_metric_response(cache_key, &requested_eval_metric_ids, request_all_metrics)
        {
            cached_hit = Some((cache_key.clone(), cached));
            break;
        }
    }
    if let Some((hit_cache_key, cached)) = cached_hit {
        let mut perf = BTreeMap::new();
        ctx.compile_observation.write_perf(&mut perf);
        perf.insert(
            "access_artifact_only_mode".to_string(),
            u64::from(ctx.access_artifact_only),
        );
        write_runtime_policy_perf(ctx, &mut perf, false);
        perf.insert("locate_dataset_ms".to_string(), locate_dataset_ms);
        perf.insert("result_artifact_hit".to_string(), 0);
        let mut eval_observation = EvalObservation::new(true)
            .with_response_cache_key_hash(hash_metric_response_cache_key(&hit_cache_key));
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
    if result_artifact_candidate {
        let mut loaded_artifact = None;
        let mut used_fallback = false;
        for cache_key in &lookup_cache_keys {
            if let Some((artifact, artifact_load_ms)) =
                load_metric_response_result_artifact(ctx.app_root, cache_key)?
            {
                let artifact_covers_request = if request_all_metrics {
                    artifact.complete
                } else {
                    requested_eval_metric_ids
                        .iter()
                        .all(|metric_id| artifact.covered_metric_ids.contains(metric_id))
                };
                if artifact_covers_request {
                    loaded_artifact = Some((cache_key.clone(), artifact, artifact_load_ms));
                    break;
                }
            }
        }
        if loaded_artifact.is_none() {
            if let Some((cache_key, artifact, artifact_load_ms)) =
                load_prebuild_metric_response_artifact_dataset_fallback(
                    ctx.app_root,
                    ctx.app_id,
                    access_plan.owner.id.as_str(),
                    &query,
                    &requested_eval_metric_ids,
                    request_all_metrics,
                )?
            {
                used_fallback = true;
                loaded_artifact = Some((cache_key, artifact, artifact_load_ms));
            }
        }
        if let Some((hit_cache_key, artifact, artifact_load_ms)) = loaded_artifact {
            let index_stats = take_metric_response_index_stats();
            store_cached_metric_response(
                hit_cache_key.clone(),
                artifact.total_rows,
                &artifact.metrics_map,
                &artifact.covered_metric_ids,
                artifact.complete,
            );
            let mut perf = BTreeMap::new();
            ctx.compile_observation.write_perf(&mut perf);
            perf.insert(
                "access_artifact_only_mode".to_string(),
                u64::from(ctx.access_artifact_only),
            );
            write_runtime_policy_perf(ctx, &mut perf, false);
            perf.insert("locate_dataset_ms".to_string(), locate_dataset_ms);
            perf.insert("response_cache_hit".to_string(), 0);
            perf.insert("result_artifact_hit".to_string(), 1);
            perf.insert("result_artifact_load_ms".to_string(), artifact_load_ms);
            perf.insert(
                "result_artifact_index_load_ms".to_string(),
                index_stats.load_ms,
            );
            perf.insert(
                "result_artifact_index_entry_count".to_string(),
                index_stats.entry_count as u64,
            );
            perf.insert(
                "result_artifact_fallback_hit".to_string(),
                u64::from(used_fallback),
            );
            let mut eval_observation = EvalObservation::new(false)
                .with_response_cache_key_hash(hash_metric_response_cache_key(&hit_cache_key));
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
                artifact.covered_metric_ids.len() as u64,
            );
            eval_observation.insert_counter(
                "response_cache_complete".to_string(),
                u64::from(artifact.complete),
            );
            perf.insert("total_ms".to_string(), elapsed_ms(request_started));
            eval_observation.write_perf(&mut perf);
            let metrics = project_requested_metrics(
                &access_plan.owner.id,
                &access_plan.request_metric_ids,
                &owner_dataset.runtime_metric_defs,
                &artifact.metrics_map,
            );
            return Ok(MetricQueryGroupResponse {
                dataset_id: resource.id.clone(),
                total_rows: artifact.total_rows,
                metrics,
                perf,
            });
        }
        if !ctx.access_policies.allows_thin_eval() {
            let error = if crate::http::host_api::host_warmup_in_progress() {
                crate::http::host_api::warmup_pending_user_message()
            } else {
                format!(
                    "missing strict AOT metric result artifact for dataset `{}` scene `{}`; run `mei-toolchain prebuild` first",
                    resource.id, ctx.scene_id
                )
            };
            crate::http::host_api::mark_access_artifact_degraded(
                ctx.app_id,
                Some(ctx.scene_id),
                ctx.scene_path,
                &error,
            );
            return Err(AppError::status(StatusCode::SERVICE_UNAVAILABLE, error));
        }
        // Structural artifacts are ready; fall through to thin eval below.
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
    perf.insert(
        "access_artifact_only_mode".to_string(),
        u64::from(ctx.access_artifact_only),
    );
    write_runtime_policy_perf(ctx, &mut perf, result_artifact_candidate);
    perf.insert("eval_thin".to_string(), 1);
    perf.insert("locate_dataset_ms".to_string(), locate_dataset_ms);
    perf.insert("result_artifact_hit".to_string(), 0);
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
    perf.insert(
        "base_rowset_materialize_ms".to_string(),
        eval_outcome.base_rowset_materialize_ms,
    );
    perf.insert("hydrate_datasets_ms".to_string(), eval_outcome.hydrate_ms);
    perf.insert(
        "workset_artifact_load_ms".to_string(),
        eval_outcome.workset_artifact_load_ms,
    );
    perf.insert(
        "workset_artifact_hit".to_string(),
        u64::from(eval_outcome.workset_artifact_hit),
    );
    perf.insert(
        "eval_artifact_load_ms".to_string(),
        eval_outcome.eval_artifact_load_ms,
    );
    perf.insert(
        "eval_artifact_hit".to_string(),
        u64::from(eval_outcome.eval_artifact_hit),
    );
    perf.insert("compat_compiled_metric_snapshot_fallback".to_string(), 0);
    for (key, value) in eval_outcome.hydrate_perf {
        perf.insert(key, value);
    }
    perf.insert("metric_eval_ms".to_string(), eval_outcome.metric_eval_ms);
    perf.insert("total_ms".to_string(), elapsed_ms(request_started));
    store_cached_metric_response(
        response_cache_key.clone(),
        eval_outcome.total_rows,
        &metrics_map,
        &requested_eval_metric_ids,
        request_all_metrics,
    );
    if result_artifact_candidate {
        store_metric_response_result_artifact(
            ctx.app_root,
            &response_cache_key,
            eval_outcome.total_rows,
            &metrics_map,
            &requested_eval_metric_ids,
            request_all_metrics,
        )?;
    }
    Ok(MetricQueryGroupResponse {
        dataset_id: resource.id.clone(),
        total_rows: eval_outcome.total_rows,
        metrics,
        perf,
    })
}
