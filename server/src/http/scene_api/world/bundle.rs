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

use super::util::{app_relative_mei_for_preview, normalize_path, normalize_world_scope};
use crate::http::scene_api::types::{WorldRuntimeBundle, WorldScope};

fn is_mei_target(target: &str) -> bool {
    target.to_lowercase().ends_with(".mei")
}

fn resolve_preview_target(app_id: &str, target: &str) -> Option<String> {
    if !is_mei_target(target) {
        return None;
    }
    app_relative_mei_for_preview(app_id, target).or_else(|| Some(normalize_path(target)))
}

fn finish_bundle(compiled: CompiledApp, app_id: &str) -> Result<WorldRuntimeBundle> {
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
    let app_root = source_root.join(app_id);

    // 单编路径：manage/API 仅按 `.mei` 预览或显式 scene 请求时，避免「基线 + 目标」双次整包编译。
    // L1/L2/L3 仍会在 miss 时复用 scene payload 与数据行缓存。
    if requested_scene.is_none() {
        if let Some(target) = requested_target.as_deref().filter(|t| is_mei_target(t)) {
            let preview_target = resolve_preview_target(app_id, target);
            let compiled = compile(CompileOptions {
                scene: None,
                preview_target: preview_target.clone(),
            })?;
            return finish_bundle(compiled, app_id);
        }
    }

    if let Some(scene_id) = requested_scene {
        let preview_target = requested_target
            .as_deref()
            .filter(|t| is_mei_target(t))
            .and_then(|t| resolve_preview_target(app_id, t));
        let compiled = compile(CompileOptions {
            scene: Some(scene_id.to_string()),
            preview_target,
        })?;
        if compiled.active_scene.as_deref() != Some(scene_id) {
            // 回退：用路由表做一次基线解析（仅当单编未命中 scene id 时）
            let base = compile(CompileOptions::default())?;
            let exists = base.scene_routes.iter().any(|r| r.scene_id == scene_id);
            if !exists {
                return Err(anyhow!("scene `{scene_id}` not found in app `{app_id}`"));
            }
            let route = base
                .scene_routes
                .iter()
                .find(|r| r.scene_id == scene_id)
                .ok_or_else(|| anyhow!("scene `{scene_id}` not found in app `{app_id}`"))?;
            let preview_target = requested_target
                .as_deref()
                .filter(|t| is_mei_target(t))
                .and_then(|t| {
                    let nt = normalize_path(t);
                    if nt == normalize_path(route.target_file.as_str()) {
                        None
                    } else if app_root
                        .join(app_relative_mei_for_preview(app_id, t).unwrap_or(nt))
                        .is_file()
                    {
                        resolve_preview_target(app_id, t)
                    } else {
                        None
                    }
                });
            let compiled = compile(CompileOptions {
                scene: Some(scene_id.to_string()),
                preview_target,
            })?;
            if compiled.active_scene.as_deref() != Some(scene_id) {
                return Err(anyhow!("scene `{scene_id}` not found in app `{app_id}`"));
            }
            return finish_bundle(compiled, app_id);
        }
        return finish_bundle(compiled, app_id);
    }

    let compiled = compile(CompileOptions::default())?;
    finish_bundle(compiled, app_id)
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
