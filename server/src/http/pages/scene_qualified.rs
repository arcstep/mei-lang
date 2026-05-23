//! Scene-qualified compile options and dataset/metric lookup for runtime APIs.

use axum::http::StatusCode;
use mei_lang_kernel::{
    locate_dataset_resource as kernel_locate_dataset_resource, CompileOptions,
    CompiledApp, LoadedResource, RuntimeResourceResolveError,
};

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

/// Locate a dataset resource within the compiled active scene resource table.
pub fn locate_dataset_resource<'a>(
    compiled: &'a CompiledApp,
    dataset_id: &str,
    expected_scene_id: Option<&str>,
) -> Result<&'a LoadedResource, AppError> {
    let resource = kernel_locate_dataset_resource(compiled, dataset_id)
        .map_err(map_resolve_error)?;

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
                    "dataset `{}` is not available in scene `{expected}` (active scene is `{active}`)",
                    resource.id
                ),
            ));
        }
    }

    Ok(resource)
}

#[cfg(test)]
mod tests {
    use super::locate_dataset_resource;
    use mei_lang_kernel::{
        CompiledApp, CompiledSceneRoute, DatasetView, LoadedResource, SourceDecl,
    };
    use serde_json::json;

    fn sample_dataset_resource(id: &str) -> LoadedResource {
        LoadedResource {
            id: id.to_string(),
            kind: "dataset".to_string(),
            title: None,
            document: None,
            dataset: Some(DatasetView {
                id: id.to_string(),
                title: None,
                purpose: None,
                schema: Vec::new(),
                stage_schema: Vec::new(),
                columns: vec!["a".to_string()],
                rows: vec![json!({"a": 1})],
                source: SourceDecl {
                    kind: "csv".to_string(),
                    path: format!("data/{id}.csv"),
                    sheet: None,
                    header_row: None,
                    preview_rows: None,
                    page_size: None,
                    max_page_size: None,
                    table: None,
                    query: None,
                    connection: None,
                    content: None,
                },
                sources: Vec::new(),
                metrics: Default::default(),
                runtime_metric_defs: Default::default(),
            }),
        }
    }

    fn sample_compiled() -> CompiledApp {
        CompiledApp {
            app_id: "demo".to_string(),
            active_scene: Some("home".to_string()),
            active_target_file: "scenes/home.mei".to_string(),
            resources: vec![
                sample_dataset_resource("warning_list"),
                sample_dataset_resource("home"),
            ],
            world_metrics: std::collections::BTreeMap::new(),
            scene_routes: vec![CompiledSceneRoute {
                scene_id: "home".to_string(),
                frame_id: None,
                target_file: "scenes/home.mei".to_string(),
                kind: "file_ref".to_string(),
                title: None,
                is_default: true,
                access_export: true,
            }],
            app_root: ".".to_string(),
            title: "demo".to_string(),
            file_tree: Vec::new(),
            scene_contract: None,
            component_assets: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    #[test]
    fn locate_dataset_accepts_route_target_alias() {
        let compiled = sample_compiled();
        let resource =
            locate_dataset_resource(&compiled, "scenes/home.mei", Some("home")).expect("alias");
        assert_eq!(resource.id, "home");
    }

    #[test]
    fn locate_dataset_accepts_canonical_resource_id() {
        let compiled = sample_compiled();
        let resource = locate_dataset_resource(&compiled, "warning_list", None).expect("id");
        assert_eq!(resource.id, "warning_list");
    }
}
