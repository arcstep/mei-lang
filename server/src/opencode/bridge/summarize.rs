use serde_json::{json, Value};

use super::types::{
    BridgeDiffSummary, BridgeFileDiffSummary, BridgePromptRequest, BridgePromptSummary,
    BridgeSessionSummary,
};
use super::upstream::{UpstreamFileDiff, UpstreamPromptResponse, UpstreamSession};

pub(super) fn summarize_session(session: UpstreamSession) -> BridgeSessionSummary {
    BridgeSessionSummary {
        id: session.id,
        title: session.title,
        directory: session.directory,
        created_at_ms: session.time.created,
        updated_at_ms: session.time.updated,
        additions: session.summary.additions,
        deletions: session.summary.deletions,
        files: session.summary.files,
    }
}

pub(super) fn summarize_prompt_response(response: UpstreamPromptResponse) -> BridgePromptSummary {
    let mut texts = Vec::new();
    let mut part_types = Vec::new();
    for part in response.parts {
        if let Some(part_type) = part.get("type").and_then(Value::as_str) {
            part_types.push(part_type.to_string());
            if part_type == "text" {
                if let Some(text) = part.get("text").and_then(Value::as_str) {
                    texts.push(text.to_string());
                }
            }
        }
    }
    BridgePromptSummary {
        session_id: response.info.session_id,
        message_id: response.info.id,
        provider_id: response.info.provider_id,
        model_id: response.info.model_id,
        finish: response.info.finish,
        texts,
        part_types,
        error: response.info.error,
    }
}

pub(super) fn summarize_diff(
    session_id: &str,
    message_id: Option<String>,
    files: Vec<UpstreamFileDiff>,
) -> BridgeDiffSummary {
    let mut additions = 0;
    let mut deletions = 0;
    let files = files
        .into_iter()
        .map(|item| {
            additions += item.additions;
            deletions += item.deletions;
            BridgeFileDiffSummary {
                file: item.file,
                additions: item.additions,
                deletions: item.deletions,
                before: item.before,
                after: item.after,
            }
        })
        .collect::<Vec<_>>();
    BridgeDiffSummary {
        session_id: session_id.to_string(),
        message_id,
        additions,
        deletions,
        files,
    }
}

pub(super) fn prompt_body(request: BridgePromptRequest) -> Value {
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
