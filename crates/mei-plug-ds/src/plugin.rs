use std::time::Instant;

use mei_host_core::{DsPlugin, HostContext, MaterializeRequest, MaterializeResult, Plugin};
use mei_host_graph::default_metric_response_descriptor;
use tracing::info;

use crate::eval::{eval_metric_ids, load_compiled_for_warmup};
use crate::warmup::WarmupTarget;

pub struct DsPluginImpl;

impl Plugin for DsPluginImpl {
    fn id(&self) -> &'static str {
        "mei-plug-ds"
    }
}

impl DsPlugin for DsPluginImpl {
    fn materialize(&self, _request: &MaterializeRequest) -> anyhow::Result<MaterializeResult> {
        anyhow::bail!("materialize requires HostContext; use materialize_with_context")
    }
}

pub fn materialize_with_context(
    ctx: &HostContext,
    request: &MaterializeRequest,
) -> anyhow::Result<MaterializeResult> {
    let started = Instant::now();
    let (compiled, compile_revision) = load_compiled_for_warmup(ctx, request.scope_key.as_str())?;
    let results = eval_metric_ids(
        ctx,
        &compiled,
        compile_revision.as_str(),
        request.scope_key.as_str(),
        request.owner_resource_id.as_str(),
        &request.metric_ids,
    )?;
    let wall_ms = started.elapsed().as_millis() as u64;
    let mut slots = Vec::new();
    for (metric_id, content_hash) in results {
        slots.push(default_metric_response_descriptor(
            &format!("{}::{}", request.workset_id, metric_id),
            request.scope_key.as_str(),
            request.owner_resource_id.as_str(),
            &request.bundle_key,
            "ds:v1",
            &content_hash,
            wall_ms,
            false,
        ));
        info!(
            metric_id = %metric_id,
            scope = %request.scope_key,
            "materialized metric"
        );
    }
    Ok(MaterializeResult { slots })
}

pub fn materialize_targets(
    ctx: &HostContext,
    targets: &[WarmupTarget],
) -> anyhow::Result<MaterializeResult> {
    let mut all_slots = Vec::new();
    for target in targets {
        let request = MaterializeRequest {
            scope_key: target.scope_key.clone(),
            workset_id: target.workset_id.clone(),
            owner_resource_id: target.owner_resource_id.clone(),
            metric_ids: target.metric_ids.clone(),
            bundle_key: target.bundle_key.clone(),
        };
        let result = materialize_with_context(ctx, &request)?;
        all_slots.extend(result.slots);
    }
    Ok(MaterializeResult { slots: all_slots })
}

pub fn query_dataset(
    ctx: &HostContext,
    body: &serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    crate::dataset_api::query_dataset(ctx, body)
}

pub fn query_metrics(
    ctx: &HostContext,
    body: &serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    crate::dataset_api::query_metrics(ctx, body)
}
