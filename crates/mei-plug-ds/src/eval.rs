use std::collections::BTreeSet;

use mei_host_core::HostContext;
use mei_host_graph::assemble_scope_from_registry;
use mei_lang_datasets::{
    collect_all_query_options, evaluate_runtime_metrics, metric_request_revision_fingerprint_for_compiled,
    metric_response_cache_scope_key, store_metric_response_result_artifact, RuntimeMetricEvalMode,
};
use mei_lang_kernel::{CompiledApp, QueryState};

pub fn load_compiled_for_warmup(
    ctx: &HostContext,
    scope_key: &str,
) -> anyhow::Result<(CompiledApp, String)> {
    let outcome = assemble_scope_from_registry(
        ctx.workspace_root.as_path(),
        ctx.app_id.as_str(),
        scope_key,
    )?
    .ok_or_else(|| anyhow::anyhow!("scene `{scope_key}` not assembled"))?;
    Ok((outcome.compiled, outcome.compile_revision))
}

pub fn eval_metric_ids(
    ctx: &HostContext,
    compiled: &CompiledApp,
    compile_revision: &str,
    scope_key: &str,
    owner_resource_id: &str,
    metric_ids: &[String],
) -> anyhow::Result<Vec<(String, String)>> {
    let app_root = ctx.app_root();
    let query_state = QueryState::default();
    let query_options = collect_all_query_options(&query_state);
    let owner_dataset = compiled
        .resources
        .iter()
        .find(|resource| resource.id == owner_resource_id)
        .and_then(|resource| resource.dataset.as_ref())
        .ok_or_else(|| anyhow::anyhow!("owner resource `{owner_resource_id}` not found"))?;
    let dependency_revision_key = metric_request_revision_fingerprint_for_compiled(
        app_root.as_path(),
        compiled,
        owner_resource_id,
        &owner_dataset.runtime_metric_defs,
    );
    let eval = evaluate_runtime_metrics(
        compiled,
        app_root.as_path(),
        owner_resource_id,
        metric_ids,
        scope_key,
        None,
        &query_state,
        &[],
        RuntimeMetricEvalMode::WithDag,
    )?;
    let cache_key = metric_response_cache_scope_key(
        ctx.app_id.as_str(),
        scope_key,
        None,
        owner_resource_id,
        &query_options,
        compile_revision,
        dependency_revision_key.as_str(),
        &[],
        None,
    );
    let covered: BTreeSet<String> = metric_ids.iter().cloned().collect();
    store_metric_response_result_artifact(
        app_root.as_path(),
        cache_key.as_str(),
        eval.total_rows,
        &eval.metrics_map,
        &covered,
        covered.len() == metric_ids.len(),
    )?;
    Ok(metric_ids
        .iter()
        .map(|id| (id.clone(), cache_key.clone()))
        .collect())
}
