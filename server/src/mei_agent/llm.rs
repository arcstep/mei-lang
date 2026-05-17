use std::io::Read;
use std::path::{Component, Path};
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context, Result};
use reqwest::blocking::Client;
use serde_json::{json, Value};

pub(crate) fn sanitize_relative_path(value: &str) -> Option<String> {
    let mut parts = Vec::new();
    for component in Path::new(value).components() {
        match component {
            Component::Normal(part) => parts.push(part.to_string_lossy().to_string()),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("/"))
    }
}

/// 非流式 tool 轮次上限（每轮一次 `chat/completions`）；过小会在多步排错时提前结束。
const MAX_TOOL_ROUNDS: usize = 16;

/// 兼容 `content` 为字符串或 OpenAI 风格 `[{ "type":"text", "text":"..." }]`。
fn message_assistant_text_content(msg: &Value) -> Option<String> {
    match msg.get("content")? {
        Value::String(s) => {
            if s.is_empty() {
                None
            } else {
                Some(s.clone())
            }
        }
        Value::Array(parts) => {
            let mut out = String::new();
            for p in parts {
                if let Some(t) = p.get("text").and_then(Value::as_str) {
                    out.push_str(t);
                } else if let Some(t) = p.as_str() {
                    out.push_str(t);
                }
            }
            if out.is_empty() {
                None
            } else {
                Some(out)
            }
        }
        _ => None,
    }
}

/// 在阻塞线程内调用（配合 `rusqlite` 等非 Send 状态）；无 tools。
pub(crate) fn stream_chat_completion_blocking(
    client: &Client,
    base_url: &str,
    api_key: &str,
    model: &str,
    messages: Vec<Value>,
    abort_flag: Option<&AtomicBool>,
    mut on_text_delta: impl FnMut(&str) -> Result<()>,
) -> Result<()> {
    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
    let body = json!({
        "model": model,
        "messages": messages,
        "stream": true,
    });
    let mut response = client
        .post(&url)
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .with_context(|| format!("POST {url}"))?
        .error_for_status()
        .with_context(|| format!("POST {url} error status"))?;

    stream_sse_deltas(&mut response, abort_flag, &mut on_text_delta)
}

#[derive(Debug, Clone, Default)]
struct StreamToolCall {
    id: String,
    name: String,
    arguments: String,
}

#[derive(Debug, Clone, Default)]
struct StreamToolRound {
    assistant_text: String,
    tool_calls: Vec<StreamToolCall>,
}

const MAX_STREAM_TOOL_CALLS_PER_ROUND: usize = 32;

fn apply_stream_tool_delta(calls: &mut Vec<StreamToolCall>, item: &Value) {
    let index = item
        .get("index")
        .and_then(Value::as_u64)
        .map(|n| n as usize)
        .unwrap_or(calls.len());
    if index >= MAX_STREAM_TOOL_CALLS_PER_ROUND {
        return;
    }
    if calls.len() <= index {
        calls.resize(index + 1, StreamToolCall::default());
    }
    let slot = &mut calls[index];
    if let Some(id) = item.get("id").and_then(Value::as_str) {
        if !id.is_empty() {
            slot.id = id.to_string();
        }
    }
    if let Some(name) = item.pointer("/function/name").and_then(Value::as_str) {
        if !name.is_empty() {
            slot.name = name.to_string();
        }
    }
    if let Some(args_piece) = item.pointer("/function/arguments").and_then(Value::as_str) {
        if !args_piece.is_empty() {
            slot.arguments.push_str(args_piece);
        }
    }
}

fn stream_chat_with_tools_round_blocking(
    client: &Client,
    url: &str,
    api_key: &str,
    model: &str,
    messages: &[Value],
    tools: &[Value],
    abort_flag: Option<&AtomicBool>,
    on_text_delta: &mut impl FnMut(&str) -> Result<()>,
) -> Result<StreamToolRound> {
    if abort_flag.is_some_and(|f| f.load(Ordering::SeqCst)) {
        anyhow::bail!("aborted");
    }
    let body = json!({
        "model": model,
        "messages": messages,
        "tools": tools,
        "tool_choice": "auto",
        "stream": true,
    });
    let mut response = client
        .post(url)
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .with_context(|| format!("POST {url}"))?
        .error_for_status()
        .with_context(|| format!("POST {url} error status"))?;

    let mut round = StreamToolRound::default();
    let mut carry = String::new();
    let mut buf = [0u8; 4096];
    loop {
        if abort_flag.is_some_and(|f| f.load(Ordering::SeqCst)) {
            anyhow::bail!("aborted");
        }
        let n = response.read(&mut buf).context("read stream with tools")?;
        if n == 0 {
            break;
        }
        carry.push_str(&String::from_utf8_lossy(&buf[..n]));
        if carry.contains("\r\n") {
            carry = carry.replace("\r\n", "\n");
        }
        while let Some(pos) = carry.find("\n\n") {
            let frame = carry[..pos].to_string();
            carry = carry[pos + 2..].to_string();
            for line in frame.lines() {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                let Some(data) = line.strip_prefix("data:") else {
                    continue;
                };
                let data = data.trim();
                if data == "[DONE]" {
                    return Ok(round);
                }
                let chunk: Value = match serde_json::from_str(data) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                if let Some(piece) = chunk
                    .pointer("/choices/0/delta/content")
                    .and_then(Value::as_str)
                {
                    if !piece.is_empty() {
                        round.assistant_text.push_str(piece);
                        on_text_delta(piece)?;
                    }
                }
                if let Some(items) = chunk
                    .pointer("/choices/0/delta/tool_calls")
                    .and_then(Value::as_array)
                {
                    for item in items {
                        apply_stream_tool_delta(&mut round.tool_calls, item);
                    }
                }
            }
        }
    }
    Ok(round)
}

