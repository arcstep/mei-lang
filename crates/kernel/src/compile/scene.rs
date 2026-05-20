use std::path::Path;

use serde_json::Value;

use crate::model::{AppDecl, CompiledSceneRoute, Diagnostic, Severity};
use crate::typed_refs::{decode_ref_value, normalize_rel_path, RefKind};

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

    collect_route_from_app_scene_field(app_decl, &mut routes);
    collect_inline_scene_routes(app_decls, &mut routes);
    collect_scene_ref_routes(app_decls, &mut routes);

    if routes.is_empty() {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "missing_app_scene".to_string(),
            message:
                "app(...) must bind at least one scene (inline scene, app.scene=scene_ref(...), or app.add_scene(scene_ref(...)))"
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
                    "default_scene `{default_scene}` did not match an inline scene or scene_ref route"
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
    let normalized = normalize_rel_path(selector);
    routes.iter().find(|route| {
        route.scene_id == selector || normalize_rel_path(route.target_file.as_str()) == normalized
    })
}

fn collect_route_from_app_scene_field(app_decl: &AppDecl, routes: &mut Vec<CompiledSceneRoute>) {
    let Some(raw_scene) = app_decl.scene.as_ref() else {
        return;
    };
    if let Some(scene_id) = raw_scene
        .as_str()
        .map(str::trim)
        .filter(|scene_id| !scene_id.is_empty())
    {
        upsert_route(
            routes,
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
    if let Some(route) = route_from_scene_value(raw_scene) {
        upsert_route(routes, route);
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
    upsert_route(
        routes,
        CompiledSceneRoute {
            scene_id,
            frame_id: None,
            target_file: scene_ref.path,
            kind: "file_ref".to_string(),
            title: None,
            is_default: false,
            access_export: true,
        },
    );
}

fn collect_inline_scene_routes(raw: &Value, routes: &mut Vec<CompiledSceneRoute>) {
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
        upsert_route(
            routes,
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

fn collect_scene_ref_routes(raw: &Value, routes: &mut Vec<CompiledSceneRoute>) {
    let Some(values) = raw.as_array() else {
        return;
    };
    for value in values {
        if value.get("kind").and_then(Value::as_str) != Some("app_scene_ref") {
            continue;
        }
        let scene_value = value.get("scene").cloned().unwrap_or(Value::Null);
        if let Some(route) = route_from_scene_value(&scene_value) {
            upsert_route(routes, route);
        }
    }
}

fn route_from_scene_value(raw_scene: &Value) -> Option<CompiledSceneRoute> {
    if let Some(expr) = decode_ref_value(raw_scene) {
        if expr.kind != RefKind::Scene {
            return None;
        }
        let path = expr
            .locator
            .scene_file
            .as_deref()
            .map(str::trim)
            .filter(|path| !path.is_empty())?;
        let scene_id = expr
            .locator
            .scene_id
            .clone()
            .or(expr.id.clone())
            .unwrap_or_else(|| scene_name_from_path(path));
        return Some(CompiledSceneRoute {
            scene_id,
            frame_id: None,
            target_file: path.to_string(),
            kind: "file_ref".to_string(),
            title: None,
            is_default: false,
            access_export: true,
        });
    }
    let scene_ref = serde_json::from_value::<SceneFileRefDecl>(raw_scene.clone()).ok()?;
    if scene_ref.kind != "scene_file_ref" {
        return None;
    }
    let scene_id = scene_ref
        .id
        .clone()
        .unwrap_or_else(|| scene_name_from_path(&scene_ref.path));
    Some(CompiledSceneRoute {
        scene_id,
        frame_id: None,
        target_file: scene_ref.path,
        kind: "file_ref".to_string(),
        title: None,
        is_default: false,
        access_export: true,
    })
}

fn upsert_route(routes: &mut Vec<CompiledSceneRoute>, route: CompiledSceneRoute) {
    if let Some(existing) = routes
        .iter_mut()
        .find(|item| item.scene_id == route.scene_id)
    {
        *existing = route;
        return;
    }
    routes.push(route);
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
