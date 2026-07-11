use std::path::Path;

use anyhow::{anyhow, Result};
use mei_lang_kernel::resolve_runtime_warmup_manifest;
use mei_lang_toolchain::resolve_components_root;

use crate::graph::load_mrg_registry;
use crate::graph::observability::{run_graph_inspect, run_graph_status};
use crate::graph::types::MaterialState;
use crate::prebuild::{ensure_compile_scope, PrebuildMode, PrebuildScopeProfile};

use super::compile::block_compile;
use super::id::parse_block_id;
use super::types::{BlockId, BlockLayer, LayerStatusReport};

#[derive(Debug, Clone, Default)]
pub struct LayerCompileOptions {
    pub target_file: Option<String>,
    pub continue_on_error: bool,
}

pub fn layer_compile(
    source_root: &Path,
    app_id: &str,
    layer: BlockLayer,
    options: LayerCompileOptions,
) -> Result<Vec<super::types::BlockResult>> {
    match layer {
        BlockLayer::L3 => {
            let manifest = resolve_runtime_warmup_manifest(source_root)?
                .ok_or_else(|| anyhow!("runtime warmup manifest missing"))?;
            let app = manifest
                .apps
                .iter()
                .find(|entry| entry.app_id == app_id)
                .ok_or_else(|| anyhow!("app `{app_id}` not in warmup manifest"))?;
            let components_root = resolve_components_root(source_root);
            let mut results = Vec::new();
            if let Some(target) = options.target_file.as_deref() {
                let block_id = BlockId {
                    kind: crate::graph::types::GraphNodeKind::ScenePayload,
                    key: target.to_string(),
                    scope_key: None,
                };
                match block_compile(source_root, app_id, &block_id) {
                    Ok(result) => results.push(result),
                    Err(error) => {
                        if !options.continue_on_error {
                            return Err(error);
                        }
                        results.push(super::types::BlockResult::err(block_id, "compile", &error));
                    }
                }
                return Ok(results);
            }
            let plan =
                crate::prebuild::build_prebuild_manifest_plan(app, PrebuildScopeProfile::Full);
            let continue_on_error = options.continue_on_error;
            for scope in plan.hot_scopes.iter().chain(plan.deferred_scopes.iter()) {
                if scope.requested_target_file.is_none() {
                    continue;
                }
                let block_id = BlockId {
                    kind: crate::graph::types::GraphNodeKind::ScenePayload,
                    key: scope.requested_target_file.clone().unwrap_or_default(),
                    scope_key: scope.requested_scene_id.clone(),
                };
                match ensure_compile_scope(
                    source_root,
                    app_id,
                    scope,
                    PrebuildMode::Build,
                    components_root.as_path(),
                ) {
                    Ok(outcome) => {
                        let compile_options = scope.to_options();
                        let payloads =
                            crate::graph::runtime_payloads_from_compiled(&outcome.compiled);
                        crate::graph::maybe_update_graph_after_compile(
                            source_root,
                            app_id,
                            &compile_options,
                            &outcome.compiled,
                            outcome.compile_revision.as_str(),
                            &payloads,
                        );
                        let mut result = super::types::BlockResult::ok(block_id, "compile");
                        result.output_revision = Some(outcome.compile_revision.clone());
                        results.push(result);
                    }
                    Err(error) => {
                        if !continue_on_error {
                            return Err(error);
                        }
                        results.push(super::types::BlockResult::err(block_id, "compile", &error));
                    }
                }
            }
            Ok(results)
        }
        BlockLayer::L4 => {
            let manifest = resolve_runtime_warmup_manifest(source_root)?
                .ok_or_else(|| anyhow!("runtime warmup manifest missing"))?;
            let app = manifest
                .apps
                .iter()
                .find(|entry| entry.app_id == app_id)
                .ok_or_else(|| anyhow!("app `{app_id}` not in warmup manifest"))?;
            crate::prebuild::run_prebuild_for_app(
                source_root,
                app,
                PrebuildMode::Build,
                PrebuildScopeProfile::Full,
                false,
                None,
                true,
                None,
            )?;
            Ok(Vec::new())
        }
        BlockLayer::L2 => Err(anyhow!(
            "layer compile L2 not implemented; use graph migrate"
        )),
    }
}

pub fn layer_inspect(
    source_root: &Path,
    app_id: &str,
    layer: BlockLayer,
    node: Option<&str>,
) -> Result<serde_json::Value> {
    let layer_slug = match layer {
        BlockLayer::L3 => "mcg",
        BlockLayer::L4 => "mrg",
        BlockLayer::L2 => "navigation",
    };
    if let Some(node_raw) = node {
        let block_id = parse_block_id(node_raw)?;
        let result = super::inspect::block_inspect(source_root, app_id, &block_id)?;
        return Ok(serde_json::to_value(result)?);
    }
    let report = run_graph_inspect(source_root, app_id, layer_slug, None);
    Ok(serde_json::to_value(report)?)
}

pub fn layer_status(source_root: &Path, app_id: &str) -> Result<LayerStatusReport> {
    let status = run_graph_status(source_root, Some(app_id));
    let app_status = status
        .apps
        .into_iter()
        .find(|app| app.app_id == app_id)
        .ok_or_else(|| anyhow!("app `{app_id}` not found"))?;
    let mrg = load_mrg_registry(source_root, app_id);
    let dirty_slot_count = mrg
        .slots
        .iter()
        .filter(|slot| {
            matches!(
                slot.state,
                MaterialState::Stale | MaterialState::Missing | MaterialState::Failed
            )
        })
        .count();
    Ok(LayerStatusReport {
        app_id: app_id.to_string(),
        mcg_nodes: app_status.mcg.node_count,
        mrg_slots_ready: app_status.mrg.slot_ready,
        mrg_slots_stale: app_status.mrg.slot_stale,
        mrg_slots_failed: app_status.mrg.slot_failed,
        dirty_slot_count,
    })
}
