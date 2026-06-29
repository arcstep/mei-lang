use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use mei_bundle::{read_bundle, MeiCompileExchange};
use mei_graph::GraphBlock;
use mei_host_core::{HostContext, ImportReport};
use mei_lang_kernel::{resolve_app_eval_cache_root, resolve_app_registry_root, resolve_app_root};
use serde_json::{json, Value};

use crate::bridge::export_bridge_from_mcg;
use crate::content_store::{
    self, APP_SKELETON, METRIC_DEF_BUNDLE, NAVIGATION, PANEL_CONTRACT, PROJECTION_ASSEMBLY,
    WARMUP_POLICY,
};
use crate::mcg::registry::{McgNodeRecord, McgRegistryWriter};
use crate::types::{stable_hash, GraphNodeId, GraphNodeKind, MaterialState, PayloadRef};

#[derive(Debug, Clone, Default)]
pub struct ImportOptions {
    pub bundle_path: Option<std::path::PathBuf>,
}

pub fn import_bundle(ctx: &HostContext, options: &ImportOptions) -> Result<ImportReport> {
    let bundle_path = options
        .bundle_path
        .clone()
        .unwrap_or_else(|| ctx.bundle_path());
    let (manifest, blocks) = read_bundle(&bundle_path)
        .with_context(|| format!("read bundle {}", bundle_path.display()))?;
    let exchange = mei_bundle::MeiCompileExchange {
        app_id: manifest.app_id.clone(),
        syntax_version: manifest.syntax_version.clone(),
        graph_schema_version: manifest.graph_schema_version.clone(),
        blocks,
        sources: manifest.sources.clone(),
    };
    import_exchange(ctx, &exchange)
}

pub fn import_exchange(ctx: &HostContext, exchange: &MeiCompileExchange) -> Result<ImportReport> {
    let app_root = resolve_app_root(ctx.workspace_root.as_path(), ctx.app_id.as_str());
    std::fs::create_dir_all(&app_root)?;
    let build_root = mei_lang_kernel::resolve_app_build_root(app_root.as_path());
    std::fs::create_dir_all(build_root.join("exchange"))?;
    std::fs::create_dir_all(resolve_app_registry_root(&app_root))?;
    std::fs::create_dir_all(resolve_app_eval_cache_root(&app_root))?;

    let mut registry = McgRegistryWriter::load(ctx.workspace_root.as_path(), ctx.app_id.as_str());
    let mut cas_upserts = 0usize;
    let warnings = Vec::new();
    let mut bundle_owners = BTreeMap::new();

    for block in &exchange.blocks {
        let (kind, schema_version) = cas_kind_for_block(block);
        let revision = block_revision(block);
        let artifact = wrap_block_artifact(block, &revision);
        let bytes = serde_json::to_vec(&artifact)?;
        let put = content_store::put_if_absent(app_root.as_path(), kind, &bytes)?;
        if put.created {
            cas_upserts += 1;
        }

        let node_kind = GraphNodeKind::from_block_kind(block.kind.as_str());
        let node_key = node_key_for_block(block);
        let owner = owner_resource_for_block(block);

        if node_kind == GraphNodeKind::MetricDefBundle {
            if let Some(owner_id) = owner.clone() {
                bundle_owners.insert(
                    owner_id,
                    (
                        revision.clone(),
                        stable_hash(&serde_json::to_string(&block.payload).unwrap_or_default()),
                    ),
                );
            }
        }

        registry.upsert_node(McgNodeRecord {
            id: GraphNodeId::new(node_kind, node_key),
            revision,
            state: MaterialState::Ready,
            layer: "import".to_string(),
            payload_ref: Some(PayloadRef::new(kind, put.content_hash, schema_version)),
            deps: Vec::new(),
            owner_resource_id: owner,
            assembly_inputs: Vec::new(),
        });
    }

    registry.finalize();
    McgRegistryWriter::save(ctx.workspace_root.as_path(), &registry)?;

    let bridge = export_bridge_from_mcg(ctx.app_id.as_str(), &registry, &bundle_owners);
    crate::bridge::save_bridge(ctx.workspace_root.as_path(), &bridge)?;

    let stale_count = crate::mrg::slots::mark_slots_stale_for_bundles(
        ctx.workspace_root.as_path(),
        ctx.app_id.as_str(),
        &bundle_owners.keys().cloned().collect::<Vec<_>>(),
    )?;
    if stale_count > 0 {
        let mrg = crate::mrg::registry::MrgRegistryWriter::load(
            ctx.workspace_root.as_path(),
            ctx.app_id.as_str(),
        );
        let cleared = crate::mrg::client_bootstrap::clear_client_bootstraps_for_stale_scopes(
            app_root.as_path(),
            &mrg,
        );
        let _ = (stale_count, cleared);
    }

    Ok(ImportReport {
        app_id: exchange.app_id.clone(),
        block_count: exchange.blocks.len(),
        cas_upserts,
        mcg_nodes: registry.nodes.len(),
        registry_revision: registry.registry_revision.clone(),
        index_by_kind: mei_bundle::index_by_kind(&exchange.blocks),
        warnings,
    })
}

