mod page;
mod query;
mod scene;

pub use page::app_page;
pub type AppQuery = query::AppQuery;

use axum::{extract::State, response::Redirect};
use mei_lang_kernel::discover_apps;

use crate::{AppError, AppState};

use super::app_render::choose_default_app;

pub async fn index(State(state): State<AppState>) -> Result<Redirect, AppError> {
    let apps = discover_apps(&state.source_root).map_err(AppError::from)?;
    let first = choose_default_app(&state.source_root, &apps).or_else(|| apps.first());
    let first = first.ok_or_else(|| {
        AppError::msg(format!(
            "source root has no discoverable apps (need at least one first-level subdirectory under `{}` containing `main.mei`; root-level `main.mei` is ignored)",
            state.source_root.display()
        ))
    })?;
    Ok(Redirect::to(&format!("/apps/manage/{}", first.id)))
}