fn stream_sse_deltas(
    response: &mut impl Read,
    abort_flag: Option<&AtomicBool>,
    on_text_delta: &mut impl FnMut(&str) -> Result<()>,
) -> Result<()> {
    let mut carry = String::new();
    let mut buf = [0u8; 4096];
    loop {
        if abort_flag.is_some_and(|f| f.load(Ordering::SeqCst)) {
            anyhow::bail!("aborted");
        }
        let n = response.read(&mut buf).context("read stream")?;
        if n == 0 {
            break;
        }
        carry.push_str(&String::from_utf8_lossy(&buf[..n]));
        if carry.contains("\r\n") {
            carry = carry.replace("\r\n", "\n");
        }
        while let Some(pos) = carry.find("\n\n") {
            let frame = carry[..pos].to_string();
            carry = carry[pos + 2..].to_string();
            for line in frame.lines() {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                let Some(data) = line.strip_prefix("data:") else {
                    continue;
                };
                let data = data.trim();
                if data == "[DONE]" {
                    return Ok(());
                }
                let v: Value = match serde_json::from_str(data) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                let piece = v
                    .pointer("/choices/0/delta/content")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                if !piece.is_empty() {
                    on_text_delta(piece)?;
                }
            }
        }
    }
    Ok(())
}

/// 拒绝明显二进制路径，避免无意义 UTF-8 解码与多余轮次。
pub(crate) fn is_non_text_workspace_asset(rel: &str) -> bool {
    let n = rel.replace('\\', "/").to_ascii_lowercase();
    const BINARY_SUFFIX: &[&str] = &[
        ".xlsx", ".xls", ".xlsm", ".xlsb", ".zip", ".gz", ".tgz", ".7z", ".rar", ".png", ".jpg",
        ".jpeg", ".gif", ".webp", ".ico", ".bmp", ".tif", ".tiff", ".pdf", ".parquet", ".arrow",
        ".feather", ".orc", ".sqlite", ".db", ".wasm", ".woff", ".woff2", ".ttf", ".otf", ".eot",
        ".mp4", ".mp3", ".webm",
    ];
    BINARY_SUFFIX.iter().any(|ext| n.ends_with(ext))
}

pub(crate) fn read_file_tool_definition() -> Value {
    json!({
        "type": "function",
        "function": {
            "name": "read_file",
            "description": "Read a UTF-8 **text** file under the workspace root (e.g. `.mei`, `.md`, `.json`, `.csv`). Do **not** use this on spreadsheets or binaries (`.xlsx`, `.xls`, images, sqlite, etc.) — use `dataset_query` for dataset rows/schema. Path uses forward slashes relative to the workspace.",
            "parameters": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Relative to workspace root (no ..). App sources are usually under <app_id>/..., e.g. spbjw/data/dataset/foo.mei." }
                },
                "required": ["path"]
            }
        }
    })
}

pub(crate) fn execute_read_file_under_root(source_root: &Path, arguments_json: &str) -> String {
    let args: Value = match serde_json::from_str(arguments_json) {
        Ok(v) => v,
        Err(e) => return format!("error: invalid tool arguments JSON: {e}"),
    };
    let raw = args.get("path").and_then(Value::as_str).unwrap_or("");
    let Some(rel) = sanitize_relative_path(raw) else {
        return "error: path must be a relative path without '..'".to_string();
    };
    if is_non_text_workspace_asset(rel.as_str()) {
        return format!(
            "error: `{}` looks like a binary or non-UTF8 workspace asset; read_file only supports text. For tabular datasets use `dataset_query(\"<dataset_resource_id>\")` (bounded schema + sample rows). To inspect exact DSL use a `.mei` text path, not `.xlsx`.",
            rel.replace('\\', "/")
        );
    }
    let full = source_root.join(&rel);
    let Ok(canonical_root) = source_root.canonicalize() else {
        return "error: cannot canonicalize workspace root".to_string();
    };
    let Ok(canonical_file) = full.canonicalize() else {
        return format!("error: file not found: {}", full.display());
    };
    if !canonical_file.starts_with(&canonical_root) {
        return "error: path escapes workspace root".to_string();
    }
    match std::fs::read_to_string(&canonical_file) {
        Ok(s) if s.len() > 200_000 => format!("error: file too large ({} bytes)", s.len()),
        Ok(s) => s,
        Err(e) => format!("error: {e}"),
    }
}

