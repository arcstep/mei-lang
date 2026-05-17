mod summarize;
mod types;

pub(crate) use types::*;

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use serde_json::{json, Value};

    use super::summarize::prompt_body;
    use super::{BridgeModelRef, BridgePromptRequest};

    fn decode_applied_response(action: &str, value: Value) -> Result<bool> {
        match value {
            Value::Bool(applied) => Ok(applied),
            Value::Object(_) => Ok(true),
            other => anyhow::bail!(
                "unexpected {action} response shape: {}",
                serde_json::to_string(&other).unwrap_or_else(|_| "<non-json>".to_string())
            ),
        }
    }

    #[test]
    fn decode_applied_response_accepts_bool() {
        assert!(decode_applied_response("revert", serde_json::json!(true)).expect("bool response"));
    }

    #[test]
    fn decode_applied_response_accepts_session_object() {
        assert!(decode_applied_response(
            "revert",
            serde_json::json!({
                "id": "ses_demo",
                "revert": { "messageID": "msg_demo" }
            }),
        )
        .expect("object response"));
    }

    #[test]
    fn decode_applied_response_rejects_unexpected_shape() {
        assert!(decode_applied_response("revert", serde_json::json!("unexpected")).is_err());
    }

    #[test]
    fn prompt_body_includes_system_and_model() {
        let req = BridgePromptRequest {
            text: "hello".to_string(),
            app_id: None,
            scene_id: None,
            entry_id: None,
            target_file: None,
            system: Some("system prompt".to_string()),
            agent: Some("build".to_string()),
            model: Some(BridgeModelRef {
                provider_id: "qwen".to_string(),
                model_id: "qwen-max".to_string(),
            }),
        };
        let body = prompt_body(req);
        assert_eq!(body["system"], json!("system prompt"));
        assert_eq!(body["agent"], json!("build"));
        assert_eq!(
            body["model"],
            json!({"providerID": "qwen", "modelID": "qwen-max"})
        );
    }
}
