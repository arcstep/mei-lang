use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    pub fn msg(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
        }
    }

    pub fn status(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let kind = match self.status {
            StatusCode::BAD_REQUEST | StatusCode::UNPROCESSABLE_ENTITY => "validation",
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => "forbidden",
            StatusCode::NOT_FOUND => "not-found",
            StatusCode::CONFLICT => "conflict",
            StatusCode::SERVICE_UNAVAILABLE => "provider-unavailable",
            _ if self.status.is_server_error() => "provider-unavailable",
            _ => "request-failed",
        };
        (
            self.status,
            Json(json!({
                "error": self.message.clone(),
                "message": self.message,
                "kind": kind,
            })),
        )
            .into_response()
    }
}
