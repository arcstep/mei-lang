//! Reverse proxy from Host control plane to managed App Runtime instances.
//!
//! When an active LaunchManifest route has a reachable runtime, Host **must** proxy
//! Access / view / dataset traffic there. Legacy Host in-process / plug-ds is a
//! deprecated fallback (warn) unless `MEI_APP_RUNTIME_REQUIRED=1` (503).

use axum::body::Body;
use axum::http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use http_body::Frame;
use mei_host_auth::AuthPrincipal;
use mei_host_core::{
    HEADER_APP_ID, HEADER_GENERATION, HEADER_INSTANCE_ID, HEADER_INSTANCE_TOKEN, HEADER_PRINCIPAL,
    HEADER_SPEC_DIGEST,
};
use serde_json::json;
use std::collections::BTreeMap;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

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
    if let Some(response) = circuit_open_response(identity.app_id.as_str()) {
        return response;
    }
    let permit = match acquire_app_proxy_permit(identity.app_id.as_str()).await {
        Ok(permit) => permit,
        Err(response) => return response,
    };
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
    let response =
        match tokio::time::timeout(std::time::Duration::from_secs(30), request.send()).await {
            Ok(Ok(response)) => {
                record_proxy_transport_success(identity.app_id.as_str());
                response
            }
            Ok(Err(error)) => {
                record_proxy_transport_failure(identity.app_id.as_str());
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
            Err(_) => {
                record_proxy_transport_failure(identity.app_id.as_str());
                return (
                    StatusCode::GATEWAY_TIMEOUT,
                    axum::Json(json!({
                        "error": "app-runtime response header timeout",
                        "endpoint": identity.endpoint,
                    })),
                )
                    .into_response();
            }
        };
    let status =
        StatusCode::from_u16(response.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let response_headers = response.headers().clone();
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
        .body(Body::new(PermitBody {
            inner: Body::from_stream(response.bytes_stream()),
            _permit: permit,
        }))
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
    crate::state::runtime_identity_from_snapshot(&http.route_table, app_id, principal)
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

/// Surfaces that may fall through to Host in-process assemble when runtime is down.
pub fn surface_allows_host_without_runtime(surface: Option<&str>) -> bool {
    let slug = surface
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_else(|| "app".to_string());
    mei_lang_app::UiRouteMode::from_slug(slug.as_str()).allows_host_plane_without_runtime()
}

fn response_indicates_runtime_unavailable(response: &Response) -> bool {
    matches!(
        response.status(),
        StatusCode::SERVICE_UNAVAILABLE | StatusCode::GATEWAY_TIMEOUT
    )
}

/// When an active App Runtime route exists for `app_id`, proxy an arbitrary request.
///
/// For res-admin / config / upload (`surface`), missing or unreachable runtime falls
/// through to Host (`None`) so pages work while the app is enabled-but-unloaded.
pub async fn maybe_proxy_app_request(
    http: &crate::state::HostHttpState,
    app_id: &str,
    method: Method,
    path_and_query: &str,
    headers: &HeaderMap,
    body: Option<Vec<u8>>,
    principal: Option<AuthPrincipal>,
    surface: Option<&str>,
) -> Option<Response> {
    let host_ok = surface_allows_host_without_runtime(surface);
    let surface_label = surface.unwrap_or("app-api");
    match app_request_gateway(
        http,
        app_id,
        method,
        path_and_query,
        headers,
        body,
        principal,
        surface_label,
    )
    .await
    {
        GatewayProxyOutcome::Proxied(response) => {
            if host_ok && response_indicates_runtime_unavailable(&response) {
                tracing::info!(
                    app_id = %app_id,
                    surface = %surface_label,
                    status = %response.status(),
                    "res-admin host plane: runtime proxy unavailable — Host assemble fallback"
                );
                return None;
            }
            Some(response)
        }
        GatewayProxyOutcome::RequiredUnavailable(response) => {
            if host_ok {
                tracing::info!(
                    app_id = %app_id,
                    surface = %surface_label,
                    "res-admin host plane: app-runtime not loaded — Host assemble fallback"
                );
                return None;
            }
            Some(response)
        }
        GatewayProxyOutcome::LegacyFallback => None,
    }
}

fn app_runtime_proxy_client() -> &'static reqwest::Client {
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(5))
            .build()
            .expect("app-runtime proxy reqwest client")
    })
}

