use super::prelude::*;

pub(crate) static REQUEST_ID_SEQ: AtomicU64 = AtomicU64::new(1);
pub(crate) fn is_noisy_success_request(method: &Method, uri: &Uri) -> bool {
    let path = uri.path();
    if *method == Method::POST && path.starts_with("/api/datasets/query/") {
        return true;
    }
    if *method != Method::GET {
        return false;
    }
    matches!(
        path,
        "/api/agent/config"
            | "/api/agent/runtime"
            | "/api/agent/skill"
            | "/api/agent/health"
            | "/api/agent/session"
            | "/api/host/ready"
            | "/api/host/heartbeat"
            | "/favicon.ico"
    ) || path.starts_with("/app-assets/")
        || path.starts_with("/workspace-components/")
        || path.starts_with("/gis/")
        || path.ends_with("/events")
        || path.contains("/messages")
}

pub(crate) fn next_request_id() -> String {
    let id = REQUEST_ID_SEQ.fetch_add(1, Ordering::Relaxed);
    format!("req-{id:08x}")
}

pub(crate) fn is_expected_auth_client_error(uri: &Uri, status: StatusCode) -> bool {
    if status != StatusCode::UNAUTHORIZED && status != StatusCode::FORBIDDEN {
        return false;
    }
    let path = uri.path();
    path.starts_with("/api/agent/") || path == "/api/auth/session"
}

pub(crate) async fn log_request(request: Request<Body>, next: Next) -> Response {
    let method = request.method().clone();
    let uri = request.uri().clone();
    let request_id = next_request_id();
    let request_bytes = crate::http::request_trace::request_content_length(request.headers());
    let (route_kind, app_id) = crate::http::request_trace::classify_route(&method, &uri);
    let span = tracing::info_span!(
        "http_request",
        request_id = %request_id,
        route_kind = %route_kind,
        app_id = %app_id,
        method = %method,
        uri = %uri
    );
    let started_at = Instant::now();
    let mut response = next.run(request).instrument(span).await;
    let status = response.status();
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
    let latency_ms = started_at.elapsed().as_millis();
    if let Ok(value) = HeaderValue::from_str(&request_id) {
        response
            .headers_mut()
            .insert(HeaderName::from_static("x-mei-request-id"), value);
    }

    crate::http::request_trace::record_request(
        &request_id,
        &method,
        &uri,
        &route_kind,
        &app_id,
        status,
        latency_ms,
        request_bytes,
        response_bytes,
    );

    if status.is_server_error() {
        tracing::error!(
            request_id = %request_id,
            route_kind = %route_kind,
            app_id = %app_id,
            status = %status,
            latency_ms,
            request_bytes,
            response_bytes,
            method = %method,
            uri = %uri,
            "request finished with error status"
        );
    } else if status.is_client_error() {
        if is_expected_auth_client_error(&uri, status) {
            tracing::debug!(
                request_id = %request_id,
                route_kind = %route_kind,
                app_id = %app_id,
                status = %status,
                latency_ms,
                request_bytes,
                response_bytes,
                method = %method,
                uri = %uri,
                "request finished with expected auth client error"
            );
        } else {
            tracing::warn!(
                request_id = %request_id,
                route_kind = %route_kind,
                app_id = %app_id,
                status = %status,
                latency_ms,
                request_bytes,
                response_bytes,
                method = %method,
                uri = %uri,
                "request finished with client error status"
            );
        }
    } else if !is_noisy_success_request(&method, &uri) {
        tracing::info!(
            request_id = %request_id,
            route_kind = %route_kind,
            app_id = %app_id,
            status = %status,
            latency_ms,
            request_bytes,
            response_bytes,
            method = %method,
            uri = %uri,
            "request finished"
        );
    }

    response
}

#[derive(Debug)]
pub(crate) struct AppError {
    status: StatusCode,
    message: String,
}

impl AppError {
    pub(crate) fn msg(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
        }
    }

    pub(crate) fn status(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }
}

impl From<anyhow::Error> for AppError {
    fn from(value: anyhow::Error) -> Self {
        Self::msg(value.to_string())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        if self.status.is_server_error() {
            tracing::error!(status = %self.status, error = %self.message, "request failed");
        } else {
            tracing::warn!(status = %self.status, error = %self.message, "request failed");
        }
        (self.status, self.message).into_response()
    }
}

