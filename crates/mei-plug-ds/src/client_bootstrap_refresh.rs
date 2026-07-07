use std::collections::BTreeMap;

use mei_host_core::HostContext;
use mei_host_graph::{
    client_bootstrap_pack_candidate_scopes, client_bootstrap_scope_allowed, write_client_bootstrap,
};
use mei_lang_datasets::default_result_artifact_scope;
use mei_lang_kernel::{load_mei_config_for_app, FilterIntent, QueryState};

use crate::eval_pipeline::EvalPipelineOutcome;

pub fn maybe_refresh_client_bootstrap_after_eval(
    ctx: &HostContext,
    scene_id: &str,
    workset_id: &str,
    pipeline: &EvalPipelineOutcome,
    query_state: &QueryState,
    filter_intents: &[FilterIntent],
) {
    if pipeline.artifact_hit {
        return;
    }
    if !default_result_artifact_scope(query_state, filter_intents) {
        return;
    }
    let config = load_mei_config_for_app(ctx.app_root().as_path(), None);
    let client_cfg = config
        .runtime
        .client_bootstrap
        .clone()
        .unwrap_or_default();
    if !client_cfg.enabled {
        return;
    }
    let pack_scopes = client_bootstrap_pack_candidate_scopes(
        ctx.workspace_root.as_path(),
        ctx.app_id.as_str(),
        scene_id,
    );
    if !client_bootstrap_scope_allowed(
        scene_id,
        client_cfg.scopes.as_slice(),
        pack_scopes.as_slice(),
    ) {
        return;
    }
    let mut metrics_map = BTreeMap::new();
    let mut metric_total_rows = BTreeMap::new();
    for metric in &pipeline.metrics {
        metrics_map.insert(metric.id.clone(), metric.clone());
        metric_total_rows.insert(metric.id.clone(), pipeline.total_rows);
    }
    if metrics_map.is_empty() {
        return;
    }
    if let Err(error) = write_client_bootstrap(
        ctx.app_root().as_path(),
        ctx.app_id.as_str(),
        scene_id,
        workset_id,
        &pipeline.descriptors,
        &metrics_map,
        &metric_total_rows,
        client_cfg.max_metrics_per_scope,
    ) {
        tracing::warn!(
            app_id = %ctx.app_id,
            scene_id = %scene_id,
            error = %error,
            "failed to refresh client bootstrap after JIT eval"
        );
    }
}

pub fn configure_runtime_eval_cache(ctx: &HostContext) {
    let config = load_mei_config_for_app(ctx.app_root().as_path(), None);
    mei_lang_datasets::configure_metric_response_cache_ttl_ms(
        config.runtime.server_eval_cache.ttl_ms,
    );
}
