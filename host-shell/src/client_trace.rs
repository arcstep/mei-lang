//! User-action markers in host terminal logs (always on at INFO).

use axum::{http::StatusCode, response::IntoResponse, Json};
use serde::Deserialize;

fn kind_label(kind: &str) -> &'static str {
    match kind.trim().to_ascii_uppercase().as_str() {
        "ROUTE" | "NAV" | "NAVIGATION" => "路由",
        "WORKSPACE_NAV" | "WORKSPACE" => "工作区导航",
        "BOARD" | "BOARD_OPEN" => "看板打开",
        "DRILLDOWN" => "下钻",
        "TAB" => "标签切换",
        "REFRESH" => "刷新",
        "INITIAL" => "首屏",
        "CACHE" => "缓存",
        _ => "操作",
    }
}

#[derive(Debug, Clone, Default)]
pub struct ClientCommandContext {
    pub id: String,
    pub kind: String,
    pub label: String,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct PageRequestObservability {
    pub ssr_emit_ms: Option<u64>,
    pub artifact_hits: crate::artifact_observability::ArtifactHitMatrix,
    pub view_revision_status: Option<&'static str>,
    pub page_cache_status: Option<&'static str>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infer_workspace_nav_for_layout() {
        assert_eq!(
            infer_page_command_kind("/apps/demo/layout?tab=preview", true),
            "WORKSPACE_NAV"
        );
    }

    #[test]
    fn decode_client_cmd_label_round_trip() {
        assert_eq!(decode_client_cmd_label(""), "");
        assert_eq!(decode_client_cmd_label("route"), "route");
        assert_eq!(
            decode_client_cmd_label("%E6%89%A7%E6%B3%95%E5%8D%95%E4%BD%8D"),
            "执法单位"
        );
        assert_eq!(
            decode_client_cmd_label("%E7%A3%81%E5%99%A8%E5%8F%A3%E8%A1%97%E9%81%93"),
            "磁器口街道"
        );
    }

    #[test]
    fn client_error_payload_keeps_structured_detail_and_truncates_log_fields() {
        let payload: ClientTracePayload = serde_json::from_value(serde_json::json!({
            "id": "client-error-1",
            "kind": "CLIENT_ERROR",
            "label": "map_render_error",
            "detail": {
                "kind": "map_render_error",
                "message": "abcdef",
                "status": 503,
                "occurrenceCount": 4
            }
        }))
        .expect("client error payload");
        let detail = payload.detail.as_ref().expect("detail");
        assert_eq!(client_detail_text(detail, "message", 4), "abcd");
        assert_eq!(client_detail_text(detail, "status", 16), "503");
        assert_eq!(client_detail_text(detail, "occurrenceCount", 16), "4");
    }
}

fn decode_client_cmd_label(raw: &str) -> String {
    let raw = raw.trim();
    if raw.is_empty() {
        return String::new();
    }
    let mut bytes = Vec::with_capacity(raw.len());
    let input = raw.as_bytes();
    let mut index = 0usize;
    while index < input.len() {
        if input[index] == b'%' && index + 2 < input.len() {
            let hi = input[index + 1];
            let lo = input[index + 2];
            if let (Some(hi), Some(lo)) = (from_hex(hi), from_hex(lo)) {
                bytes.push(hi << 4 | lo);
                index += 3;
                continue;
            }
        }
        bytes.push(input[index]);
        index += 1;
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

fn from_hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

pub fn parse_client_command_headers(
    id: Option<&str>,
    kind: Option<&str>,
    label: Option<&str>,
) -> Option<ClientCommandContext> {
    let id = id
        .map(str::trim)
        .filter(|value| !value.is_empty())?
        .to_string();
    let kind = kind.map(str::trim).unwrap_or("CMD").to_string();
    let label = label
        .map(str::trim)
        .map(decode_client_cmd_label)
        .unwrap_or_default();
    Some(ClientCommandContext { id, kind, label })
}

fn is_workspace_surface_path(path: &str) -> bool {
    let path = path.split('?').next().unwrap_or(path);
    let Some(tail) = path.strip_prefix("/apps/") else {
        return false;
    };
    let mut parts = tail.split('/').filter(|segment| !segment.is_empty());
    let _app = parts.next();
    matches!(parts.next(), Some("layout" | "prototype"))
}

pub fn infer_page_command_kind(path: &str, spa_nav: bool) -> &'static str {
    if is_workspace_surface_path(path) {
        return if spa_nav { "WORKSPACE_NAV" } else { "REFRESH" };
    }
    if spa_nav {
        return "ROUTE";
    }
    "REFRESH"
}

pub fn is_user_page_get(method: &str, path: &str) -> bool {
    method.eq_ignore_ascii_case("GET") && path.starts_with("/apps/")
}

pub fn parse_page_observability_from_headers(
    headers: &axum::http::HeaderMap,
) -> PageRequestObservability {
    let ssr_emit_ms = headers
        .get("x-mei-ssr-http-response-body-ms")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok());
    let view_revision_status = headers
        .get("x-mei-view-revision-status")
        .and_then(|value| value.to_str().ok())
        .map(|value| match value.trim() {
            "bootstrap" => "bootstrap",
            "assemble_local" => "assemble_local",
            "refetch" => "refetch",
            "local_miss" => "local_miss",
            _ => "unknown",
        });
    let page_cache_status = headers
        .get("x-mei-page-cache")
        .and_then(|value| value.to_str().ok())
        .map(|value| match value.trim() {
            "hit" => "hit",
            "miss" => "miss",
            _ => "unknown",
        });
    PageRequestObservability {
        ssr_emit_ms,
        artifact_hits: crate::artifact_observability::parse_artifact_hits_from_headers(headers),
        view_revision_status,
        page_cache_status,
    }
}

