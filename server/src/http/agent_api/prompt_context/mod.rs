mod agent_scope;
mod dynamic_context;
mod mei_scan;
mod paths;
mod request_scope;
mod system_prompt;
mod world_snapshot_lines;

pub(crate) use agent_scope::AgentScopeProfile;
pub(crate) use dynamic_context::build_dynamic_session_context_preview;
pub(crate) use dynamic_context::load_or_refresh_session_context;
// 供 crate 内其它模块复用；mod.rs 自身不引用。
#[allow(unused_imports)]
pub(crate) use paths::{resolve_app_root, sanitize_relative_path};

use crate::{agent_runtime::bridge::BridgePromptRequest, AppState};

pub(crate) fn enrich_prompt_request(
    state: &AppState,
    session_context: Option<&str>,
    mut request: BridgePromptRequest,
) -> BridgePromptRequest {
    let user_text = request.text.trim().to_string();
    request.text = user_text;
    request.system = system_prompt::build_meilang_system_prompt(
        state,
        request.system.as_deref(),
        &request,
        session_context,
    );
    request
}
