use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use mei_bundle::{read_bundle, MeiCompileExchange};
use mei_graph::GraphBlock;
use mei_host_core::{HostContext, ImportReport};
use mei_lang_kernel::{
    resolve_app_eval_cache_root, resolve_app_registry_root, resolve_app_root, ObjectCatalog,
    ObjectCatalogAuthoringMode, DEFAULT_OBJECT_ASSEMBLY_KIND, OBJECT_INDEX_ENTRY_KIND,
    OBJECT_RECIPE_SCHEMA_VERSION,
};
use serde_json::{json, Value};

use crate::bridge::export_bridge_from_mcg;
use crate::content_store::{
    self, APP_SKELETON, CONTENT_PANEL, METRIC_DEF_BUNDLE, NAVIGATION, OBJECT_CATALOG,
    PROJECTION_ASSEMBLY, SEMANTIC_SCENE, WARMUP_POLICY,
};
use crate::mcg::registry::{McgNodeRecord, McgRegistryWriter};
use crate::types::{stable_hash, GraphNodeId, GraphNodeKind, MaterialState, PayloadRef};

#[derive(Debug, Clone, Default)]
pub struct ImportOptions {
    pub bundle_path: Option<std::path::PathBuf>,
}

pub fn compile_and_import_workspace(workspace_root: &Path, app_id: &str) -> Result<ImportReport> {
    let outcome = mei_graph::compile_app(workspace_root, app_id)
        .with_context(|| format!("compile native v2 graph for app `{app_id}`"))?;
    let exchange = mei_bundle::exchange_from_outcome(&outcome);
    let ctx = HostContext::new(workspace_root, app_id);
    import_exchange(&ctx, &exchange)
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
    // Replace nodes for kinds present in this bundle so renamed/moved keys do not linger.
    let kinds_in_bundle: std::collections::HashSet<GraphNodeKind> = exchange
        .blocks
        .iter()
        .map(|block| GraphNodeKind::from_block_kind(block.kind.as_str()))
        .collect();
    registry
        .nodes
        .retain(|node| !kinds_in_bundle.contains(&node.id.kind));
    let mut cas_upserts = 0usize;
    let warnings = Vec::new();
    let mut bundle_owners = BTreeMap::new();

    for block in &exchange.blocks {
        let object_catalog = decode_object_catalog_block(block)?;
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
            layer: import_layer_for_object_catalog(object_catalog.as_ref()).to_string(),
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
        "scene" => (SEMANTIC_SCENE, "mei-scene-semantic-v1"),
        "presentation" => (SEMANTIC_SCENE, "mei-presentation-semantic-v1"),
        "plane_layout" | "region_layout" | "section_layout" => {
            (SEMANTIC_SCENE, "mei-scene-layout-fragment-v1")
        }
        "slide_layout" => (SEMANTIC_SCENE, "mei-presentation-slide-fragment-v1"),
        "map_spec" => (SEMANTIC_SCENE, "mei-map-spec-v1"),
        "view_spec" => (SEMANTIC_SCENE, "mei-view-spec-v1"),
        "content_panel" => (CONTENT_PANEL, "mei-panel-contract-artifact-v1"),
        "metric_def_bundle" => (METRIC_DEF_BUNDLE, "mei-metric-def-bundle-artifact-v1"),
        "object_catalog" => (OBJECT_CATALOG, "mei-object-catalog-v1"),
        "page_instance" => (PROJECTION_ASSEMBLY, "mei-projection-assembly-v1"),
        "navigation" | "link_decl" => (NAVIGATION, "mei-navigation-artifact-v1"),
        "warmup_policy" => (WARMUP_POLICY, "mei-warmup-policy-artifact-v1"),
        _ => (PROJECTION_ASSEMBLY, "mei-graph-block-v2"),
    }
}

