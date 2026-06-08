use std::path::Path;

use crate::compile::entry_payload::clone_merge::{normalize_frame_decl, normalize_world_decl};
use crate::compile::load_external::{load_frame_from_file, load_world_from_file};
use crate::compile::scene_binding::{
    parse_frame_binding, parse_world_binding, pick_only_frame, pick_only_world, SceneBinding,
};
use crate::model::{CompiledSceneRoute, Diagnostic, Severity};
use crate::typed_refs::SceneRegistry;

use super::super::flow_binding::resolve_flow_binding;
use super::super::scene_binding::validate_scene_binding_contract;
use super::diagnostic::push_deprecated_ref_binding_diagnostic;
use super::state::CompileSceneCtx;

pub(super) fn resolve_bindings(
    ctx: &mut CompileSceneCtx,
    app_root: &Path,
    target_file: &str,
    route_meta: Option<&CompiledSceneRoute>,
    scene_registry: &SceneRegistry,
) -> anyhow::Result<()> {
    ctx.frame = if let Some(frame_id) = route_meta
        .and_then(|meta| meta.frame_id.as_deref())
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(|id| id.to_string())
    {
        let matched = ctx.frames.get(frame_id.as_str()).cloned();
        if matched.is_none() {
            ctx.diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "missing_bound_frame".to_string(),
                message: format!("declared frame `{frame_id}` was not found"),
                source_path: Some(target_file.to_string()),
            });
        }
        matched
    } else if let Some(scene_decl) = ctx.selected_scene.as_ref() {
        let binding = scene_decl
            .frame
            .as_ref()
            .map(|value| parse_frame_binding(value, Some(scene_registry)));
        match binding {
            Some(Ok(SceneBinding::LocalId(frame_id))) => {
                let matched = ctx.frames.get(frame_id.as_str()).cloned();
                if matched.is_none() {
                    ctx.diagnostics.push(Diagnostic {
                        severity: Severity::Error,
                        code: "missing_bound_frame".to_string(),
                        message: format!("declared frame `{frame_id}` was not found"),
                        source_path: Some(target_file.to_string()),
                    });
                }
                matched
            }
            Some(Ok(SceneBinding::FileRef {
                path,
                id,
                compat_source,
            })) => {
                push_deprecated_ref_binding_diagnostic(
                    &mut ctx.diagnostics,
                    compat_source.as_deref(),
                    target_file,
                );
                match load_frame_from_file(app_root, path.as_str(), id.as_deref()) {
                    Ok(frame_decl) => {
                        if frame_decl.base.is_some() {
                            let frame_value = serde_json::to_value(&frame_decl)?;
                            normalize_frame_decl(
                                app_root,
                                frame_decl,
                                &frame_value,
                                scene_registry,
                                &mut ctx.diagnostics,
                                target_file,
                            )
                        } else {
                            Some(frame_decl)
                        }
                    }
                    Err(error) => {
                        ctx.diagnostics.push(Diagnostic {
                            severity: Severity::Error,
                            code: "load_frame_ref_failed".to_string(),
                            message: error.to_string(),
                            source_path: Some(target_file.to_string()),
                        });
                        None
                    }
                }
            }
            Some(Ok(SceneBinding::Absent)) => {
                pick_only_frame(&ctx.frames, ctx.frame_default.clone())
            }
            Some(Err(message)) => {
                ctx.diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    code: "invalid_scene_frame_binding".to_string(),
                    message: message.to_string(),
                    source_path: Some(target_file.to_string()),
                });
                None
            }
            None => pick_only_frame(&ctx.frames, ctx.frame_default.clone()),
        }
    } else {
        pick_only_frame(&ctx.frames, ctx.frame_default.clone())
    };
    if ctx.selected_scene.is_some() && ctx.frame.is_none() && ctx.frame_decl_count == 0 {
        ctx.diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "missing_frame".to_string(),
            message: "scene route requires a frame(...) declaration or frame_ref(...)".to_string(),
            source_path: Some(target_file.to_string()),
        });
    }

    ctx.world = if let Some(scene_decl) = ctx.selected_scene.as_ref() {
        let binding = scene_decl
            .world
            .as_ref()
            .map(|value| parse_world_binding(value, Some(scene_registry)));
        match binding {
            Some(Ok(SceneBinding::LocalId(world_id))) => {
                let matched = ctx.worlds.get(world_id.as_str()).cloned();
                if matched.is_none() {
                    ctx.diagnostics.push(Diagnostic {
                        severity: Severity::Error,
                        code: "missing_bound_world".to_string(),
                        message: format!("declared world `{world_id}` was not found"),
                        source_path: Some(target_file.to_string()),
                    });
                }
                matched
            }
            Some(Ok(SceneBinding::FileRef {
                path,
                id,
                compat_source,
            })) => {
                push_deprecated_ref_binding_diagnostic(
                    &mut ctx.diagnostics,
                    compat_source.as_deref(),
                    target_file,
                );
                match load_world_from_file(app_root, path.as_str(), id.as_deref()) {
                    Ok(world_decl) => {
                        let world_value = serde_json::to_value(&world_decl)?;
                        normalize_world_decl(
                            app_root,
                            world_decl,
                            &world_value,
                            scene_registry,
                            &mut ctx.diagnostics,
                            target_file,
                        )
                    }
                    Err(error) => {
                        ctx.diagnostics.push(Diagnostic {
                            severity: Severity::Error,
                            code: "load_world_ref_failed".to_string(),
                            message: error.to_string(),
                            source_path: Some(target_file.to_string()),
                        });
                        None
                    }
                }
            }
            Some(Ok(SceneBinding::Absent)) => {
                pick_only_world(&ctx.worlds, ctx.world_default.clone())
            }
            Some(Err(message)) => {
                ctx.diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    code: "invalid_scene_world_binding".to_string(),
                    message: message.to_string(),
                    source_path: Some(target_file.to_string()),
                });
                None
            }
            None => pick_only_world(&ctx.worlds, ctx.world_default.clone()),
        }
    } else {
        pick_only_world(&ctx.worlds, ctx.world_default.clone())
    };
    if ctx.selected_scene.is_some() && ctx.world.is_none() && ctx.world_decl_count == 0 {
        ctx.diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "missing_world".to_string(),
            message: "scene entry requires a world(...) declaration or world_ref(...)".to_string(),
            source_path: Some(target_file.to_string()),
        });
    }
    if let (Some(scene_decl), Some(world_decl)) = (ctx.selected_scene.as_ref(), ctx.world.as_ref())
    {
        validate_scene_binding_contract(scene_decl, world_decl, target_file, &mut ctx.diagnostics);
    }

    ctx.flow = ctx
        .selected_scene
        .as_ref()
        .and_then(|scene| scene.flow.as_ref())
        .and_then(|value| {
            resolve_flow_binding(
                value,
                &ctx.flows,
                app_root,
                Some(scene_registry),
                &mut ctx.diagnostics,
                target_file,
            )
        })
        .or_else(|| {
            ctx.flow_default.clone().or_else(|| {
                (ctx.flows.len() == 1)
                    .then(|| ctx.flows.values().next().cloned())
                    .flatten()
            })
        });

    Ok(())
}