/// 统一走流式：
/// - 无工具调用：首个 delta 直接透传到前端；
/// - 有工具调用：先流出该轮正文，再执行工具，下一轮继续流式，直到无 tool_call 收敛。
pub(crate) fn stream_chat_with_tools_blocking(
    client: &Client,
    base_url: &str,
    api_key: &str,
    model: &str,
    messages: &mut Vec<Value>,
    tools: &[Value],
    abort_flag: Option<&AtomicBool>,
    mut run_tool_batch: impl FnMut(&[(String, String, String)]) -> Vec<String>,
    mut on_text_delta: impl FnMut(&str) -> Result<()>,
    mut on_after_tool_calls: impl FnMut() -> Result<()>,
) -> Result<()> {
    if tools.is_empty() {
        return stream_chat_completion_blocking(
            client,
            base_url,
            api_key,
            model,
            messages.clone(),
            abort_flag,
            on_text_delta,
        );
    }
    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
    for _ in 0..MAX_TOOL_ROUNDS {
        let round = stream_chat_with_tools_round_blocking(
            client,
            &url,
            api_key,
            model,
            messages,
            tools,
            abort_flag,
            &mut on_text_delta,
        )?;

        if round.tool_calls.is_empty() {
            // 无论是否有正文，这一轮都不再触发工具，直接结束。
            return Ok(());
        }

        let tool_calls_json = round
            .tool_calls
            .iter()
            .map(|tc| {
                json!({
                    "id": tc.id,
                    "type": "function",
                    "function": {
                        "name": tc.name,
                        "arguments": tc.arguments,
                    }
                })
            })
            .collect::<Vec<_>>();
        let mut assistant_msg = json!({
            "role": "assistant",
            "content": if round.assistant_text.is_empty() {
                Value::Null
            } else {
                Value::String(round.assistant_text.clone())
            },
            "tool_calls": tool_calls_json,
        });

        if message_assistant_text_content(&assistant_msg).is_none() && !round.assistant_text.is_empty() {
            assistant_msg["content"] = Value::String(round.assistant_text.clone());
        }
        messages.push(assistant_msg);

        let mut batch: Vec<(String, String, String)> = Vec::with_capacity(round.tool_calls.len());
        for tc in &round.tool_calls {
            let id = tc.id.trim();
            let name = tc.name.trim();
            if id.is_empty() || name.is_empty() {
                anyhow::bail!("stream tool call missing id or name");
            }
            let args = if tc.arguments.trim().is_empty() {
                "{}".to_string()
            } else {
                tc.arguments.clone()
            };
            batch.push((id.to_string(), name.to_string(), args));
        }
            let outputs = run_tool_batch(&batch);
            if outputs.len() != batch.len() {
                anyhow::bail!(
                    "tool batch size mismatch (calls={}, outputs={})",
                    batch.len(),
                    outputs.len()
                );
            }
            for ((id, _, _), content) in batch.iter().zip(outputs.into_iter()) {
                messages.push(json!({
                    "role": "tool",
                    "tool_call_id": id,
                    "content": content,
                }));
            }
            // 让后续流式/非流式正文写入新的 text part，使 parts 顺序与对话时间线一致（正文、工具、再正文…）。
            on_after_tool_calls()?;
    }

    anyhow::bail!(
        "tool rounds exceeded max limit ({MAX_TOOL_ROUNDS}); stop to prevent infinite loop"
    )
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        is_non_text_workspace_asset, message_assistant_text_content, sanitize_relative_path,
    };

    #[test]
    fn non_text_asset_suffix_detection() {
        assert!(is_non_text_workspace_asset("spbjw/data/raw/a.xlsx"));
        assert!(is_non_text_workspace_asset("x.XLSX"));
        assert!(!is_non_text_workspace_asset("spbjw/x.mei"));
        assert!(!is_non_text_workspace_asset("spbjw/data/x.csv"));
    }

    #[test]
    fn sanitize_rejects_parent_components() {
        assert!(sanitize_relative_path("../x").is_none());
        assert!(sanitize_relative_path("foo/../bar").is_none());
    }

    #[test]
    fn message_assistant_text_accepts_string_or_parts_array() {
        let msg = json!({"role": "assistant", "content": "hi"});
        assert_eq!(message_assistant_text_content(&msg).as_deref(), Some("hi"));
        let msg2 = json!({"role": "assistant", "content": [{"type":"text","text":"a"},{"type":"text","text":"b"}]});
        assert_eq!(message_assistant_text_content(&msg2).as_deref(), Some("ab"));
    }

    #[test]
    fn sanitize_accepts_plain_relative() {
        assert_eq!(
            sanitize_relative_path("app/foo.mei").as_deref(),
            Some("app/foo.mei")
        );
    }
}