fn cache_tag(_obs: PageRequestObservability) -> &'static str {
    "ssr"
}

fn format_ssr_ms(obs: PageRequestObservability, total_ms: u128) -> String {
    if let Some(ssr_ms) = obs.ssr_emit_ms {
        return format!("{ssr_ms}ms");
    }
    format!("{total_ms}ms")
}

pub fn log_client_command_banner(ctx: &ClientCommandContext) {
    let label = kind_label(ctx.kind.as_str());
    let detail = if ctx.label.is_empty() {
        String::new()
    } else {
        format!(" · {}", ctx.label)
    };
    tracing::info!(
        target: "mei_user_cmd",
        client_cmd_id = %ctx.id,
        client_cmd_kind = %ctx.kind,
        "USER ▶ {label}{detail}"
    );
}

pub fn log_client_command_request(
    ctx: &ClientCommandContext,
    method: &str,
    uri: &str,
    status: u16,
    latency_ms: u128,
    response_bytes: u64,
    obs: PageRequestObservability,
) {
    let label = kind_label(ctx.kind.as_str());
    let size = format_bytes(response_bytes);
    let cache = cache_tag(obs);
    let ssr = format_ssr_ms(obs, latency_ms);
    let artifacts = obs.artifact_hits.summary_tag();
    let view_revision = obs.view_revision_status.unwrap_or("legacy");
    tracing::info!(
        target: "mei_user_cmd",
        client_cmd_id = %ctx.id,
        client_cmd_kind = %ctx.kind,
        "USER   ├─ {label}  {method} {uri}  → {status}  total={latency_ms}ms  ssr={ssr}  cache={cache}  view_revision={view_revision}  artifacts={artifacts}  size={size}"
    );
}

pub fn log_user_page_request(
    path: &str,
    uri: &str,
    spa_nav: bool,
    status: u16,
    latency_ms: u128,
    response_bytes: u64,
    obs: PageRequestObservability,
) {
    let kind = infer_page_command_kind(path, spa_nav);
    let label = kind_label(kind);
    let html_bytes = format_bytes(response_bytes);
    let cache = cache_tag(obs);
    let page_cache = obs.page_cache_status.unwrap_or("-");
    let ssr = format_ssr_ms(obs, latency_ms);
    let artifacts = obs.artifact_hits.summary_tag();
    let view_revision = obs.view_revision_status.unwrap_or("legacy");
    tracing::info!(
        target: "mei_user_cmd",
        route_mode = %path.split('/').nth(2).unwrap_or("-"),
        "USER ▶ {label}  GET {uri}  → {status}  total={latency_ms}ms  ssr={ssr}  cache={cache}  page_cache={page_cache}  view_revision={view_revision}  artifacts={artifacts}  html_bytes={html_bytes}"
    );
}

pub fn log_background_request(method: &str, uri: &str, status: u16, latency_ms: u128) {
    tracing::debug!(
        target: "mei_bg",
        "bg  {method} {uri} → {status} ({latency_ms}ms)"
    );
}

fn format_bytes(bytes: u64) -> String {
    if bytes >= 1_048_576 {
        format!("{:.1}MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.0}KB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes}B")
    }
}

#[derive(Debug, Deserialize)]
pub struct ClientTracePayload {
    pub id: String,
    pub kind: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub pipeline: Option<serde_json::Value>,
    #[serde(default)]
    pub detail: Option<serde_json::Value>,
}

fn pipeline_u64(value: &serde_json::Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_i64().and_then(|n| u64::try_from(n).ok()))
}

