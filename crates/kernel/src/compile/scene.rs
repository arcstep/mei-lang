use std::{collections::BTreeSet, path::Path};

use serde_json::Value;

use crate::model::{AppDecl, CompiledSceneRoute, Diagnostic, Severity};

use super::decls::SceneFileRefDecl;

pub(super) struct SceneRouteRegistry {
    pub routes: Vec<CompiledSceneRoute>,
    pub default_scene_id: Option<String>,
}

pub(super) fn resolve_scene_routes(
    app_main: &Path,
    app_decl: &AppDecl,
    app_decls: &Value,
    diagnostics: &mut Vec<Diagnostic>,
) -> SceneRouteRegistry {
    let mut routes = Vec::new();
    let mut seen_scene_ids = BTreeSet::new();

    collect_route_from_app_scene_field(app_decl, &mut routes, &mut seen_scene_ids);
    collect_inline_scene_routes(app_decls, &mut routes, &mut seen_scene_ids);
    collect_scene_file_ref_routes(app_decls, &mut routes, &mut seen_scene_ids);

    if routes.is_empty() {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "missing_app_scene".to_string(),
            message:
                "app(...) must bind at least one scene (inline scene, app.scene, or app.add_scene(scene_file_ref(...)))"
                    .to_string(),
            source_path: Some(app_main.to_string_lossy().to_string()),
        });
    }

    if let Some(default_scene) = app_decl.default_scene.as_deref() {
        if !routes.is_empty() && !routes.iter().any(|r| r.scene_id == default_scene) {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "missing_default_scene".to_string(),
                message: format!(
                    "default_scene `{default_scene}` did not match an inline scene or app.add_scene(scene_file_ref(...))"
                ),
                source_path: Some(app_main.to_string_lossy().to_string()),
            });
        }
    }

    let mut default_scene_id = resolve_default_scene_id(app_decl, &routes);
    if default_scene_id.is_none() {
        default_scene_id = routes.first().map(|route| route.scene_id.clone());
    }
    if let Some(default_id) = default_scene_id.as_deref() {
        for route in &mut routes {
            route.is_default = route.scene_id == default_id;
        }
    }

    SceneRouteRegistry {
        routes,
        default_scene_id,
    }
}

pub(super) fn find_scene_route<'a>(
    routes: &'a [CompiledSceneRoute],
    selector: &str,
) -> Option<&'a CompiledSceneRoute> {
    let normalized = normalize_path(selector);
    routes.iter().find(|route| {
        route.scene_id == selector || normalize_path(route.target_file.as_str()) == normalized
    })
}

fn collect_route_from_app_scene_field(
    app_decl: &AppDecl,
    routes: &mut Vec<CompiledSceneRoute>,
    seen_scene_ids: &mut BTreeSet<String>,
) {
    let Some(raw_scene) = app_decl.scene.as_ref() else {
        return;
    };
    if let Some(scene_id) = raw_scene
        .as_str()
        .map(str::trim)
        .filter(|scene_id| !scene_id.is_empty())
    {
        append_route(
            routes,
            seen_scene_ids,
            CompiledSceneRoute {
                scene_id: scene_id.to_string(),
                frame_id: None,
                target_file: "main.mei".to_string(),
                kind: "declarative".to_string(),
                title: None,
                is_default: false,
                access_export: true,
            },
        );
        return;
    }
    let Ok(scene_ref) = serde_json::from_value::<SceneFileRefDecl>(raw_scene.clone()) else {
        return;
    };
    if scene_ref.kind != "scene_file_ref" {
        return;
    }
    let scene_id = scene_ref
        .id
        .clone()
        .unwrap_or_else(|| scene_name_from_path(&scene_ref.path));
    append_route(
        routes,
        seen_scene_ids,
        CompiledSceneRoute {
            scene_id: scene_id.clone(),
            frame_id: None,
            target_file: scene_ref.path,
            kind: "file_ref".to_string(),
            title: None,
            is_default: false,
            access_export: true,
        },
    );
}

fn collect_inline_scene_routes(
    raw: &Value,
    routes: &mut Vec<CompiledSceneRoute>,
    seen_scene_ids: &mut BTreeSet<String>,
) {
    let Some(values) = raw.as_array() else {
        return;
    };
    for value in values {
        if value.get("kind").and_then(Value::as_str) != Some("scene") {
            continue;
        }
        let scene_id = value
            .get("id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_string())
            .unwrap_or_else(|| scene_name_from_path("main.mei"));
        append_route(
            routes,
            seen_scene_ids,
            CompiledSceneRoute {
                scene_id: scene_id.clone(),
                frame_id: None,
                target_file: "main.mei".to_string(),
                kind: "inline".to_string(),
                title: value
                    .get("summary")
                    .and_then(Value::as_str)
                    .map(|value| value.to_string()),
                is_default: false,
                access_export: value
                    .get("access_export")
                    .and_then(Value::as_bool)
                    .unwrap_or(true),
            },
        );
    }
}

fn collect_scene_file_ref_routes(
    raw: &Value,
    routes: &mut Vec<CompiledSceneRoute>,
    seen_scene_ids: &mut BTreeSet<String>,
) {
    let Some(values) = raw.as_array() else {
        return;
    };
    for value in values {
        if value.get("kind").and_then(Value::as_str) != Some("app_scene_ref") {
            continue;
        }
        let Ok(scene_ref) = serde_json::from_value::<SceneFileRefDecl>(
            value.get("scene").cloned().unwrap_or(Value::Null),
        ) else {
            continue;
        };
        if scene_ref.kind != "scene_file_ref" {
            continue;
        }
        let scene_id = scene_ref
            .id
            .clone()
            .unwrap_or_else(|| scene_name_from_path(&scene_ref.path));
        append_route(
            routes,
            seen_scene_ids,
            CompiledSceneRoute {
                scene_id: scene_id.clone(),
                frame_id: None,
                target_file: scene_ref.path,
                kind: "file_ref".to_string(),
                title: None,
                is_default: false,
                access_export: true,
            },
        );
    }
}

fn append_route(
    routes: &mut Vec<CompiledSceneRoute>,
    seen_scene_ids: &mut BTreeSet<String>,
    route: CompiledSceneRoute,
) {
    if seen_scene_ids.insert(route.scene_id.clone()) {
        routes.push(route);
    }
}

fn resolve_default_scene_id(app_decl: &AppDecl, routes: &[CompiledSceneRoute]) -> Option<String> {
    if let Some(default_scene) = app_decl.default_scene.as_deref() {
        if let Some(route) = routes.iter().find(|route| route.scene_id == default_scene) {
            return Some(route.scene_id.clone());
        }
    }

    routes
        .iter()
        .find(|route| route.kind == "inline")
        .or_else(|| routes.iter().find(|route| route.kind == "file_ref"))
        .map(|route| route.scene_id.clone())
}

pub(super) fn scene_name_from_path(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or(path)
        .to_string()
}

fn normalize_path(value: &str) -> String {
    value
        .trim()
        .replace('\\', "/")
        .trim_start_matches("./")
        .to_string()
}
