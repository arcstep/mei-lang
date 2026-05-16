mod api;
mod summarize;
mod types;
mod upstream;
mod wire;

pub(crate) use api::{
    abort_session, create_session, global_event, health, list_pending_permissions, list_sessions,
    project_current_worktree, respond_permission, revert_session_message, send_prompt,
    session_diff, session_messages, unrevert_session, vcs_summary,
};
pub(crate) use types::*;

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::wire::decode_applied_response;

    #[test]
    fn decode_applied_response_accepts_bool() {
        assert!(decode_applied_response("revert", json!(true)).expect("bool response"));
    }

    #[test]
    fn decode_applied_response_accepts_session_object() {
        assert!(decode_applied_response(
            "revert",
            json!({
                "id": "ses_demo",
                "revert": { "messageID": "msg_demo" }
            }),
        )
        .expect("object response"));
    }

    #[test]
    fn decode_applied_response_rejects_unexpected_shape() {
        assert!(decode_applied_response("revert", json!("unexpected")).is_err());
    }
}
