use axum::body::Body;
use axum::http::StatusCode;
use axum::response::Response;
use mei_host_core::{load_app_config_for_ctx, HostContext};
use serde_json::json;

const DEFAULT_PLUG_DS_URL: &str = "http://127.0.0.1:9528";

pub fn resolve_plug_ds_endpoint(ctx: &HostContext) -> String {
    if let Ok(url) = std::env::var("MEI_PLUG_DS_URL") {
        let trimmed = url.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
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
            return endpoint.to_string();
        }
    }
    DEFAULT_PLUG_DS_URL.to_string()
}

pub async fn proxy_post_json(
    ctx: &HostContext,
    path: &str,
    body: serde_json::Value,
) -> Response {
    let endpoint = resolve_plug_ds_endpoint(ctx);
    let url = join_url(endpoint.as_str(), path);
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

    #[test]
    fn join_url_normalizes_slashes() {
        assert_eq!(
            join_url("http://127.0.0.1:9528", "/api/datasets/query"),
            "http://127.0.0.1:9528/api/datasets/query"
        );
    }
}