fn log_client_render_pipeline(ctx: &ClientCommandContext, pipeline: &serde_json::Value) {
    let wall_ms = pipeline.get("wallMs").and_then(pipeline_u64);
    let document_ms = pipeline.get("documentMs").and_then(pipeline_u64);
    let client_ms = pipeline.get("clientAfterDocumentMs").and_then(pipeline_u64);
    let fragment_ms = pipeline.get("previewFragmentMs").and_then(pipeline_u64);
    let assembly_ms = pipeline.get("assemblyMs").and_then(pipeline_u64);
    let surface_ms = pipeline.get("surfaceReadyMs").and_then(pipeline_u64);
    let source = pipeline
        .get("flags")
        .and_then(|flags| flags.get("source"))
        .and_then(|value| value.as_str())
        .unwrap_or("-");
    tracing::info!(
        target: "mei_user_cmd",
        client_cmd_id = %ctx.id,
        client_cmd_kind = %ctx.kind,
        wall_ms = ?wall_ms,
        document_ms = ?document_ms,
        client_ms = ?client_ms,
        fragment_ms = ?fragment_ms,
        assembly_ms = ?assembly_ms,
        surface_ms = ?surface_ms,
        source = %source,
        "USER ◀ 渲染链路  wall={wall}  document={document}  client={client}  fragment={fragment}  assembly={assembly}  surface={surface}  source={source}",
        wall = wall_ms.map(|ms| format!("{ms}ms")).unwrap_or_else(|| "-".to_string()),
        document = document_ms
            .map(|ms| format!("{ms}ms"))
            .unwrap_or_else(|| "-".to_string()),
        client = client_ms
            .map(|ms| format!("{ms}ms"))
            .unwrap_or_else(|| "-".to_string()),
        fragment = fragment_ms
            .map(|ms| format!("{ms}ms"))
            .unwrap_or_else(|| "-".to_string()),
        assembly = assembly_ms
            .map(|ms| format!("{ms}ms"))
            .unwrap_or_else(|| "-".to_string()),
        surface = surface_ms
            .map(|ms| format!("{ms}ms"))
            .unwrap_or_else(|| "-".to_string()),
    );
}

fn client_detail_text(detail: &serde_json::Value, key: &str, max_chars: usize) -> String {
    let text = detail
        .get(key)
        .and_then(|value| {
            value
                .as_str()
                .map(str::to_string)
                .or_else(|| value.as_i64().map(|number| number.to_string()))
                .or_else(|| value.as_u64().map(|number| number.to_string()))
        })
        .unwrap_or_default();
    text.chars().take(max_chars).collect()
}

fn log_client_error(ctx: &ClientCommandContext, detail: Option<&serde_json::Value>) {
    let empty = serde_json::json!({});
    let detail = detail.unwrap_or(&empty);
    let error_kind = client_detail_text(detail, "kind", 80);
    let message = client_detail_text(detail, "message", 2000);
    let app_id = client_detail_text(detail, "appId", 160);
    let scene_id = client_detail_text(detail, "sceneId", 160);
    let component = client_detail_text(detail, "component", 160);
    let panel_id = client_detail_text(detail, "panelId", 240);
    let phase = client_detail_text(detail, "phase", 120);
    let target = client_detail_text(detail, "target", 1000);
    let api = client_detail_text(detail, "api", 1000);
    let status = client_detail_text(detail, "status", 16);
    let occurrence_count = client_detail_text(detail, "occurrenceCount", 16);
    let first_occurred_at = client_detail_text(detail, "firstOccurredAt", 80);
    let last_occurred_at = client_detail_text(detail, "lastOccurredAt", 80);
    let page_url = client_detail_text(detail, "pageUrl", 1000);
    let stack = client_detail_text(detail, "stack", 2000);
    let benign_runtime_restart_fetch = error_kind == "unhandled_rejection"
        && status.parse::<u16>().unwrap_or(0) == 0
        && message.to_ascii_lowercase().contains("failed to fetch")
        && page_url.contains("/runtime");
    if benign_runtime_restart_fetch {
        tracing::debug!(
            target: "mei_client_error",
            client_error_id = %ctx.id,
            client_error_kind = %error_kind,
            page_url = %page_url,
            occurrence_count = %occurrence_count,
            "suppressed runtime-console fetch failure during Host restart"
        );
        return;
    }
    tracing::error!(
        target: "mei_client_error",
        client_error_id = %ctx.id,
        client_error_kind = %error_kind,
        app_id = %app_id,
        scene_id = %scene_id,
        component = %component,
        panel_id = %panel_id,
        phase = %phase,
        target = %target,
        api = %api,
        status = %status,
        occurrence_count = %occurrence_count,
        first_occurred_at = %first_occurred_at,
        last_occurred_at = %last_occurred_at,
        page_url = %page_url,
        stack = %stack,
        message = %message,
        "CLIENT ✖ {}",
        if ctx.label.is_empty() {
            "客户端运行失败"
        } else {
            ctx.label.as_str()
        }
    );
}

pub async fn api_host_client_trace(Json(payload): Json<ClientTracePayload>) -> impl IntoResponse {
    let id = payload.id.trim();
    if id.is_empty() {
        return (StatusCode::BAD_REQUEST, "id is required").into_response();
    }
    let ctx = ClientCommandContext {
        id: id.to_string(),
        kind: payload.kind.trim().to_string(),
        label: payload.label.trim().to_string(),
    };
    if payload.kind.eq_ignore_ascii_case("RENDER_PIPELINE") {
        if let Some(pipeline) = payload.pipeline.as_ref() {
            log_client_render_pipeline(&ctx, pipeline);
        } else {
            log_client_command_banner(&ctx);
        }
    } else if payload.kind.eq_ignore_ascii_case("CLIENT_ERROR") {
        log_client_error(&ctx, payload.detail.as_ref());
    } else {
        log_client_command_banner(&ctx);
    }
    StatusCode::NO_CONTENT.into_response()
}
