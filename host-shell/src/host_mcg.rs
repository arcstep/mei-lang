//! Legacy `/mcg` page — permanently redirected to `/runtime` (应用中心).
//! Build-graph APIs under `/api/build/graph/mcg*` remain available for tooling.

use axum::response::{IntoResponse, Redirect, Response};

pub async fn host_mcg_page() -> Response {
    Redirect::permanent("/runtime").into_response()
}
