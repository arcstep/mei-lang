//! User-action markers in host terminal logs (always on at INFO).

use axum::{http::StatusCode, response::IntoResponse, Json};
use serde::Deserialize;

fn kind_label(kind: &str) -> &'static str {
    match kind.trim().to_ascii_uppercase().as_str() {
        "ROUTE" | "NAV" | "NAVIGATION" => "路由",
        "BUILD_NAV" | "BUILD" => "开发导航",
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
    pub page_render_cache_hit: bool,
    pub ssr_emit_ms: Option<u64>,
    pub artifact_hits: crate::artifact_observability::ArtifactHitMatrix,
}

pub fn parse_client_command_headers(
    id: Option<&str>,
    kind: Option<&str>,
    label: Option<&str>,
) -> Option<ClientCommandContext> {
    let id = id.map(str::trim).filter(|value| !value.is_empty())?.to_string();
    let kind = kind.map(str::trim).unwrap_or("CMD").to_string();
    let label = label.map(str::trim).unwrap_or("").to_string();
    Some(ClientCommandContext { id, kind, label })
}

pub fn infer_page_command_kind(path: &str, spa_nav: bool) -> &'static str {
    if path.starts_with("/apps/build/") || path.starts_with("/apps/manage/") {
        return if spa_nav { "BUILD_NAV" } else { "REFRESH" };
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
    let page_render_cache_hit = headers
        .get("x-mei-page-render-cache-hit")
        .and_then(|value| value.to_str().ok())
        .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let ssr_emit_ms = headers
        .get("x-mei-ssr-http-response-body-ms")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok());
    PageRequestObservability {
        page_render_cache_hit,
        ssr_emit_ms,
        artifact_hits: crate::artifact_observability::parse_artifact_hits_from_headers(headers),
    }
}

/// Host SSR page-render-cache (memory/disk template), not browser fragment cache.
fn cache_tag(obs: PageRequestObservability) -> &'static str {
    if obs.page_render_cache_hit {
        "ssr-hit"
    } else {
        "miss"
    }
}

fn format_ssr_ms(obs: PageRequestObservability, total_ms: u128) -> String {
    if let Some(ssr_ms) = obs.ssr_emit_ms {
        return format!("{ssr_ms}ms");
    }
    if obs.page_render_cache_hit {
        return "0ms".to_string();
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
    tracing::info!(
        target: "mei_user_cmd",
        client_cmd_id = %ctx.id,
        client_cmd_kind = %ctx.kind,
        page_render_cache_hit = obs.page_render_cache_hit,
        "USER   ├─ {label}  {method} {uri}  → {status}  total={latency_ms}ms  ssr={ssr}  cache={cache}  artifacts={artifacts}  size={size}"
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
    let size = format_bytes(response_bytes);
    let cache = cache_tag(obs);
    let ssr = format_ssr_ms(obs, latency_ms);
    let artifacts = obs.artifact_hits.summary_tag();
    tracing::info!(
        target: "mei_user_cmd",
        page_render_cache_hit = obs.page_render_cache_hit,
        route_mode = %path.split('/').nth(2).unwrap_or("-"),
        "USER ▶ {label}  GET {uri}  → {status}  total={latency_ms}ms  ssr={ssr}  cache={cache}  artifacts={artifacts}  size={size}"
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
    log_client_command_banner(&ctx);
    StatusCode::NO_CONTENT.into_response()
}
