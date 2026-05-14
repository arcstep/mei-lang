use serde::Serialize;
use serde_json::Value;

use super::bridge::BridgeSessionMessageRaw;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct HostOpencodeMessageList {
    pub session_id: String,
    pub messages: Vec<HostOpencodeMessageSnapshot>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct HostOpencodeMessageSnapshot {
    pub session_id: String,
    pub message_id: String,
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish: Option<String>,
    pub parts: Vec<HostOpencodePartSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct HostOpencodePartSummary {
    pub part_id: String,
    pub message_id: String,
    pub part_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<HostOpencodeToolSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct HostOpencodeToolSummary {
    pub call_id: String,
    pub tool: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum HostOpencodeEvent {
    SessionStatus {
        session_id: String,
        status: String,
        message: String,
    },
    MessageInfo {
        session_id: String,
        message_id: String,
        role: String,
        finish: Option<String>,
    },
    MessagePartUpsert {
        session_id: String,
        message_id: String,
        part: HostOpencodePartSummary,
    },
    MessagePartDelta {
        session_id: String,
        message_id: String,
        part_id: String,
        field: String,
        delta: String,
    },
    MessagePartRemoved {
        session_id: String,
        message_id: String,
        part_id: String,
    },
    PermissionRequested {
        session_id: String,
        permission_id: String,
        permission: String,
        patterns: Vec<String>,
        metadata: Value,
    },
    PermissionBlocked {
        session_id: String,
        permission_id: String,
        permission: String,
        path: Option<String>,
        patterns: Vec<String>,
        requires_admin: bool,
        message: String,
    },
    PermissionResolved {
        session_id: String,
        permission_id: String,
        response: String,
    },
    DebugRawEvent {
        session_id: Option<String>,
        event_type: String,
        payload: Value,
    },
}

impl HostOpencodeEvent {
    pub(crate) fn debug(event_type: impl Into<String>, payload: Value) -> Self {
        let session_id = extract_session_id_from_global_event(&payload).map(ToString::to_string);
        Self::DebugRawEvent {
            session_id,
            event_type: event_type.into(),
            payload,
        }
    }
}

fn as_str<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

pub(crate) fn looks_like_meilang_skill_path(value: &str) -> bool {
    let normalized = value.replace('\\', "/");
    normalized.contains("/.mei/opencode/skills/meilang-author")
}

fn extract_payload_event(value: &Value) -> Option<&Value> {
    value
        .get("payload")
        .and_then(Value::as_object)
        .map(|_| &value["payload"])
        .or(Some(value))
}

pub(crate) fn extract_session_id_from_global_event(value: &Value) -> Option<&str> {
    let payload = extract_payload_event(value)?;
    let properties = payload.get("properties")?;
    as_str(properties, "sessionID")
        .or_else(|| {
            properties
                .get("part")
                .and_then(|part| as_str(part, "sessionID"))
        })
        .or_else(|| {
            properties
                .get("info")
                .and_then(|info| as_str(info, "sessionID"))
        })
}

fn normalize_part_value(part: &Value, with_raw: bool) -> Option<HostOpencodePartSummary> {
    let part_id = as_str(part, "id")?.to_string();
    let message_id = as_str(part, "messageID")?.to_string();
    let part_type = as_str(part, "type")?.to_string();
    let text = as_str(part, "text").map(ToString::to_string);

    let tool = if part_type == "tool" {
        let call_id = as_str(part, "callID").unwrap_or_default().to_string();
        let tool_name = as_str(part, "tool").unwrap_or_default().to_string();
        let state = part.get("state");
        let status = state
            .and_then(|s| as_str(s, "status"))
            .unwrap_or("pending")
            .to_string();
        let input_path = state
            .and_then(|s| s.get("input"))
            .and_then(|input| as_str(input, "filePath"))
            .map(ToString::to_string);
        let title = state
            .and_then(|s| as_str(s, "title"))
            .map(ToString::to_string);
        let output = state
            .and_then(|s| as_str(s, "output"))
            .map(ToString::to_string);
        let error = state
            .and_then(|s| as_str(s, "error"))
            .map(ToString::to_string);
        Some(HostOpencodeToolSummary {
            call_id,
            tool: tool_name,
            status,
            input_path,
            title,
            output,
            error,
        })
    } else {
        None
    };

    let raw = if with_raw {
        Some(part.clone())
    } else {
        match part_type.as_str() {
            "text" | "reasoning" | "tool" | "step-start" | "step-finish" | "patch" => None,
            _ => Some(part.clone()),
        }
    };

    Some(HostOpencodePartSummary {
        part_id,
        message_id,
        part_type,
        text,
        tool,
        raw,
    })
}

pub(crate) fn normalize_upstream_message_to_snapshot(
    raw: &BridgeSessionMessageRaw,
) -> Option<HostOpencodeMessageSnapshot> {
    let info = raw.info.as_object()?;
    let message_id = info.get("id").and_then(Value::as_str)?.to_string();
    let session_id = info.get("sessionID").and_then(Value::as_str)?.to_string();
    let role = info
        .get("role")
        .and_then(Value::as_str)
        .unwrap_or("assistant")
        .to_string();
    let finish = info
        .get("finish")
        .and_then(Value::as_str)
        .map(ToString::to_string);
    let parts = raw
        .parts
        .iter()
        .filter_map(|part| normalize_part_value(part, false))
        .collect::<Vec<_>>();
    Some(HostOpencodeMessageSnapshot {
        session_id,
        message_id,
        role,
        finish,
        parts,
    })
}

pub(crate) fn normalize_global_event_to_host_event(
    value: Value,
    current_session_id: &str,
) -> Option<HostOpencodeEvent> {
    let payload = extract_payload_event(&value)?.clone();
    let event_type = as_str(&payload, "type")?.to_string();
    let properties = payload.get("properties").cloned().unwrap_or(Value::Null);

    match event_type.as_str() {
        "server.connected" => {
            return Some(HostOpencodeEvent::SessionStatus {
                session_id: current_session_id.to_string(),
                status: "connected".to_string(),
                message: "事件流已连接".to_string(),
            });
        }
        "server.heartbeat" => {
            return Some(HostOpencodeEvent::SessionStatus {
                session_id: current_session_id.to_string(),
                status: "heartbeat".to_string(),
                message: "事件流心跳".to_string(),
            });
        }
        _ => {}
    }

    let session_id = extract_session_id_from_global_event(&payload)?;
    if session_id != current_session_id {
        return None;
    }

    match event_type.as_str() {
        "message.updated" => {
            let info = properties.get("info")?;
            let message_id = as_str(info, "id")?.to_string();
            let role = as_str(info, "role").unwrap_or("assistant").to_string();
            Some(HostOpencodeEvent::MessageInfo {
                session_id: session_id.to_string(),
                message_id,
                role,
                finish: info.get("finish").and_then(Value::as_str).map(ToString::to_string),
            })
        }
        "message.part.updated" => {
            let part = properties.get("part")?;
            let normalized = normalize_part_value(part, false)?;
            Some(HostOpencodeEvent::MessagePartUpsert {
                session_id: session_id.to_string(),
                message_id: normalized.message_id.clone(),
                part: normalized,
            })
        }
        "message.part.delta" => Some(HostOpencodeEvent::MessagePartDelta {
            session_id: session_id.to_string(),
            message_id: as_str(&properties, "messageID")?.to_string(),
            part_id: as_str(&properties, "partID")?.to_string(),
            field: as_str(&properties, "field")?.to_string(),
            delta: as_str(&properties, "delta")?.to_string(),
        }),
        "message.part.removed" => Some(HostOpencodeEvent::MessagePartRemoved {
            session_id: session_id.to_string(),
            message_id: as_str(&properties, "messageID")?.to_string(),
            part_id: as_str(&properties, "partID")?.to_string(),
        }),
        "permission.asked" => Some(HostOpencodeEvent::PermissionRequested {
            session_id: session_id.to_string(),
            permission_id: as_str(&properties, "id")
                .or_else(|| as_str(&properties, "requestID"))
                .unwrap_or_default()
                .to_string(),
            permission: as_str(&properties, "permission").unwrap_or("unknown").to_string(),
            patterns: properties
                .get("patterns")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default(),
            metadata: properties.get("metadata").cloned().unwrap_or(Value::Null),
        }),
        "permission.replied" => {
            let permission_id = as_str(&properties, "id")
                .or_else(|| as_str(&properties, "requestID"))
                .unwrap_or_default()
                .to_string();
            Some(HostOpencodeEvent::PermissionResolved {
                session_id: session_id.to_string(),
                permission_id,
                response: as_str(&properties, "response")
                    .or_else(|| as_str(&properties, "reply"))
                    .unwrap_or("unknown")
                    .to_string(),
            })
        }
        _ => Some(HostOpencodeEvent::DebugRawEvent {
            session_id: Some(session_id.to_string()),
            event_type,
            payload,
        }),
    }
}

pub(crate) fn extract_sse_data(frame: &str) -> Option<String> {
    let mut lines = Vec::new();
    for line in frame.lines() {
        let trimmed = line.trim_end();
        if let Some(data) = trimmed.strip_prefix("data:") {
            lines.push(data.trim_start().to_string());
        }
    }
    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n"))
    }
}
