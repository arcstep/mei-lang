use std::path::Path;

use anyhow::Result;
use serde_json::Value;

use crate::{
    eval::evaluate_mei_file,
    model::{AppDecl, Diagnostic, EntryDecl, Severity},
};

use super::decls::SceneFileRefDecl;

pub(super) fn resolve_scene_source(
    app_root: &Path,
    app_main: &Path,
    app_decl: &AppDecl,
    app_decls: &Value,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<(String, Value)> {
    if let Some(entry) = app_decl.entries.first().cloned() {
        let entry_target = entry_source(&entry).unwrap_or_else(|| "main.mei".to_string());
        let entry_path = app_root.join(&entry_target);
        return Ok((entry_target, evaluate_mei_file(&entry_path)?));
    }

    if let Some(default_scene) = app_decl.default_scene.as_deref() {
        if let Some(target) = resolve_default_scene_target(app_decls, default_scene) {
            if target == "main.mei" {
                return Ok((target, app_decls.clone()));
            }
            let entry_path = app_root.join(&target);
            return Ok((target, evaluate_mei_file(&entry_path)?));
        }

        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "missing_default_scene".to_string(),
            message: format!(
                "default_scene `{default_scene}` did not match an inline scene or app.add_scene(scene_file_ref(...))"
            ),
            source_path: Some(app_main.to_string_lossy().to_string()),
        });
    }

    if has_inline_scene(app_decls, None) {
        return Ok(("main.mei".to_string(), app_decls.clone()));
    }

    if let Some(target) = first_scene_file_ref_target(app_decls) {
        let entry_path = app_root.join(&target);
        return Ok((target, evaluate_mei_file(&entry_path)?));
    }

    Ok(("main.mei".to_string(), app_decls.clone()))
}

fn resolve_default_scene_target(raw: &Value, default_scene: &str) -> Option<String> {
    if has_inline_scene(raw, Some(default_scene)) {
        return Some("main.mei".to_string());
    }

    scene_file_ref_target(raw, default_scene)
}

fn has_inline_scene(raw: &Value, scene_id: Option<&str>) -> bool {
    raw.as_array().is_some_and(|values| {
        values.iter().any(|value| {
            if value.get("kind").and_then(Value::as_str) != Some("scene") {
                return false;
            }
            match scene_id {
                Some(expected) => value.get("id").and_then(Value::as_str) == Some(expected),
                None => true,
            }
        })
    })
}

fn scene_file_ref_target(raw: &Value, scene_id: &str) -> Option<String> {
    raw.as_array().and_then(|values| {
        values.iter().find_map(|value| {
            if value.get("kind").and_then(Value::as_str) != Some("app_scene_ref") {
                return None;
            }
            let scene_ref = serde_json::from_value::<SceneFileRefDecl>(
                value.get("scene").cloned().unwrap_or(Value::Null),
            )
            .ok()?;
            if scene_ref.kind != "scene_file_ref" {
                return None;
            }
            if scene_ref.id.as_deref() == Some(scene_id)
                || (scene_ref.id.is_none() && scene_name_from_path(&scene_ref.path) == scene_id)
            {
                return Some(scene_ref.path);
            }
            None
        })
    })
}

fn first_scene_file_ref_target(raw: &Value) -> Option<String> {
    raw.as_array().and_then(|values| {
        values.iter().find_map(|value| {
            if value.get("kind").and_then(Value::as_str) != Some("app_scene_ref") {
                return None;
            }
            let scene_ref = serde_json::from_value::<SceneFileRefDecl>(
                value.get("scene").cloned().unwrap_or(Value::Null),
            )
            .ok()?;
            if scene_ref.kind == "scene_file_ref" {
                Some(scene_ref.path)
            } else {
                None
            }
        })
    })
}

fn scene_name_from_path(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or(path)
        .to_string()
}

fn entry_source(entry: &EntryDecl) -> Option<String> {
    entry.scene.clone().or_else(|| entry.frame.clone())
}
