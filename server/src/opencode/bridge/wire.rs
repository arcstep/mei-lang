use anyhow::{Context, Result};
use reqwest::Client;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::time::Duration;

pub(super) fn normalize_server_url(server_url: &str) -> String {
    server_url.trim_end_matches('/').to_string()
}

pub(super) async fn get_json<T: serde::de::DeserializeOwned>(
    client: &Client,
    url: &str,
) -> Result<T> {
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

pub(super) async fn get_json_with_timeout<T: DeserializeOwned>(
    client: &Client,
    url: &str,
    timeout: Duration,
) -> Result<T> {
    let response = client
        .get(url)
        .timeout(timeout)
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

pub(super) async fn post_json<T: DeserializeOwned>(
    client: &Client,
    url: &str,
    body: Value,
) -> Result<T> {
    let response = client
        .post(url)
        .json(&body)
        .send()
        .await
        .with_context(|| format!("failed to POST {url}"))?;
    let status = response.status();
    let text = response
        .text()
        .await
        .with_context(|| format!("failed to read response body from {url}"))?;
    if !status.is_success() {
        anyhow::bail!("POST {url} returned {status}: {text}");
    }
    serde_json::from_str::<T>(&text)
        .with_context(|| format!("failed to decode JSON from {url}: {text}"))
}

pub(super) fn decode_applied_response(action: &str, value: Value) -> Result<bool> {
    match value {
        Value::Bool(applied) => Ok(applied),
        Value::Object(_) => Ok(true),
        other => anyhow::bail!(
            "unexpected {action} response shape: {}",
            serde_json::to_string(&other).unwrap_or_else(|_| "<non-json>".to_string())
        ),
    }
}