struct PermitBody {
    inner: Body,
    _permit: OwnedSemaphorePermit,
}

impl axum::body::HttpBody for PermitBody {
    type Data = axum::body::Bytes;
    type Error = axum::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        Pin::new(&mut self.inner).poll_frame(cx)
    }

    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }
}

fn app_proxy_limit() -> usize {
    std::env::var("MEI_HOST_APP_PROXY_CONCURRENCY")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(64)
}

fn app_proxy_semaphore(app_id: &str) -> Arc<Semaphore> {
    static LIMITERS: std::sync::OnceLock<Mutex<BTreeMap<String, Arc<Semaphore>>>> =
        std::sync::OnceLock::new();
    let mut guard = LIMITERS
        .get_or_init(|| Mutex::new(BTreeMap::new()))
        .lock()
        .expect("app proxy limiter lock");
    guard
        .entry(app_id.to_string())
        .or_insert_with(|| Arc::new(Semaphore::new(app_proxy_limit())))
        .clone()
}

async fn acquire_app_proxy_permit(app_id: &str) -> Result<OwnedSemaphorePermit, Response> {
    match tokio::time::timeout(
        std::time::Duration::from_millis(250),
        app_proxy_semaphore(app_id).acquire_owned(),
    )
    .await
    {
        Ok(Ok(permit)) => Ok(permit),
        _ => Err((
            StatusCode::TOO_MANY_REQUESTS,
            [("retry-after", "1")],
            axum::Json(json!({
                "error": "app-runtime proxy concurrency limit reached",
                "appId": app_id,
            })),
        )
            .into_response()),
    }
}

#[derive(Default)]
struct CircuitState {
    consecutive_failures: u32,
    open_until_ms: u64,
}

fn proxy_circuits() -> &'static Mutex<BTreeMap<String, CircuitState>> {
    static CIRCUITS: std::sync::OnceLock<Mutex<BTreeMap<String, CircuitState>>> =
        std::sync::OnceLock::new();
    CIRCUITS.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn circuit_open_response(app_id: &str) -> Option<Response> {
    let now = crate::state::current_time_ms();
    let mut guard = proxy_circuits().lock().expect("proxy circuit lock");
    let state = guard.get_mut(app_id)?;
    if state.open_until_ms <= now {
        state.open_until_ms = 0;
        return None;
    }
    Some(
        (
            StatusCode::SERVICE_UNAVAILABLE,
            [("retry-after", "2")],
            axum::Json(json!({
                "error": "app-runtime proxy circuit open",
                "appId": app_id,
                "retryAtMs": state.open_until_ms,
            })),
        )
            .into_response(),
    )
}

fn record_proxy_transport_failure(app_id: &str) {
    let mut guard = proxy_circuits().lock().expect("proxy circuit lock");
    let state = guard.entry(app_id.to_string()).or_default();
    state.consecutive_failures = state.consecutive_failures.saturating_add(1);
    if state.consecutive_failures >= 5 {
        state.open_until_ms = crate::state::current_time_ms().saturating_add(5_000);
    }
}

