use super::prelude::*;
use super::*;


pub(crate) fn ensure_compiled_app_artifact_alias(
    source_root: &Path,
    app_id: &str,
    options: &CompileOptions,
    components_root: &Path,
    compiled: &CompiledApp,
) {
    if !compiled_app_artifact_enabled() {
        return;
    }
    let revision_stamp = compile_revision(source_root, app_id, options, components_root);
    maybe_write_compiled_app_artifact(source_root, app_id, options, &revision_stamp, compiled);
}

pub(crate) fn compiled_app_artifact_lookup_scopes(
    app_root: &Path,
    options: &CompileOptions,
) -> Vec<WorldScope> {
    let scene_id = options
        .scene
        .as_deref()
        .map(str::trim)
        .filter(|scene| !scene.is_empty());
    let preview_target = normalized_scope_target(options.preview_target.as_deref());
    let has_target = preview_target.is_some();
    if scene_id.is_none() || has_target {
        let mut scopes = Vec::new();
        let mut seen = BTreeMap::<String, ()>::new();
        let mut push_scope = |scope: WorldScope| {
            let key = format!(
                "{}|{}",
                scope.scene_id.as_deref().unwrap_or(""),
                scope.target_file.as_deref().unwrap_or("")
            );
            if seen.insert(key, ()).is_some() {
                return;
            }
            scopes.push(scope);
        };
        push_scope(compiled_app_artifact_scope(options));
        if let Some(target_file) = preview_target.as_deref() {
            for scope in list_compiled_app_scopes_for_target(app_root, target_file) {
                push_scope(scope);
            }
        }
        if scene_id.is_none() {
            return scopes;
        }
        if let Ok(Some((_, default_artifact))) = read_json_artifact::<CompiledAppDiskArtifact>(
            app_root,
            COMPILED_APP_ARTIFACT_KIND,
            COMPILED_APP_ARTIFACT_NAME,
            &WorldScope {
                scene_id: None,
                target_file: None,
            },
        ) {
            for route in &default_artifact.compiled.scene_routes {
                if route.scene_id.trim() != scene_id.expect("scene id checked above") {
                    continue;
                }
                let target = route.target_file.trim();
                if target.is_empty() {
                    continue;
                }
                if preview_target.as_deref().is_some_and(|requested| requested != target) {
                    continue;
                }
                push_scope(WorldScope {
                    scene_id: Some(scene_id.expect("scene id checked above").to_string()),
                    target_file: Some(target.to_string()),
                });
            }
        }
        push_scope(WorldScope {
            scene_id: None,
            target_file: None,
        });
        return scopes;
    }
    let scene_id = scene_id.expect("scene-only lookup requires scene id");
    let mut scopes = Vec::new();
    let mut seen = BTreeMap::<String, ()>::new();
    let mut push_scope = |scope: WorldScope| {
        let key = format!(
            "{}|{}",
            scope.scene_id.as_deref().unwrap_or(""),
            scope.target_file.as_deref().unwrap_or("")
        );
        if seen.insert(key, ()).is_some() {
            return;
        }
        scopes.push(scope);
    };
    if let Ok(Some((_, default_artifact))) = read_json_artifact::<CompiledAppDiskArtifact>(
        app_root,
        COMPILED_APP_ARTIFACT_KIND,
        COMPILED_APP_ARTIFACT_NAME,
        &WorldScope {
            scene_id: None,
            target_file: None,
        },
    ) {
        for route in &default_artifact.compiled.scene_routes {
            if route.scene_id.trim() != scene_id {
                continue;
            }
            let target = route.target_file.trim();
            if target.is_empty() {
                continue;
            }
            push_scope(WorldScope {
                scene_id: Some(scene_id.to_string()),
                target_file: Some(target.to_string()),
            });
        }
        if default_artifact
            .compiled
            .active_scene
            .as_deref()
            .map(str::trim)
            == Some(scene_id)
        {
            let target = default_artifact.compiled.active_target_file.trim();
            if !target.is_empty() {
                push_scope(WorldScope {
                    scene_id: Some(scene_id.to_string()),
                    target_file: Some(target.to_string()),
                });
            }
        }
    }
    push_scope(compiled_app_artifact_scope(options));
    push_scope(WorldScope {
        scene_id: None,
        target_file: None,
    });
    scopes
}
