use axum::body::Body;
use axum::http::StatusCode;
use axum::response::Response;
use mei_host_core::{load_app_config_for_ctx, HostContext};
use serde_json::json;

pub fn configured_plug_ds_endpoint(ctx: &HostContext) -> Option<String> {
    if let Ok(url) = std::env::var("MEI_PLUG_DS_URL") {
        let trimmed = url.trim();
        if !trimmed.is_empty() {
            crate::legacy_compat::warn_migration_plug_ds_url();
            return Some(trimmed.to_string());
        }
    }
    if let Ok(config) = load_app_config_for_ctx(ctx) {
        if let Some(endpoint) = config
            .runtime
            .plugs
            .ds
            .as_ref()
            .map(|plug| plug.endpoint.trim())
            .filter(|value| !value.is_empty())
        {
            return Some(endpoint.to_string());
        }
    }
    None
}

pub async fn proxy_post_json(endpoint: &str, path: &str, body: serde_json::Value) -> Response {
    let url = join_url(endpoint, path);
    let client = plug_proxy_client();
    let response = match client.post(url.as_str()).json(&body).send().await {
        Ok(response) => response,
        Err(error) => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                axum::Json(json!({
                    "error": "plug-ds unreachable",
                    "endpoint": endpoint,
                    "detail": error.to_string(),
                })),
            )
                .into_response();
        }
    };
    let status =
        StatusCode::from_u16(response.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let bytes = match response.bytes().await {
        Ok(bytes) => bytes,
        Err(error) => {
            return (
                StatusCode::BAD_GATEWAY,
                axum::Json(json!({
                    "error": "plug-ds response read failed",
                    "detail": error.to_string(),
                })),
            )
                .into_response();
        }
    };
    Response::builder()
        .status(status)
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(Body::from(bytes))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

/// Call plug-ds scope activation warmup via sidecar HTTP.
pub async fn proxy_plug_ds_activate(
    endpoint: &str,
    scope: &str,
    hops: usize,
) -> Result<(), String> {
    let path = format!(
        "/api/plug-ds/activate?scope={}&hops={}",
        urlencoding_encode(scope),
        hops
    );
    let url = join_url(endpoint, path.as_str());
    let client = plug_proxy_client();
    let response = client
        .post(url.as_str())
        .send()
        .await
        .map_err(|error| format!("plug-ds unreachable at {endpoint}: {error}"))?;
    if response.status().is_success() {
        return Ok(());
    }
    let status = response.status();
    let body = response
        .text()
        .await
        .unwrap_or_else(|_| "<unreadable body>".to_string());
    Err(format!("plug-ds activate failed ({status}): {body}"))
}

fn urlencoding_encode(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => ch.to_string(),
            _ => format!("%{:02X}", ch as u8),
        })
        .collect()
}

fn join_url(base: &str, path: &str) -> String {
    let base = base.trim_end_matches('/');
    let path = path.trim_start_matches('/');
    format!("{base}/{path}")
}

fn plug_proxy_client() -> &'static reqwest::Client {
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .build()
            .expect("plug proxy reqwest client")
    })
}

use axum::response::IntoResponse;

#[cfg(test)]
mod tests {
    use super::*;
    use mei_host_core::HostContext;

    #[test]
    fn join_url_normalizes_slashes() {
        assert_eq!(
            join_url("http://127.0.0.1:9528", "/api/datasets/query"),
            "http://127.0.0.1:9528/api/datasets/query"
        );
    }

    #[test]
    fn urlencoding_encode_escapes_scope_query() {
        assert_eq!(super::urlencoding_encode("a b/c"), "a%20b%2Fc");
    }

    #[test]
    fn configured_endpoint_reads_app_config_override() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            tmp.path().join("workspace.json"),
            r#"{"schemaVersion":1,"workspace":{"id":"test","defaultApp":"data-demo"}}"#,
        )
        .expect("write workspace");
        std::fs::create_dir_all(tmp.path().join("apps/data-demo")).expect("create app dir");
        std::fs::write(
            tmp.path().join("apps/data-demo/app.config.json"),
            r#"{"schemaVersion":1,"runtime":{"plugs":{"ds":{"endpoint":"http://127.0.0.1:9999"}}}}"#,
        )
        .expect("write app config");
        let ctx = HostContext::new(tmp.path().to_path_buf(), "data-demo".to_string());
        assert_eq!(
            configured_plug_ds_endpoint(&ctx).as_deref(),
            Some("http://127.0.0.1:9999")
        );
    }
}
