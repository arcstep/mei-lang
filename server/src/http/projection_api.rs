use axum::{
    extract::{Path as AxumPath, State},
    Json,
};

use mei_lang_kernel::compile_app;

use crate::{AppError, AppState};

pub async fn projection_api(
    State(state): State<AppState>,
    AxumPath(app_id): AxumPath<String>,
) -> Result<Json<mei_lang_kernel::CompiledApp>, AppError> {
    let compiled = compile_app(&state.source_root, &app_id).map_err(AppError::from)?;
    Ok(Json(compiled))
}
