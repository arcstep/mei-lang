//! Access stage routes: `/apps/{id}/{stage}` (legacy view/layout redirects).

use std::path::Path;

use mei_lang_app::UiRouteMode;

use crate::pages::AppQuery;

#[allow(dead_code)]
pub fn parse_apps_surface_path(path: &str) -> Option<(String, UiRouteMode, String)> {
    let segments: Vec<&str> = path
        .trim()
        .trim_start_matches('/')
        .split('/')
        .filter(|part| !part.is_empty())
        .collect();
    if segments.len() < 3 || segments.first().copied() != Some("apps") {
        return None;
    }
    let app_id = segments[1].to_string();
    let surface = match segments[2] {
        "app" => UiRouteMode::App,
        "layout" => UiRouteMode::Layout,
        "prototype" => UiRouteMode::Prototype,
        _ => return None,
    };
    let tail = segments[3..].join("/");
    Some((app_id, surface, tail))
}

pub fn merge_surface_query_defaults(query: &mut AppQuery, route_mode: UiRouteMode) {
    match route_mode {
        UiRouteMode::Layout => {
            if query.data_mode.is_none() {
                query.data_mode = Some("static".to_string());
            }
            if query.review_projection.is_none() {
                query.review_projection = Some("plane_region_section_slot".to_string());
            }
            if query.tree_max.is_none() {
                query.tree_max = Some("content".to_string());
            }
        }
        UiRouteMode::Prototype => {
            if query.data_mode.is_none() {
                query.data_mode = Some("static".to_string());
            }
            if query.review_projection.is_none() {
                query.review_projection = Some("static_full".to_string());
            }
            if query.tree_max.is_none() {
                query.tree_max = Some("content".to_string());
            }
        }
        UiRouteMode::App => {
            query.review_projection = None;
        }
        _ => {}
    }
}

pub fn parse_app_surface_tail(
    app_tail: &str,
    scene_query: Option<&str>,
    route_mode: UiRouteMode,
) -> (String, String, Option<String>) {
    let parts: Vec<&str> = app_tail
        .split('/')
        .filter(|part| !part.is_empty())
        .collect();
    if parts.is_empty() {
        return (String::new(), "home".to_string(), None);
    }
    let app_id = parts[0].to_string();
    if route_mode == UiRouteMode::App && parts.len() >= 3 && parts[1] == "scene" {
        return (app_id, parts[2].to_string(), None);
    }
    let scene = if route_mode == UiRouteMode::App {
        scene_query
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("__default_access__")
            .to_string()
    } else {
        scene_query
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("home")
            .to_string()
    };
    (app_id, scene, None)
}

pub fn legacy_app_access_redirect(workspace: &Path, app_tail: &str) -> Option<String> {
    let parts: Vec<&str> = app_tail.split('/').filter(|p| !p.is_empty()).collect();
    if parts.len() >= 2 && parts[1] == "access" {
        return Some(crate::shell_chrome::app_access_href(workspace, parts[0]));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_surface_path() {
        let (app, mode, tail) =
            parse_apps_surface_path("/apps/pretty-panels/layout").expect("layout path");
        assert_eq!(app, "pretty-panels");
        assert_eq!(mode, UiRouteMode::Layout);
        assert!(tail.is_empty());
    }
}
