//! Precomputed manifest index: layer refs + digests per surface (AOT / warmup artifact).

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::content_store::{put_if_absent, read_payload_json};
use crate::layer_store::{store_layer, take_layer};
use crate::semantic_cache::SemanticCacheCore;
use crate::types::PayloadRef;
use crate::view_artifact::{
    ComposeRequest, LayerRef, SceneViewManifest, SCENE_VIEW_MANIFEST_SCHEMA,
};

pub const MANIFEST_INDEX_KIND: &str = "manifest_index";
pub const MANIFEST_INDEX_SCHEMA: &str = "manifest-index-v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SurfaceManifestSlice {
    pub route_mode: String,
    pub shell_layer_name: String,
    pub surface_revision_digest: String,
    pub compose_defaults: ComposeRequest,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub shell_layer_ref: BTreeMap<String, LayerRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManifestIndexDocument {
    pub schema_version: String,
    pub app_id: String,
    pub scene_id: String,
    pub data_mode: String,
    pub layout_policy_revision: String,
    pub semantic_core: SemanticCacheCore,
    pub manifest_revision_digest: String,
    #[serde(default)]
    pub semantic_layer_refs: BTreeMap<String, LayerRef>,
    #[serde(default)]
    pub eval_slot_group_ids: Vec<String>,
    #[serde(default)]
    pub surfaces: Vec<SurfaceManifestSlice>,
}

fn memory_index_store() -> &'static Mutex<BTreeMap<String, ManifestIndexDocument>> {
    static CACHE: OnceLock<Mutex<BTreeMap<String, ManifestIndexDocument>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(BTreeMap::new()))
}

pub fn manifest_index_cache_key(
    semantic_core: &SemanticCacheCore,
    layout_policy_revision: &str,
    data_mode: &str,
) -> String {
    json!({
        "artifact": MANIFEST_INDEX_KIND,
        "semantic_core": semantic_core,
        "layout_policy_revision": layout_policy_revision,
        "data_mode": data_mode,
        "schema_version": MANIFEST_INDEX_SCHEMA,
    })
    .to_string()
}

pub fn manifest_index_cache_key_partitioned(
    partition: &mei_host_core::CachePartitionKey,
    semantic_core: &SemanticCacheCore,
    layout_policy_revision: &str,
    data_mode: &str,
) -> String {
    partition.prefix_key(&manifest_index_cache_key(
        semantic_core,
        layout_policy_revision,
        data_mode,
    ))
}

pub fn persist_manifest_index(
    app_root: &Path,
    document: &ManifestIndexDocument,
) -> Result<PayloadRef> {
    let bytes = serde_json::to_vec(document)?;
    let put = put_if_absent(app_root, MANIFEST_INDEX_KIND, &bytes)?;
    Ok(PayloadRef::new(
        MANIFEST_INDEX_KIND,
        put.content_hash,
        MANIFEST_INDEX_SCHEMA,
    ))
}

pub fn store_manifest_index_memory(cache_key: &str, document: &ManifestIndexDocument) {
    if let Ok(bytes) = serde_json::to_vec(document) {
        store_layer(
            cache_key.to_string(),
            MANIFEST_INDEX_KIND,
            crate::content_store::content_hash_bytes(bytes.as_slice()).as_str(),
            bytes.as_slice(),
        );
    }
    if let Ok(mut cache) = memory_index_store().lock() {
        cache.insert(cache_key.to_string(), document.clone());
    }
}

pub fn take_manifest_index(cache_key: &str) -> Option<ManifestIndexDocument> {
    if let Ok(cache) = memory_index_store().lock() {
        if let Some(doc) = cache.get(cache_key) {
            if crate::schema_gate::document_schema_ok(
                doc.schema_version.as_str(),
                MANIFEST_INDEX_SCHEMA,
            ) {
                return Some(doc.clone());
            }
        }
    }
    let bytes = take_layer(cache_key)?;
    if !crate::schema_gate::layer_bytes_match_schema(bytes.as_slice(), MANIFEST_INDEX_SCHEMA) {
        return None;
    }
    let doc: ManifestIndexDocument = serde_json::from_slice(bytes.as_slice()).ok()?;
    if !crate::schema_gate::document_schema_ok(doc.schema_version.as_str(), MANIFEST_INDEX_SCHEMA) {
        return None;
    }
    Some(doc)
}

