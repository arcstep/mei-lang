use std::io::Read;
use std::path::{Component, Path};
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{anyhow, Context, Result};
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

/// 将非流式 `chat/completions` 返回的正文写入 parts。
///
/// 历史上曾按 `as_bytes().chunks(64)` + `str::from_utf8` 切片；在中文等多字节 UTF-8 下极易落在码点中间，
/// `from_utf8` 失败则整段被 `unwrap_or("")` **丢弃**，数据库里会只剩半截 Markdown，与「提取问题」极像。
fn emit_nonstream_assistant_text(
    text: &str,
    abort_flag: Option<&AtomicBool>,
    on_text_delta: &mut impl FnMut(&str) -> Result<()>,
) -> Result<()> {
    if text.is_empty() {
        return Ok(());
    }
    if abort_flag.is_some_and(|f| f.load(Ordering::SeqCst)) {
        anyhow::bail!("aborted");
    }
    on_text_delta(text)
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

fn complete_chat_nonstream_blocking(
    client: &Client,
    url: &str,
    api_key: &str,
    model: &str,
    messages: &[Value],
    tools: &[Value],
    abort_flag: Option<&AtomicBool>,
) -> Result<Value> {
    if abort_flag.is_some_and(|f| f.load(Ordering::SeqCst)) {
        anyhow::bail!("aborted");
    }
    let body = json!({
        "model": model,
        "messages": messages,
        "tools": tools,
        "tool_choice": "auto",
        "stream": false,
    });
    let text = client
        .post(url)
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .with_context(|| format!("POST {url}"))?
        .error_for_status()
        .with_context(|| format!("POST {url} error status"))?
        .text()
        .context("read completion body")?;
    let v: Value = serde_json::from_str(&text).with_context(|| {
        format!(
            "parse completion JSON (prefix {}): {}",
            text.len().min(200),
            &text.chars().take(200).collect::<String>()
        )
    })?;
    if let Some(err) = v.pointer("/error/message").and_then(Value::as_str) {
        anyhow::bail!("LLM API error: {err}");
    }
    Ok(v)
}

/// 多轮 tool 用非流式，最终 assistant 文本用流式（与面板 delta 一致）。
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
    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
    let mut had_tool_round = false;

    for _ in 0..MAX_TOOL_ROUNDS {
        let resp = complete_chat_nonstream_blocking(
            client, &url, api_key, model, messages, tools, abort_flag,
        )?;
        let msg = resp
            .pointer("/choices/0/message")
            .cloned()
            .unwrap_or(Value::Null);
        let tool_calls = msg
            .get("tool_calls")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if !tool_calls.is_empty() {
            had_tool_round = true;
            // 与 UI 一致：同一轮里「模型先说的正文」必须落在工具块之前（此前仅进 messages，未写入 parts）。
            if let Some(s) = message_assistant_text_content(&msg) {
                if !s.is_empty() {
                    emit_nonstream_assistant_text(s.as_str(), abort_flag, &mut on_text_delta)?;
                }
            }
            messages.push(msg);
            let mut batch: Vec<(String, String, String)> = Vec::with_capacity(tool_calls.len());
            for tc in &tool_calls {
                let id = tc
                    .get("id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| anyhow!("tool_calls entry missing id"))?
                    .to_string();
                let name = tc
                    .pointer("/function/name")
                    .and_then(Value::as_str)
                    .ok_or_else(|| anyhow!("tool_calls entry missing function.name"))?
                    .to_string();
                let args = tc
                    .pointer("/function/arguments")
                    .and_then(Value::as_str)
                    .unwrap_or("{}")
                    .to_string();
                batch.push((id, name, args));
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
            continue;
        }

        if let Some(s) = message_assistant_text_content(&msg) {
            if !s.is_empty() {
                // 一旦出现过工具回调，最终正文必须走 stream，避免「工具后正文整段蹦出」。
                if had_tool_round {
                    break;
                }
                emit_nonstream_assistant_text(s.as_str(), abort_flag, &mut on_text_delta)?;
                return Ok(());
            }
        }

        messages.push(msg);
        break;
    }

    stream_chat_completion_blocking(
        client,
        base_url,
        api_key,
        model,
        messages.clone(),
        abort_flag,
        on_text_delta,
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
