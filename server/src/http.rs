pub mod pages;
pub mod projection_api;
pub mod scene_api;

use axum::routing::{get, post};
use axum::Router;

use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(pages::index))
        .route("/apps/:mode/:app_id", get(pages::app_page))
        .route("/api/projection/:app_id", get(projection_api::projection_api))
        .route("/api/sim/step/:app_id", post(scene_api::sim_step_api))
        .route("/workspace-components/*path", get(pages::component_asset))
}
