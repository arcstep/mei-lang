use std::collections::BTreeMap;

use super::types::*;
use crate::http::compile_cache::{RuntimeArtifactPolicy, RuntimeAssemblyPolicy, RuntimeEvalPolicy};
use crate::http::pages::metric_api::assembly::{
    MetricQueryGroupRequest, MetricQueryGroupResponse, MetricQueryRequest,
};
use crate::AppError;
use axum::http::StatusCode;

pub(super) fn normalize_metric_query_groups(
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

pub(super) fn requested_metric_ids_label(metric_ids: &[String]) -> String {
    if metric_ids.is_empty() {
        "-".to_string()
    } else {
        metric_ids.join(",")
    }
}

pub(super) fn merge_metric_query_groups(
    groups: &[MetricQueryGroupRequest],
) -> Vec<MergedMetricGroupRequest> {
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

pub(super) fn project_metric_group_response(
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

pub(super) fn write_runtime_policy_perf(
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