fn wrap_block_artifact(block: &GraphBlock, revision: &str) -> Value {
    json!({
        "schemaVersion": block.schema,
        "blockId": block.block_id,
        "kind": block.kind,
        "revision": revision,
        "payload": block.payload,
    })
}

fn cas_kind_for_block(block: &GraphBlock) -> (&'static str, &'static str) {
    match block.kind.as_str() {
        "app_skeleton" => (APP_SKELETON, "mei-app-skeleton-artifact-v1"),
        "panel_contract" => (PANEL_CONTRACT, "mei-panel-contract-artifact-v1"),
        "metric_def_bundle" => (METRIC_DEF_BUNDLE, "mei-metric-def-bundle-artifact-v1"),
        "assembly_view" | "board_assembly" => (PROJECTION_ASSEMBLY, "mei-projection-assembly-v1"),
        "navigation" | "link_decl" => (NAVIGATION, "mei-navigation-artifact-v1"),
        "warmup_policy" => (WARMUP_POLICY, "mei-warmup-policy-artifact-v1"),
        _ => (PROJECTION_ASSEMBLY, "mei-graph-block-v2"),
    }
}

fn block_revision(block: &GraphBlock) -> String {
    let payload_text = serde_json::to_string(&block.payload).unwrap_or_default();
    format!(
        "blk:{}",
        stable_hash(&format!("{}\n{}", block.block_id, payload_text))
    )
}

fn node_key_for_block(block: &GraphBlock) -> String {
    if let Some(key) = block.payload.get("key").and_then(|v| v.as_str()) {
        return key.to_string();
    }
    block.block_id.clone()
}

fn owner_resource_for_block(block: &GraphBlock) -> Option<String> {
    match block.kind.as_str() {
        "metric_def_bundle" => block
            .payload
            .get("key")
            .and_then(|v| v.as_str())
            .map(|key| format!("__world_metrics__::{key}")),
        _ => None,
    }
}

pub fn load_block_artifact(app_root: &Path, pref: &PayloadRef) -> Result<Option<Value>> {
    content_store::read_payload_json(app_root, pref)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mei_graph::GraphBlock;
    use serde_json::json;

    #[test]
    fn node_key_prefers_payload_key() {
        let block = GraphBlock {
            kind: "assembly_view".to_string(),
            block_id: "assembly_view:home@src/scene/home/assembly.mei".to_string(),
            schema: "mei-projection-assembly-v1".to_string(),
            payload: json!({"key": "home@src/scene/home/assembly.mei", "scene": "home"}),
        };
        assert_eq!(
            node_key_for_block(&block),
            "home@src/scene/home/assembly.mei"
        );
    }
}
