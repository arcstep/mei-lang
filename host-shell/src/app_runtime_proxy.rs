//! Reverse proxy from Host control plane to managed App Runtime instances.
//!
//! When an active LaunchManifest route has a reachable runtime, Host **must** proxy
//! Access / view / dataset traffic there. Legacy Host in-process / plug-ds is a
//! deprecated fallback (warn) unless `MEI_APP_RUNTIME_REQUIRED=1` (503).

use axum::body::Body;
use axum::http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use mei_host_auth::AuthPrincipal;
use mei_host_core::{
    HEADER_APP_ID, HEADER_GENERATION, HEADER_INSTANCE_ID, HEADER_INSTANCE_TOKEN, HEADER_PRINCIPAL,
    HEADER_SPEC_DIGEST,
};
use serde_json::json;

use crate::legacy_compat::{
    decide_data_plane_gate, runtime_required_unavailable_response, warn_legacy_data_plane_fallback,
    DataPlaneGate,
};

/// Identity forwarded Host → Runtime after Host auth succeeds.
#[derive(Debug, Clone)]
pub struct RuntimeProxyIdentity {
    pub endpoint: String,
    pub token: String,
    pub instance_id: String,
    pub app_id: String,
    pub generation: String,
    pub spec_digest: String,
    pub principal: Option<AuthPrincipal>,
}

pub fn principal_header_value(principal: &AuthPrincipal) -> String {
    serde_json::to_string(&json!({
        "id": principal.username,
        "role": principal.role_slug(),
    }))
    .unwrap_or_else(|_| {
        format!(
            r#"{{"id":"{}","role":"{}"}}"#,
            principal.username,
            principal.role_slug()
        )
    })
}

pub fn inject_runtime_headers(
    headers: &mut reqwest::header::HeaderMap,
    identity: &RuntimeProxyIdentity,
) {
    let set = |map: &mut reqwest::header::HeaderMap, name: &str, value: &str| {
        if value.trim().is_empty() {
            return;
        }
        if let (Ok(name), Ok(value)) = (
            reqwest::header::HeaderName::from_bytes(name.as_bytes()),
            reqwest::header::HeaderValue::from_str(value),
        ) {
            map.insert(name, value);
        }
    };
    set(headers, HEADER_INSTANCE_TOKEN, identity.token.as_str());
    set(headers, HEADER_INSTANCE_ID, identity.instance_id.as_str());
    set(headers, HEADER_APP_ID, identity.app_id.as_str());
    set(headers, HEADER_GENERATION, identity.generation.as_str());
    set(headers, HEADER_SPEC_DIGEST, identity.spec_digest.as_str());
    if let Some(principal) = identity.principal.as_ref() {
        set(
            headers,
            HEADER_PRINCIPAL,
            principal_header_value(principal).as_str(),
        );
    }
}

