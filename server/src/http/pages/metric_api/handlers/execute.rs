use super::helpers::{requested_metric_ids_label, write_runtime_policy_perf};
use super::types::*;
use crate::http::observation::EvalObservation;
use crate::http::pages::metric_api::assembly::{
    hash_metric_response_cache_key, metric_eval_diagnostic_code, write_dag_perf,
    MetricQueryGroupRequest, MetricQueryGroupResponse,
};
use crate::http::pages::scene_qualified::locate_dataset_resource;
use crate::http::pages::util::elapsed_ms;
use crate::AppError;
use mei_lang_datasets::{
    collect_all_query_options, default_result_artifact_scope, evaluate_runtime_metrics_from_plan,
    load_metric_response_result_artifact, metric_response_artifact_lookup_cache_keys,
    metric_response_cache_scope_key, plan_access_metric_eval_for_ids,
    populate_l1_from_loaded_metric_artifact, project_requested_metrics,
    run_metric_response_artifact_load_singleflight, run_whole_eval_singleflight,
    runtime_metric_workset, snapshot_metric_eval_singleflight_stats,
    store_cached_metric_response_aliases, store_metric_response_result_artifact,
    take_cached_metric_response, RuntimeMetricEvalMode,
};
use std::time::Instant;

pub(super) fn execute_metric_query_group(
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
        None,
        Some(&runtime_workset.defs_for_hydrate),
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
            None,
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
        perf.insert("response_cache_hit".to_string(), 1);
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
    if cached_hit.is_none() && crate::graph::feature::graph_registry_dedup_enabled() {
        let bundle_revisions =
            crate::graph::dedup::load_mcg_bundle_revisions(ctx.source_root, ctx.app_id);
        if let Some(bundle_rev) = bundle_revisions.get(&access_plan.owner.id) {
            let dependency_revision_key =
                mei_lang_datasets::metric_request_revision_fingerprint_for_compiled(
                    ctx.app_root,
                    ctx.compiled,
                    access_plan.owner.id.as_str(),
                    &owner_dataset.runtime_metric_defs,
                );
            let registry = crate::graph::load_mrg_registry(ctx.source_root, ctx.app_id);
            let scope_key = crate::graph::mrg_eval_scope_key(ctx.scene_id, ctx.scene_path);
            for cache_key in &lookup_cache_keys {
                if !crate::graph::mrg_slot_covers_eval(
                    &registry,
                    access_plan.owner.id.as_str(),
                    bundle_rev,
                    dependency_revision_key.as_str(),
                    scope_key.as_str(),
                    cache_key,
                ) {
                    continue;
                }
                if let Some((artifact, artifact_load_ms)) =
                    run_metric_response_artifact_load_singleflight(
                        format!("mrg-artifact|{cache_key}"),
                        || {
                            load_metric_response_result_artifact(ctx.app_root, cache_key)
                                .map_err(|error| error.to_string())
                        },
                    )
                    .map_err(AppError::msg)?
                {
                    populate_l1_from_loaded_metric_artifact(&lookup_cache_keys, &artifact);
                    let mut perf = BTreeMap::new();
                    ctx.compile_observation.write_perf(&mut perf);
                    write_runtime_policy_perf(ctx, &mut perf, true);
                    perf.insert("locate_dataset_ms".to_string(), locate_dataset_ms);
                    perf.insert("response_cache_hit".to_string(), 0);
                    perf.insert("response_cache_populated".to_string(), 1);
                    perf.insert("result_artifact_hit".to_string(), 1);
                    perf.insert("result_artifact_load_ms".to_string(), artifact_load_ms);
                    perf.insert("mrg_eval_skip".to_string(), 1);
                    perf.insert("total_ms".to_string(), elapsed_ms(request_started));
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
            }
        }
    }
    if result_artifact_candidate {
        let mut loaded_artifact = None;
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
            // Strict AOT: no index/dataset fallback; MRG slot + direct artifact load only.
        }
        if let Some((hit_cache_key, artifact, artifact_load_ms)) = loaded_artifact {
            populate_l1_from_loaded_metric_artifact(&lookup_cache_keys, &artifact);
            let mut perf = BTreeMap::new();
            ctx.compile_observation.write_perf(&mut perf);
            perf.insert(
                "access_artifact_only_mode".to_string(),
                u64::from(ctx.access_artifact_only),
            );
            write_runtime_policy_perf(ctx, &mut perf, false);
            perf.insert("locate_dataset_ms".to_string(), locate_dataset_ms);
            perf.insert("response_cache_hit".to_string(), 0);
            perf.insert("response_cache_populated".to_string(), 1);
            perf.insert("result_artifact_hit".to_string(), 1);
            perf.insert("result_artifact_load_ms".to_string(), artifact_load_ms);
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
        if ctx.access_artifact_only || !ctx.access_policies.allows_thin_eval() {
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

    let sf_outcome =
        run_whole_eval_singleflight(format!("whole-eval|{response_cache_key}"), || {
            evaluate_runtime_metrics_from_plan(
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
            .map_err(|error| error.to_string())
        })
        .map_err(|error| {
            let diagnostic_code = metric_eval_diagnostic_code(&error);
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
            AppError::msg(error)
        })?;
    let is_leader = matches!(sf_outcome.role, mei_lang_datasets::SingleflightRole::Leader);
    let eval_outcome = sf_outcome.value;
    let metrics = eval_outcome.metrics;
    let metrics_map = eval_outcome.metrics_map;
    let mut perf = eval_outcome.query_perf;
    perf.insert("eval_singleflight_leader".to_string(), u64::from(is_leader));
    perf.insert(
        "eval_singleflight_waiter".to_string(),
        u64::from(!is_leader),
    );
    let sf = snapshot_metric_eval_singleflight_stats();
    perf.insert("eval_singleflight_leader_total".to_string(), sf.leader);
    perf.insert("eval_singleflight_waiter_total".to_string(), sf.waiter);
    perf.insert(
        "eval_singleflight_penetration_total".to_string(),
        sf.penetration,
    );
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
    for (key, value) in eval_outcome.hydrate_perf {
        perf.insert(key, value);
    }
    perf.insert("metric_eval_ms".to_string(), eval_outcome.metric_eval_ms);
    perf.insert("total_ms".to_string(), elapsed_ms(request_started));
    store_cached_metric_response_aliases(
        &lookup_cache_keys,
        eval_outcome.total_rows,
        &metrics_map,
        &requested_eval_metric_ids,
        request_all_metrics,
    );
    if result_artifact_candidate && is_leader {
        store_metric_response_result_artifact(
            ctx.app_root,
            &response_cache_key,
            eval_outcome.total_rows,
            &metrics_map,
            &requested_eval_metric_ids,
            request_all_metrics,
        )?;
        perf.insert("eval_persist".to_string(), 1);
        let bundle_revisions =
            crate::graph::dedup::load_mcg_bundle_revisions(ctx.source_root, ctx.app_id);
        if let Some(bundle_rev) = bundle_revisions.get(&access_plan.owner.id) {
            let dependency_revision_key =
                mei_lang_datasets::metric_request_revision_fingerprint_for_compiled(
                    ctx.app_root,
                    ctx.compiled,
                    access_plan.owner.id.as_str(),
                    &owner_dataset.runtime_metric_defs,
                );
            let scope_key = crate::graph::mrg_eval_scope_key(ctx.scene_id, ctx.scene_path);
            let workset_id = format!(
                "workset|app={}|owner={}|metrics={}",
                ctx.app_id,
                access_plan.owner.id,
                if request_all_metrics {
                    "*".to_string()
                } else {
                    requested_eval_metric_ids
                        .iter()
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(",")
                }
            );
            crate::graph::record_prebuild_slot(
                ctx.source_root,
                ctx.app_id,
                workset_id.as_str(),
                scope_key.as_str(),
                access_plan.owner.id.as_str(),
                bundle_rev,
                dependency_revision_key.as_str(),
                response_cache_key.as_str(),
                "eval-results/results/metric-response/",
                eval_outcome.metric_eval_ms,
            );
        }
    }
    Ok(MetricQueryGroupResponse {
        dataset_id: resource.id.clone(),
        total_rows: eval_outcome.total_rows,
        metrics,
        perf,
    })
}
