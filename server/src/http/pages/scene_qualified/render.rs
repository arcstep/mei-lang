use axum::http::StatusCode;
use mei_lang_kernel::{
    locate_dataset_resource as kernel_locate_dataset_resource, CompiledApp, LoadedResource,
    RuntimeResourceResolveError,
};

use crate::AppError;

use super::route_parse::SceneQueryCoords;

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
    let scene_path = compiled.active_target_file.trim().to_string();
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

fn map_resolve_error(error: RuntimeResourceResolveError) -> AppError {
    match error {
        RuntimeResourceResolveError::EmptySelector => {
            AppError::status(StatusCode::BAD_REQUEST, error.to_string())
        }
        RuntimeResourceResolveError::ForbiddenLegacyId => {
            AppError::status(StatusCode::BAD_REQUEST, error.to_string())
        }
        RuntimeResourceResolveError::NotFound { .. } => {
            AppError::status(StatusCode::NOT_FOUND, error.to_string())
        }
        RuntimeResourceResolveError::Ambiguous { .. } => {
            AppError::status(StatusCode::BAD_REQUEST, error.to_string())
        }
        RuntimeResourceResolveError::NotDataset { .. } => {
            AppError::status(StatusCode::BAD_REQUEST, error.to_string())
        }
    }
}

/// Scene id used for post-locate availability checks on scene-qualified runtime APIs.
pub fn expected_scene_id_for_runtime_lookup(
    compiled: &CompiledApp,
    coords: Option<&SceneQueryCoords>,
) -> String {
    let scene_ctx = resolved_scene_context(compiled);
    if let Some(coords) = coords {
        if let Some(target) = coords
            .target
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            if target == compiled.active_target_file.trim() {
                return scene_ctx.scene_id;
            }
        }
        if let Some(requested) = coords
            .scene_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            return requested.to_string();
        }
    }
    scene_ctx.scene_id
}

fn compiled_serves_runtime_scene(compiled: &CompiledApp, expected_scene: &str) -> bool {
    let active = resolved_scene_context(compiled).scene_id;
    if active == expected_scene {
        return true;
    }
    compiled
        .scene_projection_assembly_by_id
        .contains_key(expected_scene)
        || compiled.scene_bindings_by_id.contains_key(expected_scene)
}

/// Locate a dataset resource within the compiled active scene resource table.
pub fn locate_dataset_resource<'a>(
    compiled: &'a CompiledApp,
    dataset_id: &str,
    coords: Option<&SceneQueryCoords>,
) -> Result<&'a LoadedResource, AppError> {
    let resource =
        kernel_locate_dataset_resource(compiled, dataset_id).map_err(map_resolve_error)?;

    let expected = expected_scene_id_for_runtime_lookup(compiled, coords);
    if !compiled_serves_runtime_scene(compiled, expected.as_str()) {
        let active = resolved_scene_context(compiled).scene_id;
        return Err(AppError::status(
            StatusCode::BAD_REQUEST,
            format!(
                "dataset `{}` is not available in scene `{expected}` (active scene is `{active}`)",
                resource.id
            ),
        ));
    }

    Ok(resource)
}