pub async fn proxy_request(
    method: Method,
    identity: &RuntimeProxyIdentity,
    path_and_query: &str,
    inbound_headers: Option<&HeaderMap>,
    body: Option<Vec<u8>>,
) -> Response {
    let url = join_url(identity.endpoint.as_str(), path_and_query);
    let client = app_runtime_proxy_client();
    let mut request = client.request(
        reqwest::Method::from_bytes(method.as_str().as_bytes()).unwrap_or(reqwest::Method::GET),
        url.as_str(),
    );
    let mut headers = reqwest::header::HeaderMap::new();
    if let Some(inbound) = inbound_headers {
        for (name, value) in inbound.iter() {
            let lower = name.as_str().to_ascii_lowercase();
            if lower == "host"
                || lower == "content-length"
                || lower.starts_with("x-mei-instance-")
                || lower == HEADER_PRINCIPAL
                || lower == HEADER_APP_ID
                || lower == HEADER_GENERATION
                || lower == HEADER_SPEC_DIGEST
            {
                continue;
            }
            if let (Ok(n), Ok(v)) = (
                reqwest::header::HeaderName::from_bytes(name.as_str().as_bytes()),
                reqwest::header::HeaderValue::from_bytes(value.as_bytes()),
            ) {
                headers.append(n, v);
            }
        }
    }
    inject_runtime_headers(&mut headers, identity);
    request = request.headers(headers);
    if let Some(body) = body {
        request = request.body(body);
    }
    let response = match request.send().await {
        Ok(response) => response,
        Err(error) => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                axum::Json(json!({
                    "error": "app-runtime unreachable",
                    "endpoint": identity.endpoint,
                    "detail": error.to_string(),
                })),
            )
                .into_response();
        }
    };
    let status =
        StatusCode::from_u16(response.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let response_headers = response.headers().clone();
    let bytes = match response.bytes().await {
        Ok(bytes) => bytes,
        Err(error) => {
            return (
                StatusCode::BAD_GATEWAY,
                axum::Json(json!({
                    "error": "app-runtime response read failed",
                    "detail": error.to_string(),
                })),
            )
                .into_response();
        }
    };
    let mut builder = Response::builder().status(status);
    for (name, value) in response_headers.iter() {
        let lower = name.as_str().to_ascii_lowercase();
        if lower == "transfer-encoding" || lower == "connection" {
            continue;
        }
        if let (Ok(n), Ok(v)) = (
            HeaderName::from_bytes(name.as_str().as_bytes()),
            HeaderValue::from_bytes(value.as_bytes()),
        ) {
            builder = builder.header(n, v);
        }
    }
    builder
        .body(Body::from(bytes))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

pub async fn proxy_get(
    identity: &RuntimeProxyIdentity,
    path_and_query: &str,
    inbound_headers: Option<&HeaderMap>,
) -> Response {
    proxy_request(Method::GET, identity, path_and_query, inbound_headers, None).await
}

pub async fn proxy_post_json(
    identity: &RuntimeProxyIdentity,
    path: &str,
    body: serde_json::Value,
) -> Response {
    let bytes = serde_json::to_vec(&body).unwrap_or_default();
    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    proxy_request(Method::POST, identity, path, Some(&headers), Some(bytes)).await
}

pub fn join_url(base: &str, path: &str) -> String {
    let base = base.trim_end_matches('/');
    if path.is_empty() {
        return base.to_string();
    }
    if path.starts_with('?') {
        return format!("{base}/{path}");
    }
    let path = path.trim_start_matches('/');
    format!("{base}/{path}")
}

/// Outcome of an Access / app-scoped API gateway decision.
#[derive(Debug)]
pub enum GatewayProxyOutcome {
    /// Proxied to reachable app-runtime (or built-in error from proxy transport).
    Proxied(Response),
    /// `MEI_APP_RUNTIME_REQUIRED` and no runtime → 503.
    RequiredUnavailable(Response),
    /// No runtime; caller may use Host in-process / plug-ds (already warned).
    LegacyFallback,
}

fn resolve_identity(
    http: &crate::state::HostHttpState,
    app_id: &str,
    principal: Option<AuthPrincipal>,
) -> Option<RuntimeProxyIdentity> {
    let shell = http.shell.read().ok()?;
    let supervisor = http.app_runtime.lock().ok()?;
    crate::state::runtime_identity_for_app(&shell, &supervisor, app_id, principal)
}

/// When an active App Runtime route exists for `app_id`, proxy the Access GET.
/// Returns [`GatewayProxyOutcome`] so callers honor `MEI_APP_RUNTIME_REQUIRED`.
pub async fn access_get_gateway(
    http: &crate::state::HostHttpState,
    app_id: &str,
    path_and_query: &str,
    headers: &HeaderMap,
    principal: Option<AuthPrincipal>,
    surface: &str,
) -> GatewayProxyOutcome {
    let identity = resolve_identity(http, app_id, principal);
    match decide_data_plane_gate(identity.is_some()) {
        DataPlaneGate::PreferRuntime => {
            let identity = identity.expect("PreferRuntime implies identity");
            GatewayProxyOutcome::Proxied(proxy_get(&identity, path_and_query, Some(headers)).await)
        }
        DataPlaneGate::RuntimeRequired => GatewayProxyOutcome::RequiredUnavailable(
            runtime_required_unavailable_response(app_id, surface),
        ),
        DataPlaneGate::AllowLegacyFallback => {
            warn_legacy_data_plane_fallback(app_id, surface);
            GatewayProxyOutcome::LegacyFallback
        }
    }
}

/// When an active App Runtime route exists for `app_id`, proxy an arbitrary request.
pub async fn app_request_gateway(
    http: &crate::state::HostHttpState,
    app_id: &str,
    method: Method,
    path_and_query: &str,
    headers: &HeaderMap,
    body: Option<Vec<u8>>,
    principal: Option<AuthPrincipal>,
    surface: &str,
) -> GatewayProxyOutcome {
    let identity = resolve_identity(http, app_id, principal);
    match decide_data_plane_gate(identity.is_some()) {
        DataPlaneGate::PreferRuntime => {
            let identity = identity.expect("PreferRuntime implies identity");
            GatewayProxyOutcome::Proxied(
                proxy_request(method, &identity, path_and_query, Some(headers), body).await,
            )
        }
        DataPlaneGate::RuntimeRequired => GatewayProxyOutcome::RequiredUnavailable(
            runtime_required_unavailable_response(app_id, surface),
        ),
        DataPlaneGate::AllowLegacyFallback => {
            warn_legacy_data_plane_fallback(app_id, surface);
            GatewayProxyOutcome::LegacyFallback
        }
    }
}

/// When an active App Runtime route exists for `app_id`, proxy an arbitrary request.
pub async fn maybe_proxy_app_request(
    http: &crate::state::HostHttpState,
    app_id: &str,
    method: Method,
    path_and_query: &str,
    headers: &HeaderMap,
    body: Option<Vec<u8>>,
    principal: Option<AuthPrincipal>,
) -> Option<Response> {
    match app_request_gateway(
        http,
        app_id,
        method,
        path_and_query,
        headers,
        body,
        principal,
        "app-api",
    )
    .await
    {
        GatewayProxyOutcome::Proxied(response) => Some(response),
        GatewayProxyOutcome::RequiredUnavailable(response) => Some(response),
        GatewayProxyOutcome::LegacyFallback => None,
    }
}

fn app_runtime_proxy_client() -> &'static reqwest::Client {
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .build()
            .expect("app-runtime proxy reqwest client")
    })
}