fn decode_object_catalog_block(block: &GraphBlock) -> Result<Option<ObjectCatalog>> {
    if block.kind != "object_catalog" {
        return Ok(None);
    }
    let catalog = serde_json::from_value::<ObjectCatalog>(block.payload.clone())
        .context("decode object_catalog payload")?;
    if catalog.authoring_mode == ObjectCatalogAuthoringMode::AuthorIntent {
        anyhow::ensure!(
            !catalog.intents.is_empty(),
            "author_intent object catalog must contain ObjectIntent"
        );
        anyhow::ensure!(
            !catalog.index.is_empty()
                && catalog
                    .index
                    .iter()
                    .all(|entry| entry.kind == OBJECT_INDEX_ENTRY_KIND),
            "author_intent object catalog must contain internal object index entries"
        );
        anyhow::ensure!(
            !catalog.default_assemblies.is_empty()
                && catalog
                    .default_assemblies
                    .iter()
                    .all(|assembly| assembly.kind == DEFAULT_OBJECT_ASSEMBLY_KIND),
            "author_intent object catalog must contain default object assemblies"
        );
        for intent in &catalog.intents {
            anyhow::ensure!(
                catalog.types.iter().any(|object_type| {
                    object_type.id == intent.object_type_id
                        && object_type.intent_id.as_deref() == Some(intent.intent_id.as_str())
                }),
                "ObjectIntent `{}` has no matching ObjectTypeContract",
                intent.intent_id
            );
            anyhow::ensure!(
                catalog.index.iter().any(|entry| {
                    entry.intent_id == intent.intent_id
                        && entry.object_type_id == intent.object_type_id
                        && entry.source == intent.source
                        && entry.recipe == intent.recipe
                }),
                "ObjectIntent `{}` has no matching internal ObjectIndexEntry",
                intent.intent_id
            );
            anyhow::ensure!(
                catalog.default_assemblies.iter().any(|assembly| {
                    assembly.intent_id == intent.intent_id
                        && assembly.recipe == intent.recipe
                        && assembly.recipe_contract.as_ref().is_some_and(|contract| {
                            contract.schema_version == OBJECT_RECIPE_SCHEMA_VERSION
                                && contract.id == format!("cockpit.{}", intent.recipe.id)
                                && contract.identity_locked
                        })
                }),
                "ObjectIntent `{}` has no matching DefaultObjectAssembly",
                intent.intent_id
            );
            anyhow::ensure!(
                intent.owner_hints.contains(&intent.source)
                    && intent.owner_hints.contains(&intent.recipe),
                "ObjectIntent `{}` owner hints must retain source and recipe owners",
                intent.intent_id
            );
        }
    }
    Ok(Some(catalog))
}

