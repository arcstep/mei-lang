//! Scene-qualified compile options and dataset/metric lookup for runtime APIs.

use axum::http::StatusCode;
use mei_lang_kernel::{CompileOptions, CompiledApp, LoadedResource};

use crate::AppError;

use super::util::is_script_target;

/// Request coordinates for scene-first dataset/metric APIs.
#[derive(Debug, Clone, Default)]
pub struct SceneQueryCoords {
    pub scene_id: Option<String>,
    /// Legacy source locator; used when `scene_id` is absent to derive compile context.
    pub target: Option<String>,
}

impl SceneQueryCoords {
    pub fn from_parts(scene_id: Option<String>, target: Option<String>) -> Self {
        Self {
            scene_id: scene_id
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
            target: target
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
        }
    }
}

pub fn compile_options_from_coords(coords: &SceneQueryCoords) -> CompileOptions {
    let preview_target = coords
        .target
        .as_deref()
        .filter(|target| is_script_target(target))
        .map(ToString::to_string);
    CompileOptions {
        scene: coords.scene_id.clone(),
        preview_target,
    }
}

pub struct ResolvedSceneContext {
    pub scene_id: String,
    pub scene_path: Option<String>,
}

/// Resolve active scene id/path after compile for API responses and validation.
pub fn resolved_scene_context(compiled: &CompiledApp) -> ResolvedSceneContext {
    let scene_id = compiled
        .active_scene
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or_else(|| {
            compiled
                .scene_routes
                .iter()
                .find(|route| route.target_file == compiled.active_target_file)
                .map(|route| route.scene_id.clone())
        })
        .unwrap_or_else(|| "default".to_string());
    let scene_path = compiled
        .active_target_file
        .trim()
        .to_string();
    let scene_path = if scene_path.is_empty() {
        None
    } else {
        Some(scene_path)
    };
    ResolvedSceneContext {
        scene_id,
        scene_path,
    }
}

/// Locate a dataset resource within the compiled active scene resource table.
pub fn locate_dataset_resource<'a>(
    compiled: &'a CompiledApp,
    dataset_id: &str,
    expected_scene_id: Option<&str>,
) -> Result<&'a LoadedResource, AppError> {
    let normalized = dataset_id.trim();
    if normalized.is_empty() {
        return Err(AppError::status(
            StatusCode::BAD_REQUEST,
            "dataset_id is required",
        ));
    }
    if normalized == "__source_path__" || normalized.ends_with(".mei") {
        return Err(AppError::status(
            StatusCode::BAD_REQUEST,
            "dataset_id must be an explicit stable world resource id",
        ));
    }

    let matches: Vec<_> = compiled
        .resources
        .iter()
        .filter(|resource| resource.id == normalized)
        .collect();

    if matches.is_empty() {
        return Err(AppError::status(
            StatusCode::NOT_FOUND,
            format!("dataset `{normalized}` not found in active scene resources"),
        ));
    }
    if matches.len() > 1 {
        return Err(AppError::status(
            StatusCode::BAD_REQUEST,
            format!("dataset `{normalized}` is ambiguous across scenes"),
        ));
    }

    let resource = matches[0];
    if let Some(expected) = expected_scene_id.map(str::trim).filter(|s| !s.is_empty()) {
        let active = compiled
            .active_scene
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(expected);
        if active != expected {
            return Err(AppError::status(
                StatusCode::BAD_REQUEST,
                format!(
                    "dataset `{normalized}` is not available in scene `{expected}` (active scene is `{active}`)"
                ),
            ));
        }
    }

    Ok(resource)
}
