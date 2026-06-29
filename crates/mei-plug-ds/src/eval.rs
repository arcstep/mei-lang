use mei_host_core::HostContext;
use mei_host_graph::assemble_scope_from_registry;

use crate::eval_pipeline::{eval_metrics_with_slots, EvalPipelineRequest};

pub fn load_compiled_for_warmup(
    ctx: &HostContext,
    scope_key: &str,
) -> anyhow::Result<(mei_lang_kernel::CompiledApp, String)> {
    let outcome =
        assemble_scope_from_registry(ctx.workspace_root.as_path(), ctx.app_id.as_str(), scope_key)?
            .ok_or_else(|| anyhow::anyhow!("scene `{scope_key}` not assembled"))?;
    Ok((outcome.compiled, outcome.compile_revision))
}

pub fn eval_metric_ids(
    ctx: &HostContext,
    compiled: &mei_lang_kernel::CompiledApp,
    compile_revision: &str,
    scope_key: &str,
    owner_resource_id: &str,
    workset_id: &str,
    bundle_key: &str,
    metric_ids: &[String],
) -> anyhow::Result<crate::eval_pipeline::EvalPipelineOutcome> {
    eval_metrics_with_slots(
        ctx,
        compiled,
        compile_revision,
        &EvalPipelineRequest {
            scope_key: scope_key.to_string(),
            target: None,
            owner_resource_id: owner_resource_id.to_string(),
            metric_ids: metric_ids.to_vec(),
            workset_id: workset_id.to_string(),
            bundle_key: bundle_key.to_string(),
            query_state: mei_lang_kernel::QueryState::default(),
            filter_intents: Vec::new(),
        },
    )
}