pub fn load_manifest_index_from_content_store(
    app_root: &Path,
    content_hash: &str,
) -> Option<ManifestIndexDocument> {
    let pref = PayloadRef::new(
        MANIFEST_INDEX_KIND,
        content_hash.to_string(),
        MANIFEST_INDEX_SCHEMA,
    );
    read_payload_json(app_root, &pref)
        .ok()
        .flatten()
        .and_then(|value| serde_json::from_value(value).ok())
}

pub fn manifest_index_to_scene_manifest(
    index: &ManifestIndexDocument,
    route_mode: &str,
) -> Option<SceneViewManifest> {
    let surface = index
        .surfaces
        .iter()
        .find(|entry| entry.route_mode == route_mode)?;
    let mut layers = BTreeMap::new();
    for (name, layer_ref) in &index.semantic_layer_refs {
        layers.insert(
            name.clone(),
            json!({
                "artifact_id": layer_ref.artifact_id,
                "content_hash": layer_ref.content_hash,
                "bytes": layer_ref.bytes,
                "encoding": layer_ref.encoding,
            }),
        );
    }
    if let Some(shell_ref) = surface.shell_layer_ref.values().next() {
        layers.insert(
            surface.shell_layer_name.clone(),
            json!({
                "artifact_id": shell_ref.artifact_id,
                "content_hash": shell_ref.content_hash,
                "bytes": shell_ref.bytes,
                "encoding": shell_ref.encoding,
            }),
        );
    }
    Some(SceneViewManifest {
        schema_version: SCENE_VIEW_MANIFEST_SCHEMA.to_string(),
        app_id: index.app_id.clone(),
        scene_id: index.scene_id.clone(),
        semantic_core: index.semantic_core.clone(),
        revision_digest: index.manifest_revision_digest.clone(),
        layers,
        compose_defaults: Some(surface.compose_defaults.clone()),
        surface_revision_digest: Some(surface.surface_revision_digest.clone()),
    })
}

pub fn clear_manifest_index_for_app(app_id: &str) {
    if let Ok(mut cache) = memory_index_store().lock() {
        let part_prefix = format!("part:{app_id}/");
        cache.retain(|key, doc| doc.app_id != app_id && !key.starts_with(part_prefix.as_str()));
    }
    crate::layer_store::clear_layers_for_app(app_id);
}

pub fn clear_manifest_index_for_partition(partition: &mei_host_core::CachePartitionKey) -> usize {
    let removed = if let Ok(mut cache) = memory_index_store().lock() {
        let before = cache.len();
        cache.retain(|key, _| !partition.matches_key(key));
        before.saturating_sub(cache.len())
    } else {
        0
    };
    crate::layer_store::clear_layers_for_partition(partition);
    removed
}

#[cfg(test)]
mod partition_tests {
    use super::*;
    use crate::semantic_cache::build_semantic_cache_core;
    use mei_host_core::CachePartitionKey;

    #[test]
    fn dual_partition_manifest_keys_do_not_collide() {
        let core = build_semantic_cache_core(
            "mini-data",
            "home",
            None,
            "reg-1",
            "client-1",
            "data-1",
            "epoch-1",
        );
        let a = CachePartitionKey::new("mini-data", "WS-1", "cfg-a");
        let b = CachePartitionKey::new("mini-data", "WS-1", "cfg-b");
        let key_a = manifest_index_cache_key_partitioned(&a, &core, "layout-1", "scoped");
        let key_b = manifest_index_cache_key_partitioned(&b, &core, "layout-1", "scoped");
        assert_ne!(key_a, key_b);
        assert!(a.matches_key(key_a.as_str()));
        assert!(!b.matches_key(key_a.as_str()));
    }
}
