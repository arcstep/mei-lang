use axum::response::{IntoResponse, Redirect, Response};
use mei_lang_app::UiRouteMode;
use mei_lang_kernel::{resolve_app_root, resolve_default_scene_from_root};

use crate::AppState;

use super::super::query::{scene_projection_canonical_location, AppQuery};

pub(super) fn try_scene_projection_redirect(
    state: &AppState,
    route_mode: UiRouteMode,
    app_id: &str,
    query: &AppQuery,
    access_path_scene: &Option<String>,
    access_static_file: &Option<String>,
) -> Option<Response> {
    if !route_mode.uses_scene_route() || access_static_file.is_some() {
        return None;
    }
    let q_scene = query
        .scene
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    if let Some(ps) = access_path_scene {
        if let Some(qs) = q_scene {
            if qs != ps {
                return Some(
                    Redirect::temporary(&scene_projection_canonical_location(
                        route_mode,
                        app_id,
                        ps,
                        query.tab.as_deref(),
                        query.chrome.as_deref(),
                        query.data_mode.as_deref(),
                        query.review_projection.as_deref(),
                    ))
                    .into_response(),
                );
            }
        }
    } else if let Some(qs) = q_scene {
        return Some(
            Redirect::temporary(&scene_projection_canonical_location(
                route_mode,
                app_id,
                qs,
                query.tab.as_deref(),
                query.chrome.as_deref(),
                query.data_mode.as_deref(),
                query.review_projection.as_deref(),
            ))
            .into_response(),
        );
    } else if let Ok(Some(default_scene)) =
        resolve_default_scene_from_root(&resolve_app_root(state.source_root.as_path(), app_id))
    {
        return Some(
            Redirect::temporary(&scene_projection_canonical_location(
                route_mode,
                app_id,
                &default_scene,
                query.tab.as_deref(),
                query.chrome.as_deref(),
                query.data_mode.as_deref(),
                query.review_projection.as_deref(),
            ))
            .into_response(),
        );
    }
    None
}
