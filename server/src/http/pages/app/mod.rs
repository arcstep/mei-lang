mod compiling_shell;
mod page;
mod query;
mod scene;

pub use page::app_page;

/// 仅用于 `http::pages` 集成测试；运行态页面从 `query` 模块直接引用 `AppQuery`。
#[cfg(test)]
pub(crate) type AppQuery = query::AppQuery;

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
    Ok(Redirect::to(&format!("/apps/build/{}", first.id)))
}
