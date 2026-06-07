use axum::{
    extract::{Path as AxumPath, State},
    Json,
};

use mei_lang_kernel::CompileOptions;
use mei_lang_toolchain as toolchain;

use crate::{AppError, AppState};

pub async fn projection_api(
    State(state): State<AppState>,
    AxumPath(app_id_raw): AxumPath<String>,
) -> Result<Json<mei_lang_kernel::CompiledApp>, AppError> {
    let app_id = app_id_raw.trim_start_matches('/');
    let report = toolchain::compile_report(&state.source_root, app_id, CompileOptions::default())
        .map_err(AppError::from)?;
    Ok(Json(report.compiled))
}
