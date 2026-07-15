use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use axum::{body::Body, extract::Request, http::Method, middleware::Next, response::Response};
use http_body_util::BodyExt;

static REQUEST_ID_SEQ: AtomicU64 = AtomicU64::new(1);

fn next_request_id() -> String {
    let id = REQUEST_ID_SEQ.fetch_add(1, Ordering::Relaxed);
    format!("req-{id:08x}")
}

fn is_noisy_success_request(method: &Method, path: &str) -> bool {
    if path == "/api/host/client-trace" {
        return true;
    }
    if *method == Method::POST && path.starts_with("/api/datasets/") {
        return true;
    }
    if *method != Method::GET {
        return false;
    }
    matches!(
        path,
        "/api/host/ready"
            | "/api/host/readiness"
            | "/api/host/heartbeat"
            | "/api/host/version"
            | "/host/starting"
            | "/favicon.ico"
            | "/login"
            | "/logout"
    ) || path.starts_with("/api/runtime/snapshot")
        || path.starts_with("/app-assets/")
        || path.starts_with("/app-bundles/")
        || path.starts_with("/workspace-components/")
        || path.starts_with("/workspace-app-assets/")
        || path.starts_with("/gis/")
}

/// 预期内的客户端探测 404（非业务故障），避免刷 WARN。
fn is_expected_probe_not_found(method: &Method, path: &str) -> bool {
    if *method != Method::GET {
        return false;
    }
    path == "/favicon.ico"
        || path == "/.well-known/appspecific/com.chrome.devtools.json"
        || path.starts_with("/.well-known/")
}

fn is_background_poll_request(method: &Method, path: &str) -> bool {
    if *method != Method::GET {
        return false;
    }
    path.starts_with("/api/agent/")
        || path.starts_with("/api/runtime/snapshot")
        || path.starts_with("/api/host/heartbeat")
        || path.starts_with("/gis/")
}

/// Open-ended streams must not be buffered — `BodyExt::collect` never completes for SSE.
fn is_streaming_response(path: &str, headers: &axum::http::HeaderMap) -> bool {
    if path == "/api/host/events" || path.ends_with("/events") {
        return true;
    }
    headers
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|content_type| {
            content_type
                .split(';')
                .next()
                .map(str::trim)
                .is_some_and(|mime| mime.eq_ignore_ascii_case("text/event-stream"))
        })
}

fn classify_route(path: &str) -> (&'static str, String) {
    if path.starts_with("/api/datasets/") {
        return ("api", "datasets".to_string());
    }
    if path.starts_with("/api/") {
        return (
            "api",
            path.trim_start_matches("/api/")
                .split('/')
                .next()
                .unwrap_or("api")
                .to_string(),
        );
    }
    if path.starts_with("/apps/") {
        return ("page", path.split('/').nth(3).unwrap_or("-").to_string());
    }
    if path.starts_with("/app-bundles/")
        || path.starts_with("/app-assets/")
        || path.starts_with("/workspace-")
    {
        return ("asset", "-".to_string());
    }
    ("other", "-".to_string())
}

fn header_flag(headers: &axum::http::HeaderMap, name: &str) -> bool {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(|value| {
            let trimmed = value.trim();
            trimmed == "1" || trimmed.eq_ignore_ascii_case("true")
        })
        .unwrap_or(false)
}

pub async fn log_request(request: Request, next: Next) -> Response {
    let method = request.method().clone();
    let uri = request.uri().clone();
    let path = uri.path().to_string();
    let request_id = next_request_id();
    let (route_kind, app_id) = classify_route(path.as_str());
    let spa_nav = header_flag(request.headers(), "x-mei-spa-nav");
    let client_cmd = crate::client_trace::parse_client_command_headers(
        request
            .headers()
            .get("x-mei-client-cmd-id")
            .and_then(|value| value.to_str().ok()),
        request
            .headers()
            .get("x-mei-client-cmd-kind")
            .and_then(|value| value.to_str().ok()),
        request
            .headers()
            .get("x-mei-client-cmd-label")
            .and_then(|value| value.to_str().ok()),
    );
    let started_at = Instant::now();
    let mut response = next.run(request).await;
    let status = response.status();
    let latency_ms = started_at.elapsed().as_millis() as u64;
    let uri_text = uri.to_string();

    // SSE / long-lived streams: pass body through. Collecting would hang forever and
    // the client would never even receive response headers (DevTools stays "pending").
    if is_streaming_response(path.as_str(), response.headers()) {
        tracing::debug!(
            request_id = %request_id,
            route_kind = %route_kind,
            app_id = %app_id,
            status = %status,
            latency_ms,
            method = %method,
            uri = %uri,
            "streaming response opened (body not collected)"
        );
        return response;
    }

    let (parts, body) = response.into_parts();
    let body_bytes = match body.collect().await {
        Ok(buffer) => buffer.to_bytes(),
        Err(error) => {
            tracing::warn!(
                request_id = %request_id,
                error = %error,
                "failed to collect response body for request trace"
            );
            axum::body::Bytes::new()
        }
    };
    let response_bytes = body_bytes.len() as u64;
    response = Response::from_parts(parts, Body::from(body_bytes));
    let page_obs = crate::client_trace::parse_page_observability_from_headers(response.headers());

    if status.is_server_error() {
        tracing::error!(
            request_id = %request_id,
            route_kind = %route_kind,
            app_id = %app_id,
            status = %status,
            latency_ms,
            response_bytes,
            method = %method,
            uri = %uri,
            "request finished with error status"
        );
    } else if status.is_client_error() {
        if status.as_u16() == 404 && is_expected_probe_not_found(&method, path.as_str()) {
            tracing::debug!(
                request_id = %request_id,
                route_kind = %route_kind,
                status = %status,
                method = %method,
                uri = %uri,
                "expected probe not found (suppressed warn)"
            );
        } else {
            tracing::warn!(
                request_id = %request_id,
                route_kind = %route_kind,
                app_id = %app_id,
                status = %status,
                latency_ms,
                response_bytes,
                method = %method,
                uri = %uri,
                "request finished with client error status"
            );
        }
    } else if let Some(cmd) = client_cmd.as_ref() {
        crate::client_trace::log_client_command_request(
            cmd,
            method.as_str(),
            uri_text.as_str(),
            status.as_u16(),
            latency_ms as u128,
            response_bytes,
            page_obs,
        );
    } else if crate::client_trace::is_user_page_get(method.as_str(), path.as_str()) {
        crate::client_trace::log_user_page_request(
            path.as_str(),
            uri_text.as_str(),
            spa_nav,
            status.as_u16(),
            latency_ms as u128,
            response_bytes,
            page_obs,
        );
    } else if is_background_poll_request(&method, path.as_str()) {
        crate::client_trace::log_background_request(
            method.as_str(),
            uri_text.as_str(),
            status.as_u16(),
            latency_ms as u128,
        );
    } else if is_noisy_success_request(&method, path.as_str()) {
        // intentionally silent at INFO
    } else {
        crate::client_trace::log_background_request(
            method.as_str(),
            uri_text.as_str(),
            status.as_u16(),
            latency_ms as u128,
        );
    }

    response
}
