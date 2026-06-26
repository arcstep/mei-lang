use axum::body::Bytes;

use crate::AppError;

use super::types::DatasetQueryRequest;

pub(super) fn dataset_query_body_preview(body: &[u8]) -> String {
    const MAX_PREVIEW_BYTES: usize = 4096;
    let len = body.len().min(MAX_PREVIEW_BYTES);
    let mut preview = String::from_utf8_lossy(&body[..len]).to_string();
    if body.len() > MAX_PREVIEW_BYTES {
        preview.push_str("...<truncated>");
    }
    preview
}

pub(super) fn parse_dataset_query_request(
    app_id: &str,
    body: &Bytes,
) -> Result<DatasetQueryRequest, AppError> {
    let mut deserializer = serde_json::Deserializer::from_slice(body);
    serde_path_to_error::deserialize::<_, DatasetQueryRequest>(&mut deserializer).map_err(|error| {
        let path = error.path().to_string();
        let inner = error.inner().to_string();
        let preview = dataset_query_body_preview(body);
        tracing::warn!(
            app_id = %app_id,
            path = %path,
            error = %inner,
            body = %preview,
            "dataset query JSON rejected"
        );
        if preview.contains("\"method\"") && preview.contains("\"headers\"") && !preview.contains("\"dataset_id\"") {
            tracing::error!(
                app_id = %app_id,
                "dataset query received fetch RequestInit object instead of payload; check frontend fetch wrapper"
            );
        }
        AppError::status(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            format!("invalid dataset query JSON at `{path}`: {inner}"),
        )
    })
}

pub(super) fn access_artifact_unavailable_error(
    request_kind: &str,
    app_id: &str,
    scene_id: &str,
    target: &str,
) -> AppError {
    let scene_label = if scene_id.trim().is_empty() || scene_id == "-" {
        "scene=<unspecified>"
    } else {
        scene_id
    };
    let target_label = if target.trim().is_empty() || target == "-" {
        "target=<unspecified>"
    } else {
        target
    };
    AppError::status(
        axum::http::StatusCode::SERVICE_UNAVAILABLE,
        format!(
            "{request_kind} requires prebuilt access artifacts on access-only host: app={app_id} {scene_label} {target_label}; wait for startup warmup or prebuild artifacts before serving access traffic"
        ),
    )
}