/// Prefer App Runtime endpoint for datasets; fallback to legacy plug-ds.
pub fn resolve_datasets_proxy_target(
    app_id: &str,
    app_runtime: Option<&RuntimeProxyIdentity>,
    plug_ds_endpoint: Option<&str>,
) -> DatasetsProxyTarget {
    match decide_data_plane_gate(app_runtime.is_some()) {
        DataPlaneGate::PreferRuntime => DatasetsProxyTarget::AppRuntime(
            app_runtime.expect("PreferRuntime implies identity").clone(),
        ),
        DataPlaneGate::RuntimeRequired => DatasetsProxyTarget::RuntimeRequired,
        DataPlaneGate::AllowLegacyFallback => {
            if let Some(endpoint) = plug_ds_endpoint.map(str::trim).filter(|v| !v.is_empty()) {
                warn_legacy_data_plane_fallback(app_id, "datasets/plug-ds");
                return DatasetsProxyTarget::PlugDs(endpoint.to_string());
            }
            DatasetsProxyTarget::None
        }
    }
}

#[derive(Debug, Clone)]
pub enum DatasetsProxyTarget {
    AppRuntime(RuntimeProxyIdentity),
    PlugDs(String),
    /// `MEI_APP_RUNTIME_REQUIRED` and no reachable runtime.
    RuntimeRequired,
    None,
}

#[cfg(test)]
mod tests {
    use super::*;
    use mei_host_auth::{AuthPrincipal, AuthRole};
    use std::collections::{BTreeMap, BTreeSet};
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn sample_identity() -> RuntimeProxyIdentity {
        RuntimeProxyIdentity {
            endpoint: "http://127.0.0.1:9".to_string(),
            token: "tok".to_string(),
            instance_id: "inst-1".to_string(),
            app_id: "mini-data".to_string(),
            generation: "WS-1".to_string(),
            spec_digest: "abc".to_string(),
            principal: Some(AuthPrincipal {
                username: "alice".into(),
                profile: String::new(),
                role: AuthRole::Admin,
                app_allowlist: BTreeSet::new(),
                app_denylist: BTreeSet::new(),
                scene_allowlist: BTreeMap::new(),
                session_exp: 0,
            }),
        }
    }

    #[test]
    fn inject_runtime_headers_sets_identity_and_principal() {
        let identity = sample_identity();
        let mut headers = reqwest::header::HeaderMap::new();
        inject_runtime_headers(&mut headers, &identity);
        assert_eq!(
            headers
                .get(HEADER_INSTANCE_TOKEN)
                .and_then(|v| v.to_str().ok()),
            Some("tok")
        );
        assert_eq!(
            headers.get(HEADER_APP_ID).and_then(|v| v.to_str().ok()),
            Some("mini-data")
        );
        assert_eq!(
            headers
                .get(HEADER_INSTANCE_ID)
                .and_then(|v| v.to_str().ok()),
            Some("inst-1")
        );
        let principal = headers
            .get(HEADER_PRINCIPAL)
            .and_then(|v| v.to_str().ok())
            .expect("principal");
        assert!(principal.contains("alice"));
        assert!(principal.contains("admin"));
    }

    #[test]
    fn datasets_proxy_prefers_runtime_then_plug_ds() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        std::env::remove_var("MEI_APP_RUNTIME_REQUIRED");
        std::env::set_var("MEI_APP_RUNTIME_ALLOW_LEGACY", "1");
        let identity = sample_identity();
        match resolve_datasets_proxy_target(
            "mini-data",
            Some(&identity),
            Some("http://127.0.0.1:1"),
        ) {
            DatasetsProxyTarget::AppRuntime(rt) => assert_eq!(rt.endpoint, identity.endpoint),
            other => panic!("expected runtime, got {other:?}"),
        }
        match resolve_datasets_proxy_target("mini-data", None, Some("http://127.0.0.1:1")) {
            DatasetsProxyTarget::PlugDs(endpoint) => {
                assert_eq!(endpoint, "http://127.0.0.1:1")
            }
            other => panic!("expected plug-ds, got {other:?}"),
        }
        assert!(matches!(
            resolve_datasets_proxy_target("mini-data", None, None),
            DatasetsProxyTarget::None
        ));
        std::env::remove_var("MEI_APP_RUNTIME_ALLOW_LEGACY");
    }

    #[test]
    fn datasets_proxy_required_without_runtime() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        std::env::remove_var("MEI_APP_RUNTIME_ALLOW_LEGACY");
        assert!(matches!(
            resolve_datasets_proxy_target("mini-data", None, Some("http://127.0.0.1:1")),
            DatasetsProxyTarget::RuntimeRequired
        ));
    }

    #[test]
    fn join_url_keeps_query() {
        assert_eq!(
            join_url("http://127.0.0.1:9", "/api/host/view-revision?app_id=a"),
            "http://127.0.0.1:9/api/host/view-revision?app_id=a"
        );
    }
}