fn import_layer_for_object_catalog(catalog: Option<&ObjectCatalog>) -> &'static str {
    match catalog.map(|catalog| &catalog.authoring_mode) {
        Some(ObjectCatalogAuthoringMode::AuthorIntent) => {
            "import:object_author_intent:internal_index"
        }
        Some(ObjectCatalogAuthoringMode::Legacy) => "import:object_catalog_legacy",
        None => "import",
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
    if block.kind == "object_catalog" {
        if let Some(id) = block.payload.get("id").and_then(|value| value.as_str()) {
            return id.to_string();
        }
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
            kind: "page_instance".to_string(),
            block_id: "page_instance:home@src/scene/home/assembly.mei".to_string(),
            schema: "mei-projection-assembly-v1".to_string(),
            payload: json!({"key": "home@src/scene/home/assembly.mei", "scene": "home"}),
        };
        assert_eq!(
            node_key_for_block(&block),
            "home@src/scene/home/assembly.mei"
        );
    }

    #[test]
    fn object_catalog_has_distinct_mcg_kind_and_valid_contract() {
        let block = GraphBlock {
            kind: "object_catalog".to_string(),
            block_id: "object_catalog:warning_objects".to_string(),
            schema: "mei-object-catalog-v1".to_string(),
            payload: json!({
                "schema_version": "mei-object-catalog-v1",
                "id": "warning_objects",
                "types": [{
                    "id": "zhifa.Warning",
                    "identity": {
                        "materialization": "dataset_row",
                        "fields": ["warning_id"],
                        "normalization": null
                    },
                    "source": {
                        "role": "source",
                        "kind": "dataset_ref",
                        "id": "warning_rows",
                        "source_anchor": "domain/warnings.objects.mei"
                    },
                    "projections": [],
                    "source_anchor": "domain/warnings.objects.mei"
                }],
                "refs": [],
                "source_anchor": "domain/warnings.objects.mei"
            }),
        };
        let catalog = decode_object_catalog_block(&block).expect("valid object catalog contract");
        assert_eq!(
            import_layer_for_object_catalog(catalog.as_ref()),
            "import:object_catalog_legacy"
        );
        assert_eq!(node_key_for_block(&block), "warning_objects");
        assert_eq!(
            GraphNodeKind::from_block_kind(&block.kind),
            GraphNodeKind::ObjectCatalog
        );
        assert_eq!(
            cas_kind_for_block(&block),
            (OBJECT_CATALOG, "mei-object-catalog-v1")
        );
    }

    #[test]
    fn author_intent_catalog_is_recognized_with_internal_index_layer() {
        let source_anchor = "domain/alerts.objects.mei";
        let source = json!({
            "role": "source",
            "kind": "dataset_ref",
            "id": "alerts",
            "source_anchor": source_anchor
        });
        let identity_ref = json!({
            "role": "identity",
            "kind": "field_ref",
            "id": "alert_id",
            "source_anchor": source_anchor
        });
        let recipe = json!({
            "role": "recipe",
            "kind": "stock_ref",
            "id": "alert",
            "source_anchor": source_anchor
        });
        let identity = json!({
            "materialization": "dataset_row",
            "fields": ["alert_id"],
            "locator": identity_ref,
            "aliases": [],
            "normalization": null
        });
        let block = GraphBlock {
            kind: "object_catalog".to_string(),
            block_id: "object_catalog:objects_stable".to_string(),
            schema: "mei-object-catalog-v1".to_string(),
            payload: json!({
                "schema_version": "mei-object-catalog-v1",
                "id": "objects_stable",
                "authoring_mode": "author_intent",
                "types": [{
                    "id": "ops.Alert",
                    "intent_id": "intent_stable",
                    "identity": identity,
                    "source": source,
                    "projections": [recipe],
                    "source_anchor": source_anchor
                }],
                "refs": [],
                "intents": [{
                    "intent_id": "intent_stable",
                    "object_type_id": "ops.Alert",
                    "source": source,
                    "identity": identity,
                    "recipe": recipe,
                    "owner_hints": [source, identity_ref, recipe],
                    "source_anchor": source_anchor
                }],
                "index": [{
                    "kind": "internal_object_index",
                    "key": "ops.Alert::dataset_ref:alerts::field_ref:alert_id",
                    "intent_id": "intent_stable",
                    "object_type_id": "ops.Alert",
                    "source": source,
                    "identity": identity_ref,
                    "recipe": recipe,
                    "owner_hints": [source, identity_ref, recipe],
                    "source_anchor": source_anchor
                }],
                "default_assemblies": [{
                    "kind": "default_object_assembly",
                    "id": "assembly_stable",
                    "intent_id": "intent_stable",
                    "recipe": recipe,
                    "recipe_contract": {
                        "schema_version": "mei-stock-object-recipe-v1",
                        "id": "cockpit.alert",
                        "slots": [],
                        "projections": [],
                        "interactions": [],
                        "responders": [],
                        "override_precedence": [
                            "local",
                            "domain",
                            "app",
                            "stock",
                            "placeholder",
                            "no_projection"
                        ],
                        "identity_locked": true,
                        "source_anchor": "stock/templates/cockpit/object-recipes.mei"
                    },
                    "source_anchor": source_anchor
                }],
                "source_anchor": source_anchor
            }),
        };

        let catalog = decode_object_catalog_block(&block)
            .expect("decode author intent")
            .expect("object catalog");
        assert_eq!(
            import_layer_for_object_catalog(Some(&catalog)),
            "import:object_author_intent:internal_index"
        );
        assert_eq!(catalog.index[0].kind, OBJECT_INDEX_ENTRY_KIND);
    }
}
