use axum::{
    extract::{Path, State},
    response::{IntoResponse, Response},
    Json,
};

use crate::{
    agent_runtime::bridge::{BridgePromptRequest, BridgePromptSummary},
    mei_agent::{agent_send_prompt, mode_policy::AgentModePolicy, resolve_agent_conn},
    AppState,
};

use crate::http::agent_api::prompt_context::{
    enrich_prompt_request, load_or_refresh_session_context, prepare_prompt_request,
};
use crate::http::error_response;

pub async fn api_agent_send_message(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Json(mut request): Json<BridgePromptRequest>,
) -> Response {
    let conn = match resolve_agent_conn(&state) {
        Ok(c) => c,
        Err(error) => return error_response(error),
    };
    let policy = AgentModePolicy::from_request(&request);
    if let Err(error) = policy.validate() {
        return error_response(error);
    }
    policy.apply_to_request(&mut request);
    if let Err(error) = prepare_prompt_request(&state, &mut request) {
        return error.into_response();
    }
    let session_context = load_or_refresh_session_context(&state, &session_id, &request);
    let request = enrich_prompt_request(&state, session_context.as_deref(), request);
    match agent_send_prompt(&state, &conn, &session_id, request).await {
        Ok(summary) => Json::<BridgePromptSummary>(summary).into_response(),
        Err(error) => error_response(error),
    }
}
