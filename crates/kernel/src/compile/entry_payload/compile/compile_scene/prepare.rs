use std::collections::BTreeSet;

use crate::compile::entry_payload::helpers::collect_asset_keys_from_nodes;
use crate::config_refs::{decode_theme_ref_token, theme_decl_from_value};
use crate::model::{CompiledSceneRoute, ComponentAsset, Diagnostic, Severity};

use super::state::CompileSceneCtx;

pub(super) fn prepare_scene_selection(
    ctx: &mut CompileSceneCtx,
    asset_map: &std::collections::BTreeMap<String, ComponentAsset>,
    target_file: &str,
    route_meta: Option<&CompiledSceneRoute>,
) {
    let mut asset_keys = BTreeSet::new();
    for panel in &ctx.panels {
        collect_asset_keys_from_nodes(&panel.blocks, &mut asset_keys);
    }
    ctx.component_assets = asset_keys
        .into_iter()
        .filter_map(|key| asset_map.get(&key).cloned())
        .collect();

    ctx.selected_scene = route_meta
        .and_then(|meta| ctx.scenes.get(meta.scene_id.as_str()).cloned())
        .or_else(|| {
            if ctx.scenes.len() == 1 {
                ctx.scenes.values().next().cloned()
            } else {
                None
            }
        });
    if let Some(scene_decl) = ctx.selected_scene.as_ref() {
        if let Some(theme_token) = scene_decl.theme.as_deref() {
            if let Some(theme_id) = decode_theme_ref_token(theme_token) {
                if !ctx.themes.iter().any(|item| item.id == theme_id) {
                    let resolver = crate::config_refs::ConfigRefResolver::new(&ctx.config);
                    if let Some(theme_value) = resolver.resolve_theme_token(theme_token) {
                        let theme_value = resolver.resolve_config_refs_in_value(
                            &theme_value,
                            target_file,
                            &mut ctx.diagnostics,
                        );
                        match theme_decl_from_value(theme_id.as_str(), theme_value) {
                            Ok(theme_decl) => ctx.themes.push(theme_decl),
                            Err(message) => ctx.diagnostics.push(Diagnostic {
                                severity: Severity::Error,
                                code: "invalid_config_ref".to_string(),
                                message,
                                source_path: Some(target_file.to_string()),
                            }),
                        }
                    }
                }
            }
        }
    }
    let requires_scene_contract = (route_meta.is_some() || target_file != ctx.app_entry_main)
        && !ctx.dataset_library_only;
    if requires_scene_contract && ctx.selected_scene.is_none() {
        let is_legacy_fragment = ctx.frame_decl_count > 0
            || !ctx.panels.is_empty()
            || ctx.world_decl_count > 0
            || ctx.frame_default.is_some()
            || ctx.world_default.is_some();
        if is_legacy_fragment {
            ctx.diagnostics.push(Diagnostic {
                severity: Severity::Warning,
                code: "public_fragment_file_deprecated".to_string(),
                message: "legacy frame/world/panel fragment without scene(...); migrate to a minimal scene capsule with scene(...) and typed refs (world_ref/frame_ref)".to_string(),
                source_path: Some(target_file.to_string()),
            });
        }
        ctx.diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "missing_scene".to_string(),
            message: "scene file must declare scene(...) for scene-first authoring".to_string(),
            source_path: Some(target_file.to_string()),
        });
    }
}
