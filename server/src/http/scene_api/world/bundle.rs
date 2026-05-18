use std::path::Path;

use anyhow::{anyhow, Result};
use mei_lang_kernel::{
    compile_app_with_options, initial_runtime_state, project_runtime_view, CompileOptions,
    CompiledApp,
};

use crate::{
    http::{compile_cache::compile_app_with_cache, pages::resolve_components_root},
    AppState,
};

use crate::http::scene_api::types::{WorldRuntimeBundle, WorldScope};
use super::util::{app_relative_mei_for_preview, normalize_path, normalize_world_scope};

pub(super) fn load_world_runtime_bundle_with<F>(
    source_root: &Path,
    app_id: &str,
    scope: Option<&WorldScope>,
    mut compile: F,
) -> Result<WorldRuntimeBundle>
where
    F: FnMut(CompileOptions) -> Result<CompiledApp>,
{
    let scope = normalize_world_scope(scope);
    let requested_scene = scope.scene_id.as_deref();
    let requested_target = scope.target_file.clone();

    let base_compiled = compile(CompileOptions {
        scene: None,
        preview_target: None,
    })?;
    let app_root = source_root.join(app_id);
    let mut selected_scene: Option<String> = None;

    if let Some(scene_id) = requested_scene {
        let by_scene = base_compiled
            .scene_routes
            .iter()
            .find(|item| item.scene_id == scene_id)
            .ok_or_else(|| anyhow!("scene `{scene_id}` not found in app `{app_id}`"))?;
        if let Some(target_file) = requested_target.as_deref() {
            let nt = normalize_path(target_file);
            let matches_target = nt == normalize_path(by_scene.target_file.as_str());
            if matches_target {
                selected_scene = Some(by_scene.scene_id.clone());
            } else if let Some(by_target) = base_compiled
                .scene_routes
                .iter()
                .find(|e| normalize_path(e.target_file.as_str()) == nt)
            {
                if by_target.scene_id != by_scene.scene_id {
                    return Err(anyhow!(
                        "scene `{scene_id}` is not bound to target `{target_file}`"
                    ));
                }
                selected_scene = Some(by_target.scene_id.clone());
            } else if let Some(rel) = app_relative_mei_for_preview(app_id, target_file) {
                if let Some(by_target) = base_compiled
                    .scene_routes
                    .iter()
                    .find(|e| normalize_path(e.target_file.as_str()) == normalize_path(&rel))
                {
                    if by_target.scene_id != by_scene.scene_id {
                        return Err(anyhow!(
                            "scene `{scene_id}` is not bound to target `{target_file}`"
                        ));
                    }
                    selected_scene = Some(by_target.scene_id.clone());
                } else if app_root.join(&rel).is_file() {
                    selected_scene = None;
                } else {
                    return Err(anyhow!(
                        "scene `{scene_id}` is not bound to target `{target_file}`"
                    ));
                }
            } else {
                return Err(anyhow!(
                    "scene `{scene_id}` is not bound to target `{target_file}`"
                ));
            }
        } else {
            selected_scene = Some(by_scene.scene_id.clone());
        }
    } else if let Some(target_only) = requested_target.as_deref() {
        let nt = normalize_path(target_only);
        if let Some(found) = base_compiled
            .scene_routes
            .iter()
            .find(|item| normalize_path(item.target_file.as_str()) == nt)
        {
            selected_scene = Some(found.scene_id.clone());
        } else if let Some(rel) = app_relative_mei_for_preview(app_id, target_only) {
            if let Some(found) = base_compiled
                .scene_routes
                .iter()
                .find(|e| normalize_path(e.target_file.as_str()) == normalize_path(&rel))
            {
                selected_scene = Some(found.scene_id.clone());
            }
        }
    }

    let preview_path = if selected_scene.is_some() {
        None
    } else {
        requested_target.as_deref().and_then(|target| {
            if !target.to_lowercase().ends_with(".mei") {
                return None;
            }
            app_relative_mei_for_preview(app_id, target).or_else(|| Some(normalize_path(target)))
        })
    };

    let compiled = compile(CompileOptions {
        scene: selected_scene.clone(),
        preview_target: preview_path,
    })?;
    if let Some(sid) = selected_scene.as_deref() {
        if compiled.active_scene.as_deref() != Some(sid) {
            return Err(anyhow!("scene `{sid}` not found in app `{app_id}`"));
        }
    }
    let contract = compiled
        .scene_contract
        .clone()
        .ok_or_else(|| anyhow!("app `{}` does not provide a scene contract", app_id))?;
    let state = initial_runtime_state(&contract, 1);
    let scene_view = project_runtime_view(&contract, &state);
    let active_target_file = compiled.active_target_file.clone();
    Ok(WorldRuntimeBundle {
        compiled,
        active_target_file,
        contract,
        state,
        scene_view,
    })
}

pub(super) fn load_world_runtime_bundle(
    source_root: &Path,
    app_id: &str,
    scope: Option<&WorldScope>,
) -> Result<WorldRuntimeBundle> {
    load_world_runtime_bundle_with(source_root, app_id, scope, |options| {
        compile_app_with_options(source_root, app_id, options)
    })
}

pub(super) fn load_world_runtime_bundle_cached(
    state: &AppState,
    app_id: &str,
    scope: Option<&WorldScope>,
) -> Result<WorldRuntimeBundle> {
    let components_root = resolve_components_root(&state.source_root);
    load_world_runtime_bundle_with(&state.source_root, app_id, scope, |options| {
        compile_app_with_cache(state, app_id, options, components_root.as_path())
            .map(|outcome| outcome.compiled)
            .map_err(|failure| failure.error)
    })
}
