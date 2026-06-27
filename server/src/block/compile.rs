use std::path::Path;
use std::time::Instant;

use anyhow::{anyhow, Result};
use mei_lang_toolchain::resolve_components_root;

use crate::graph::types::GraphNodeKind;
use crate::graph::{
    hydrate_compiled_for_prebuild_eval, maybe_update_graph_after_compile,
    runtime_payloads_from_compiled, try_assemble_scope_from_scene_payload,
};
use crate::prebuild::{ensure_compile_scope, CompileScope, PrebuildMode};

use super::types::{BlockId, BlockResult, BlockTimingMs};

/// Assemble-only: load scene_payload + hydrate without compile cache write.
pub fn block_assemble_only(source_root: &Path, app_id: &str, block_id: &BlockId) -> Result<BlockResult> {
    let started = Instant::now();
    let target = block_id.key.as_str();
    let scope_key = block_id.scope_key.as_deref();
    let assemble_started = Instant::now();
    let Some((mut compiled, compile_revision)) = try_assemble_scope_from_scene_payload(
        source_root,
        app_id,
        scope_key,
        target,
    ) else {
        return Ok(BlockResult::err(
            block_id.clone(),
            "assemble-only",
            &anyhow!("scene_payload assemble-only miss for `{target}`"),
        ));
    };
    let assemble_ms = assemble_started.elapsed().as_millis() as u64;
    hydrate_compiled_for_prebuild_eval(source_root, app_id, &mut compiled, &[], &[])?;
    let options = CompileScope {
        requested_scene_id: block_id.scope_key.clone(),
        requested_target_file: Some(block_id.key.clone()),
    }
    .canonicalized()
    .to_options();
    let payloads = runtime_payloads_from_compiled(&compiled);
    maybe_update_graph_after_compile(
        source_root,
        app_id,
        &options,
        &compiled,
        compile_revision.as_str(),
        &payloads,
    );
    let mut result = BlockResult::ok(block_id.clone(), "assemble-only");
    result.output_revision = Some(compile_revision);
    result.timing = BlockTimingMs {
        compile_ms: assemble_ms,
        total_ms: started.elapsed().as_millis() as u64,
        ..Default::default()
    };
    Ok(result)
}

pub fn block_compile(source_root: &Path, app_id: &str, block_id: &BlockId) -> Result<BlockResult> {
    let started = Instant::now();
    match block_id.kind {
        GraphNodeKind::ScenePayload => {
            let scope = CompileScope {
                requested_scene_id: block_id.scope_key.clone(),
                requested_target_file: Some(block_id.key.clone()),
            }
            .canonicalized();
            let components_root = resolve_components_root(source_root);
            let compile_started = Instant::now();
            let outcome = ensure_compile_scope(
                source_root,
                app_id,
                &scope,
                PrebuildMode::Build,
                components_root.as_path(),
            )?;
            let compile_ms = compile_started.elapsed().as_millis() as u64;
            let options = scope.to_options();
            let payloads = runtime_payloads_from_compiled(&outcome.compiled);
            maybe_update_graph_after_compile(
                source_root,
                app_id,
                &options,
                &outcome.compiled,
                outcome.compile_revision.as_str(),
                &payloads,
            );
            let mut result = BlockResult::ok(block_id.clone(), "compile");
            result.output_revision = Some(outcome.compile_revision.clone());
            result.timing = BlockTimingMs {
                compile_ms,
                total_ms: started.elapsed().as_millis() as u64,
                ..Default::default()
            };
            Ok(result)
        }
        other => Err(anyhow!(
            "block compile not supported for kind `{}`",
            other.slug()
        )),
    }
}
