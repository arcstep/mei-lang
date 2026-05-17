//! 与上游 HTTP 脱钩后仍保留的纯 JSON 构造，供 golden / 文档对照。
#![allow(dead_code)]

use serde_json::{json, Value};

use super::types::BridgePromptRequest;

pub(crate) fn prompt_body(request: BridgePromptRequest) -> Value {
    let mut body = json!({
        "parts": [{
            "type": "text",
            "text": request.text,
        }]
    });
    if let Some(system) = request.system {
        body["system"] = Value::String(system);
    }
    if let Some(agent) = request.agent {
        body["agent"] = Value::String(agent);
    }
    if let Some(model) = request.model {
        body["model"] = json!({
            "providerID": model.provider_id,
            "modelID": model.model_id,
        });
    }
    body
}
