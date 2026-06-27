use std::time::Instant;

use mei_host_core::{DsPlugin, MaterializeRequest, MaterializeResult, Plugin};
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
    fn materialize(&self, request: &MaterializeRequest) -> anyhow::Result<MaterializeResult> {
        let started = Instant::now();
        let compiled = load_compiled_for_warmup(
            request.scope_key.as_str(),
            request.owner_resource_id.as_str(),
            request.bundle_key.as_str(),
        )?;
        let results = eval_metric_ids(&compiled, &request.metric_ids)?;
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
}

pub fn materialize_targets(
    ctx: &mei_host_core::HostContext,
    targets: &[WarmupTarget],
) -> anyhow::Result<MaterializeResult> {
    let plugin = DsPluginImpl;
    let mut all_slots = Vec::new();
    for target in targets {
        let request = MaterializeRequest {
            scope_key: target.scope_key.clone(),
            workset_id: target.workset_id.clone(),
            owner_resource_id: target.owner_resource_id.clone(),
            metric_ids: target.metric_ids.clone(),
            bundle_key: target.bundle_key.clone(),
        };
        let result = plugin.materialize(&request)?;
        all_slots.extend(result.slots);
    }
    let _ = ctx;
    Ok(MaterializeResult { slots: all_slots })
}

pub fn query_dataset(
    ctx: &mei_host_core::HostContext,
    body: &serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    crate::dataset_api::query_dataset(ctx, body)
}

pub fn query_metrics(
    ctx: &mei_host_core::HostContext,
    body: &serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    crate::dataset_api::query_metrics(ctx, body)
}