/// 集成测试与 HTTP 级用例构造 `AppState`（依赖仓库内 `mei-lang/../ws-dev`）。
#[cfg(test)]
pub(crate) mod test_support {
    use std::{
        collections::HashMap,
        path::PathBuf,
        sync::{Arc, Mutex},
    };

    use anyhow::Context;

    pub(crate) fn package_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("server crate parent (mei-lang/)")
            .to_path_buf()
    }

    pub(crate) fn test_app_state() -> anyhow::Result<super::super::types::AppState> {
        let package_root = package_root();
        let source_root = package_root
            .join("../workspaces/ws-dev")
            .canonicalize()
            .context("workspace root (mei-lang/../workspaces/ws-dev)")?;
        let native_agent = Arc::new(crate::mei_agent::NativeAgent::open_with_resource_tools(
            source_root.clone(),
            Arc::new(crate::resource_tool_bridge::SceneResourceToolExecutor::default()),
        )?);
        Ok(super::super::types::AppState {
            package_root: Arc::new(package_root),
            source_root: Arc::new(source_root),
            agent_preferred_mode: Arc::new("native".into()),
            agent_preferred_server_url: Arc::new(String::new()),
            agent_auto_start: false,
            auth_enforcement: crate::auth::AuthEnforcement::Disabled,
            agent_runtime: Arc::new(Mutex::new(
                crate::agent_runtime::ManagedOpencodeRuntime::default(),
            )),
            agent_session_context: Arc::new(Mutex::new(HashMap::new())),
            native_agent,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::cli::args::{
        CheckArgs, CliAppSelectorArgs, Command, HostArgs, HostCommand, HostDescribeArgs,
        InspectArgs, InspectCommand, QueryArgs, QueryCommand, ServeArgs,
    };

    use super::super::cli_dispatch::ensure_command_allowed;
    use super::super::types::BinaryFlavor;

    fn app_selector() -> CliAppSelectorArgs {
        CliAppSelectorArgs {
            source_root: PathBuf::from("../workspaces/ws-dev"),
            app: "demo".into(),
            scene: None,
            target_file: None,
            json: false,
        }
    }

    #[test]
    fn toolchain_entry_allows_headless_commands() {
        let command = Command::Inspect(InspectArgs {
            command: InspectCommand::Summary(crate::cli::args::InspectSummaryArgs {
                app: app_selector(),
            }),
        });
        assert!(ensure_command_allowed(BinaryFlavor::Toolchain, &command).is_ok());
    }

    #[test]
    fn toolchain_entry_rejects_host_commands() {
        let command = Command::Serve(ServeArgs {
            workspace: None,
            source_root: PathBuf::from("../workspaces/ws-dev"),
            host_surface: "full".into(),
            auth: false,
            host: "127.0.0.1".into(),
            port: 3000,
            startup_policy: "background-build".into(),
            auto_agent: false,
            sync_agent_skill: false,
            toolchain_mode: "installed".into(),
        });
        assert!(ensure_command_allowed(BinaryFlavor::Toolchain, &command).is_err());
    }

    #[test]
    fn host_web_entry_allows_host_commands() {
        let command = Command::Host(HostArgs {
            command: HostCommand::Describe(HostDescribeArgs { json: true }),
        });
        assert!(ensure_command_allowed(BinaryFlavor::HostWeb, &command).is_ok());
    }

    #[test]
    fn compat_entry_rejects_all_commands() {
        let command = Command::Check(CheckArgs {
            app: app_selector(),
        });
        assert!(ensure_command_allowed(BinaryFlavor::Compat, &command).is_err());
    }

    #[test]
    fn host_web_entry_rejects_toolchain_commands() {
        let command = Command::Query(QueryArgs {
            command: QueryCommand::Resource(crate::cli::args::QueryResourceArgs {
                app: app_selector(),
                id: "orders".into(),
            }),
        });
        assert!(ensure_command_allowed(BinaryFlavor::HostWeb, &command).is_err());
    }
}
