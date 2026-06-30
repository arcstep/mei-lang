use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use axum::{
    body::Body,
    extract::Request,
    http::Method,
    middleware::Next,
    response::Response,
};
use http_body_util::BodyExt;

static REQUEST_ID_SEQ: AtomicU64 = AtomicU64::new(1);

fn next_request_id() -> String {
    let id = REQUEST_ID_SEQ.fetch_add(1, Ordering::Relaxed);
    format!("req-{id:08x}")
}

fn is_noisy_success_request(method: &Method, path: &str) -> bool {
    if *method == Method::POST && path.starts_with("/api/datasets/") {
        return true;
    }
    if *method != Method::GET {
        return false;
    }
    matches!(
        path,
        "/api/host/ready" | "/api/host/readiness" | "/api/host/heartbeat" | "/api/host/version" | "/api/host/scene-revision" | "/favicon.ico"
    ) || path.starts_with("/app-assets/")
        || path.starts_with("/workspace-components/")
        || path.starts_with("/workspace-app-assets/")
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
        return (
            "page",
            path.split('/').nth(3).unwrap_or("-").to_string(),
        );
    }
    if path.starts_with("/app-bundles/")
        || path.starts_with("/app-assets/")
        || path.starts_with("/workspace-")
    {
        return ("asset", "-".to_string());
    }
    ("other", "-".to_string())
}

pub async fn log_request(request: Request, next: Next) -> Response {
    let method = request.method().clone();
    let uri = request.uri().clone();
    let path = uri.path().to_string();
    let request_id = next_request_id();
    let (route_kind, app_id) = classify_route(path.as_str());
    let started_at = Instant::now();
    let mut response = next.run(request).await;
    let status = response.status();
    let latency_ms = started_at.elapsed().as_millis() as u64;
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
    } else if !is_noisy_success_request(&method, path.as_str()) {
        tracing::info!(
            request_id = %request_id,
            route_kind = %route_kind,
            app_id = %app_id,
            status = %status,
            latency_ms,
            response_bytes,
            method = %method,
            uri = %uri,
            "request finished"
        );
    }

    response
}
