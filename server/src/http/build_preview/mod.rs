//! Shared Build preview compile coordinate resolution for full pages and fragments.

use mei_lang_kernel::{
    catalog_preview_target_for_build_node, compile_scene_from_build_node,
    compile_scene_from_build_node_with_app, preview_target_from_build_node,
    preview_target_from_build_node_with_app, resolve_app_root, BuildNodeId, CompileOptions,
};
use std::path::Path;

use crate::http::compile_cache::{
    resolve_runtime_compile_shared, RuntimeAccessPolicies,
};
use crate::AppState;
use mei_lang_app::UiRouteMode;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct BuildNodeCompileHints {
    pub scene: Option<String>,
    pub preview_target: Option<String>,
}

pub(crate) fn resolve_build_node_compile_hints(
    state: &AppState,
    app_id: &str,
    node: &BuildNodeId,
    components_root: &Path,
) -> BuildNodeCompileHints {
    let mut scene_hint = compile_scene_from_build_node(node);
    let mut preview_target = preview_target_from_build_node(node);
    if scene_hint.is_none() || preview_target.is_none() {
        if let Ok(Some(resolution)) = resolve_runtime_compile_shared(
            state,
            app_id,
            &CompileOptions {
                scene: scene_hint.clone(),
                preview_target: None,
                ..Default::default()
            },
            components_root,
            RuntimeAccessPolicies::default_for_access_host(),
            UiRouteMode::Build,
        ) {
            let probe = crate::http::compile_cache::compile_outcome_from_shared(resolution.outcome);
            if scene_hint.is_none() {
                scene_hint = compile_scene_from_build_node_with_app(node, Some(&probe.compiled));
            }
            if preview_target.is_none() {
                preview_target =
                    preview_target_from_build_node_with_app(node, Some(&probe.compiled));
            }
        }
    }
    if preview_target.is_none() {
        preview_target = catalog_preview_target_for_build_node(
            resolve_app_root(state.source_root.as_path(), app_id).as_path(),
            node,
        );
    }
    BuildNodeCompileHints {
        scene: scene_hint,
        preview_target,
    }
}
