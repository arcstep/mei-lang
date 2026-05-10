use axum::{
    extract::{Path as AxumPath, State},
    Json,
};
use mei_lang_kernel::{
    compile_app, initial_runtime_state, project_runtime_view, render_runtime_html, runtime_step,
    RuntimeIntent, RuntimeSceneView, RuntimeState, RuntimeTraceItem,
};
use serde::{Deserialize, Serialize};

use crate::{AppError, AppState};

#[derive(Debug, Deserialize)]
pub struct SimStepRequest {
    #[serde(default)]
    pub state: Option<RuntimeState>,
    pub intent: RuntimeIntent,
}

#[derive(Debug, Serialize)]
pub struct SimStepResponse {
    pub state: RuntimeState,
    pub scene_view: RuntimeSceneView,
    #[serde(default)]
    pub trace_delta: Vec<RuntimeTraceItem>,
    pub html: String,
}

pub async fn sim_step_api(
    State(state): State<AppState>,
    AxumPath(app_id): AxumPath<String>,
    Json(request): Json<SimStepRequest>,
) -> Result<Json<SimStepResponse>, AppError> {
    let compiled = compile_app(&state.source_root, &app_id).map_err(AppError::from)?;
    let contract = compiled
        .scene_contract
        .ok_or_else(|| AppError::msg(format!("app `{app_id}` does not provide a scene contract")))?;
    let current_state = request
        .state
        .clone()
        .unwrap_or_else(|| initial_runtime_state(&contract, 1));
    let next_state = runtime_step(&contract, request.state, &request.intent);
    let trace_delta = if next_state.trace_events.len() > current_state.trace_events.len() {
        next_state.trace_events[current_state.trace_events.len()..].to_vec()
    } else if request.intent.kind == "sync" {
        next_state.trace_events.clone()
    } else {
        Vec::new()
    };
    let scene_view = project_runtime_view(&contract, &next_state);
    let html = render_runtime_html(&scene_view, &next_state);
    Ok(Json(SimStepResponse {
        state: next_state,
        scene_view,
        trace_delta,
        html,
    }))
}
