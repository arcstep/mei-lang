use std::path::Path;

use anyhow::{anyhow, Result};
use mei_lang_kernel::{
    initial_runtime_state, project_runtime_view, render_runtime_html, runtime_step, RuntimeIntent,
    RuntimeSceneView, RuntimeState, RuntimeTraceItem,
};

pub struct RuntimeSimStepResult {
    pub state: RuntimeState,
    pub scene_view: RuntimeSceneView,
    pub trace_delta: Vec<RuntimeTraceItem>,
    pub html: String,
}

pub fn runtime_sim_step(
    source_root: &Path,
    app_id: &str,
    request_state: Option<RuntimeState>,
    intent: RuntimeIntent,
) -> Result<RuntimeSimStepResult> {
    let components_root = crate::resolve_components_root(source_root);
    let compiled = crate::compile_app_with_cache(
        source_root,
        app_id,
        mei_lang_kernel::CompileOptions::default(),
        components_root.as_path(),
    )
    .map(|outcome| outcome.compiled)
    .map_err(|failure| failure.error)?;
    let contract = compiled
        .scene_contract
        .ok_or_else(|| anyhow!("app `{}` does not provide a scene contract", app_id))?;
    let current_state = request_state
        .clone()
        .unwrap_or_else(|| initial_runtime_state(&contract, 1));
    let next_state = runtime_step(&contract, request_state, &intent);
    let trace_delta = if next_state.trace_events.len() > current_state.trace_events.len() {
        next_state.trace_events[current_state.trace_events.len()..].to_vec()
    } else if intent.kind == "sync" {
        next_state.trace_events.clone()
    } else {
        Vec::new()
    };
    let scene_view = project_runtime_view(&contract, &next_state);
    let html = render_runtime_html(&scene_view, &next_state);
    Ok(RuntimeSimStepResult {
        state: next_state,
        scene_view,
        trace_delta,
        html,
    })
}
