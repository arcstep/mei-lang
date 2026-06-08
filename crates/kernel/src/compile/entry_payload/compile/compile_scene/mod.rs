mod diagnostic;
mod finalize;
mod prepare;
mod resolve;
mod scan;
mod state;
mod validate;

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;
use serde_json::Value;

use crate::compile::entry_payload::CompiledScenePayload;
use crate::model::{CompiledSceneRoute, ComponentAsset};
use crate::typed_refs::SceneRegistry;

pub(super) use diagnostic::push_deprecated_ref_binding_diagnostic;

use state::CompileSceneCtx;

pub(crate) fn compile_scene_payload(
    app_root: &Path,
    asset_map: &BTreeMap<String, ComponentAsset>,
    target_file: &str,
    entry_decls: &Value,
    route_meta: Option<&CompiledSceneRoute>,
    scene_registry: &SceneRegistry,
) -> Result<CompiledScenePayload> {
    let mut ctx = CompileSceneCtx::new(app_root);

    scan::scan_declarations(
        &mut ctx,
        app_root,
        target_file,
        entry_decls,
        scene_registry,
    )?;
    validate::validate_and_apply_mutations(&mut ctx, app_root, target_file, scene_registry);
    prepare::prepare_scene_selection(&mut ctx, asset_map, target_file, route_meta);
    resolve::resolve_bindings(
        &mut ctx,
        app_root,
        target_file,
        route_meta,
        scene_registry,
    )?;
    finalize::finalize_payload(&mut ctx, app_root, target_file, entry_decls)
}
