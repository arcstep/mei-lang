use anyhow::{Context, Result};
use reqwest::Client;
use serde::Deserialize;

#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct BridgeHealthResponse {
    pub server_url: String,
    pub healthy: bool,
    pub version: String,
}

#[derive(Debug, Deserialize)]
struct UpstreamHealth {
    healthy: bool,
    version: String,
}

fn normalize_server_url(server_url: &str) -> String {
    server_url.trim_end_matches('/').to_string()
}

async fn get_json<T: serde::de::DeserializeOwned>(client: &Client, url: &str) -> Result<T> {
    let response = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("failed to GET {url}"))?;
    let status = response.status();
    let body = response
        .text()
        .await
        .with_context(|| format!("failed to read response body from {url}"))?;
    if !status.is_success() {
        anyhow::bail!("GET {url} returned {status}: {body}");
    }
    serde_json::from_str::<T>(&body)
        .with_context(|| format!("failed to decode JSON from {url}: {body}"))
}

pub(crate) async fn health(client: &Client, server_url: &str) -> Result<BridgeHealthResponse> {
    let server_url = normalize_server_url(server_url);
    let upstream =
        get_json::<UpstreamHealth>(client, &format!("{server_url}/global/health")).await?;
    Ok(BridgeHealthResponse {
        server_url,
        healthy: upstream.healthy,
        version: upstream.version,
    })
}
