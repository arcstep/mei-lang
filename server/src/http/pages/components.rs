use std::path::Path;

use axum::{
    extract::{Path as AxumPath, State},
    response::Response,
};

use crate::{AppError, AppState};

use super::static_serve::serve_static_asset;

pub(crate) fn resolve_components_root(source_root: &Path) -> std::path::PathBuf {
    let local = source_root.join("_components");
    if local.exists() {
        return local;
    }
    if let Some(parent) = source_root.parent() {
        let shared = parent.join("_components");
        if shared.exists() {
            return shared;
        }
    }
    local
}

pub async fn component_asset(
    State(state): State<AppState>,
    AxumPath(path): AxumPath<String>,
) -> Result<Response, AppError> {
    let components_root = resolve_components_root(&state.source_root);
    serve_static_asset(components_root.join(&path), "component asset")
}
