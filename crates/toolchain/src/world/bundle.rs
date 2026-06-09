use std::path::Path;
use std::time::Instant;

use anyhow::{anyhow, Result};
use mei_lang_kernel::{initial_runtime_state, project_runtime_view, CompiledApp};

use crate::types::{ResourceQueryToolSpec, WorldRuntimeBundle, WorldScope};

fn normalize_scope_field(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_string)
}

pub(crate) fn normalize_world_scope(scope: Option<&WorldScope>) -> WorldScope {
    WorldScope {
        scene_id: normalize_scope_field(scope.and_then(|item| item.scene_id.as_deref())),
        target_file: normalize_scope_field(scope.and_then(|item| item.target_file.as_deref())),
    }
}

pub fn normalize_path(value: &str) -> String {
    value
        .trim()
        .replace('\\', "/")
        .trim_start_matches("./")
        .to_string()
}

pub(crate) fn app_relative_mei_for_preview(app_id: &str, target_file: &str) -> Option<String> {
    let mut target = normalize_path(target_file);
    if !target.ends_with(".mei") {
        return None;
    }
    let prefix = format!("{}/", app_id.trim_end_matches('/'));
    if target.starts_with(&prefix) {
        target = target[prefix.len()..].to_string();
    }
    if target.is_empty() {
        return None;
    }
    Some(target)
}

fn is_mei_target(target: &str) -> bool {
    target.to_ascii_lowercase().ends_with(".mei")
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
        "toolchain world runtime bundle loaded"
    );
}

pub fn load_world_runtime_bundle_with<F>(
    source_root: &Path,
    app_id: &str,
    scope: Option<&WorldScope>,
    mut compile: F,
) -> Result<WorldRuntimeBundle>
where
    F: FnMut(mei_lang_kernel::CompileOptions) -> Result<CompiledApp>,
{
    let load_started = Instant::now();
    let scope = normalize_world_scope(scope);
    let requested_scene = scope.scene_id.as_deref();
    let requested_target = scope.target_file.clone();
    let app_root = mei_lang_kernel::resolve_app_root(source_root, app_id);
    let mut fallback_compile = false;

    if requested_scene.is_none() {
        if let Some(target) = requested_target
            .as_deref()
            .filter(|target| is_mei_target(target))
        {
            let compiled = compile(mei_lang_kernel::CompileOptions {
                scene: None,
                preview_target: resolve_preview_target(app_id, target),
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
            .filter(|target| is_mei_target(target))
            .and_then(|target| resolve_preview_target(app_id, target));
        let compiled = compile(mei_lang_kernel::CompileOptions {
            scene: Some(scene_id.to_string()),
            preview_target,
        })?;
        if compiled.active_scene.as_deref() != Some(scene_id) {
            fallback_compile = true;
            let baseline = compile(mei_lang_kernel::CompileOptions::default())?;
            let route = baseline
                .scene_routes
                .iter()
                .find(|route| route.scene_id == scene_id)
                .ok_or_else(|| anyhow!("scene `{scene_id}` not found in app `{app_id}`"))?;
            let preview_target = requested_target
                .as_deref()
                .filter(|target| is_mei_target(target))
                .and_then(|target| {
                    let normalized = normalize_path(target);
                    if normalized == normalize_path(route.target_file.as_str()) {
                        None
                    } else if app_root
                        .join(app_relative_mei_for_preview(app_id, target).unwrap_or(normalized))
                        .is_file()
                    {
                        resolve_preview_target(app_id, target)
                    } else {
                        None
                    }
                });
            let compiled = compile(mei_lang_kernel::CompileOptions {
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

    let compiled = compile(mei_lang_kernel::CompileOptions::default())?;
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

pub fn load_world_runtime_bundle(
    source_root: &Path,
    app_id: &str,
    scope: Option<&WorldScope>,
) -> Result<WorldRuntimeBundle> {
    let components_root = crate::resolve_components_root(source_root);
    load_world_runtime_bundle_with(source_root, app_id, scope, |options| {
        crate::compile_app_with_cache(source_root, app_id, options, components_root.as_path())
            .map(|outcome| outcome.compiled)
            .map_err(|failure| failure.error)
    })
}

pub fn default_resource_query_tools() -> Vec<ResourceQueryToolSpec> {
    crate::access_host_bound_query_tools()
}