fn record_proxy_transport_success(app_id: &str) {
    let mut guard = proxy_circuits().lock().expect("proxy circuit lock");
    guard.remove(app_id);
    crate::app_enable::record_app_activity(app_id);
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
    use axum::routing::get;
    use axum::Router;
    use http_body_util::BodyExt;
    use mei_host_auth::{AuthPrincipal, AuthRole};
    use std::collections::{BTreeMap, BTreeSet};
    use std::sync::Mutex;
    use std::time::Duration;

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
    fn surface_allows_host_without_runtime_for_res_admin_planes() {
        assert!(surface_allows_host_without_runtime(Some("admin")));
        assert!(surface_allows_host_without_runtime(Some("config")));
        assert!(surface_allows_host_without_runtime(Some("upload")));
        assert!(surface_allows_host_without_runtime(Some("Admin")));
        assert!(!surface_allows_host_without_runtime(Some("app")));
        assert!(!surface_allows_host_without_runtime(None));
        assert!(!surface_allows_host_without_runtime(Some("layout")));
    }

    #[test]
    fn response_indicates_runtime_unavailable_statuses() {
        let unavailable = Response::builder()
            .status(StatusCode::SERVICE_UNAVAILABLE)
            .body(Body::empty())
            .expect("response");
        assert!(response_indicates_runtime_unavailable(&unavailable));
        let timeout = Response::builder()
            .status(StatusCode::GATEWAY_TIMEOUT)
            .body(Body::empty())
            .expect("response");
        assert!(response_indicates_runtime_unavailable(&timeout));
        let ok = Response::builder()
            .status(StatusCode::OK)
            .body(Body::empty())
            .expect("response");
        assert!(!response_indicates_runtime_unavailable(&ok));
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

    #[test]
    fn circuit_breaker_is_scoped_per_app_and_resets_on_success() {
        let app_a = "circuit-test-a";
        let app_b = "circuit-test-b";
        record_proxy_transport_success(app_a);
        record_proxy_transport_success(app_b);
        for _ in 0..5 {
            record_proxy_transport_failure(app_a);
        }
        assert!(circuit_open_response(app_a).is_some());
        assert!(circuit_open_response(app_b).is_none());
        record_proxy_transport_success(app_a);
        assert!(circuit_open_response(app_a).is_none());
    }

    #[tokio::test]
    async fn blackhole_runtime_does_not_block_another_app_proxy() {
        let blackhole = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("blackhole bind");
        let blackhole_addr = blackhole.local_addr().expect("blackhole addr");
        let blackhole_task = tokio::spawn(async move {
            let (_socket, _) = blackhole.accept().await.expect("blackhole accept");
            std::future::pending::<()>().await;
        });

        let fast = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("fast bind");
        let fast_addr = fast.local_addr().expect("fast addr");
        let fast_task = tokio::spawn(async move {
            axum::serve(
                fast,
                Router::new().route("/ready", get(|| async { "fast-app-ready" })),
            )
            .await
            .expect("fast server");
        });

        let mut slow_identity = sample_identity();
        slow_identity.endpoint = format!("http://{blackhole_addr}");
        slow_identity.app_id = "slow-app".to_string();
        let slow_task =
            tokio::spawn(async move { proxy_get(&slow_identity, "/never", None).await });
        tokio::time::sleep(Duration::from_millis(30)).await;

        let mut fast_identity = sample_identity();
        fast_identity.endpoint = format!("http://{fast_addr}");
        fast_identity.app_id = "fast-app".to_string();
        let response = tokio::time::timeout(
            Duration::from_millis(500),
            proxy_get(&fast_identity, "/ready", None),
        )
        .await
        .expect("fast app must not wait for blackhole app");
        assert_eq!(response.status(), StatusCode::OK);
        let body = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        assert_eq!(body.as_ref(), b"fast-app-ready");

        slow_task.abort();
        blackhole_task.abort();
        fast_task.abort();
    }

    #[tokio::test]
    async fn proxy_returns_headers_without_collecting_stream_body() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("stream bind");
        let addr = listener.local_addr().expect("stream addr");
        let server = tokio::spawn(async move {
            let app = Router::new().route(
                "/stream",
                get(|| async {
                    let stream = async_stream::stream! {
                        yield Ok::<_, std::io::Error>(axum::body::Bytes::from_static(b"first"));
                        tokio::time::sleep(Duration::from_secs(2)).await;
                        yield Ok::<_, std::io::Error>(axum::body::Bytes::from_static(b"second"));
                    };
                    Body::from_stream(stream)
                }),
            );
            axum::serve(listener, app).await.expect("stream server");
        });

        let mut identity = sample_identity();
        identity.endpoint = format!("http://{addr}");
        let response = tokio::time::timeout(
            Duration::from_millis(500),
            proxy_get(&identity, "/stream", None),
        )
        .await
        .expect("proxy must return after upstream headers");
        assert_eq!(response.status(), StatusCode::OK);
        server.abort();
    }
}
