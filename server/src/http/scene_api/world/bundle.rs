use std::path::Path;
use std::time::Instant;

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

fn log_bundle_loaded(
    app_id: &str,
    requested_scene: Option<&str>,
    requested_target: Option<&str>,
    strategy: &str,
    fallback_compile: bool,
    bundle: &WorldRuntimeBundle,
    started: Instant,
) {
    tracing::info!(
        app_id = %app_id,
        requested_scene = %requested_scene.unwrap_or("-"),
        requested_target = %requested_target.unwrap_or("-"),
        active_scene = %bundle.compiled.active_scene.as_deref().unwrap_or("-"),
        active_target_file = %bundle.active_target_file,
        compile_strategy = %strategy,
        fallback_compile,
        total_ms = started.elapsed().as_millis() as u64,
        "world runtime bundle loaded"
    );
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
    let load_started = Instant::now();
    let scope = normalize_world_scope(scope);
    let requested_scene = scope.scene_id.as_deref();
    let requested_target = scope.target_file.clone();
    let app_root = source_root.join(app_id);
    let mut fallback_compile = false;
    tracing::info!(
        app_id = %app_id,
        requested_scene = %requested_scene.unwrap_or("-"),
        requested_target = %requested_target.as_deref().unwrap_or("-"),
        "world runtime bundle loading"
    );

    // 单编路径：manage/API 仅按 `.mei` 预览或显式 scene 请求时，避免「基线 + 目标」双次整包编译。
    // L1/L2/L3 仍会在 miss 时复用 scene payload 与数据行缓存。
    if requested_scene.is_none() {
        if let Some(target) = requested_target.as_deref().filter(|t| is_mei_target(t)) {
            let preview_target = resolve_preview_target(app_id, target);
            let compiled = compile(CompileOptions {
                scene: None,
                preview_target: preview_target.clone(),
            })?;
            let bundle = finish_bundle(compiled, app_id)?;
            log_bundle_loaded(
                app_id,
                requested_scene,
                requested_target.as_deref(),
                "target_preview_single_compile",
                fallback_compile,
                &bundle,
                load_started,
            );
            return Ok(bundle);
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
            fallback_compile = true;
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
            let bundle = finish_bundle(compiled, app_id)?;
            log_bundle_loaded(
                app_id,
                requested_scene,
                requested_target.as_deref(),
                "scene_route_fallback_compile",
                fallback_compile,
                &bundle,
                load_started,
            );
            return Ok(bundle);
        }
        let bundle = finish_bundle(compiled, app_id)?;
        log_bundle_loaded(
            app_id,
            requested_scene,
            requested_target.as_deref(),
            "scene_single_compile",
            fallback_compile,
            &bundle,
            load_started,
        );
        return Ok(bundle);
    }

    let compiled = compile(CompileOptions::default())?;
    let bundle = finish_bundle(compiled, app_id)?;
    log_bundle_loaded(
        app_id,
        requested_scene,
        requested_target.as_deref(),
        "default_compile",
        fallback_compile,
        &bundle,
        load_started,
    );
    Ok(bundle)
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
