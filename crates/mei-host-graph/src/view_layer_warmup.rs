//! AOT warmup: materialize manifest index and layer artifacts into CAS + memory.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;
use mei_lang_kernel::{resolve_app_root, DataMode};
use serde_json::{json, Value};

use crate::eval_slot_group::collect_slot_groups;
use crate::manifest_index::{
    manifest_index_cache_key, persist_manifest_index, store_manifest_index_memory,
    take_manifest_index, ManifestIndexDocument, SurfaceManifestSlice, MANIFEST_INDEX_SCHEMA,
};
use crate::view_artifact::{
    build_semantic_core_for_scene, layer_ref_from_manifest_entry, manifest_revision_digest,
    surface_revision_digest_from_manifest, ComposeRequest, LayerRef, SceneViewManifest,
    StructureFullDocument, SCENE_VIEW_MANIFEST_SCHEMA,
};

fn layout_policy_revision(workspace_root: &Path, app_id: &str) -> String {
    let app_root = resolve_app_root(workspace_root, app_id);
    mei_lang_kernel::load_cache_generation(app_root.as_path(), app_id).data_generation
}

fn layer_ref_from_materialized(value: &Value) -> Option<LayerRef> {
    layer_ref_from_manifest_entry("layer", value)
}

fn semantic_layers_from_refs(refs: &BTreeMap<String, LayerRef>) -> BTreeMap<String, Value> {
    refs.iter()
        .map(|(name, layer_ref)| {
            (
                name.clone(),
                serde_json::to_value(layer_ref).unwrap_or(Value::Null),
            )
        })
        .collect()
}

/// Materialize view layer artifacts and manifest index (placeholder shells).
pub fn warm_manifest_index_for_scope(
    workspace_root: &Path,
    app_id: &str,
    scene_id: &str,
    data_mode: DataMode,
) -> Result<ManifestIndexDocument> {
    let layout_rev = layout_policy_revision(workspace_root, app_id);
    let semantic_core = build_semantic_core_for_scene(workspace_root, app_id, scene_id);
    let cache_key = manifest_index_cache_key(&semantic_core, layout_rev.as_str(), data_mode.slug());
    if let Some(index) = take_manifest_index(cache_key.as_str()) {
        return Ok(index);
    }

    let Some(outcome) = crate::assemble_scope_from_registry(workspace_root, app_id, scene_id)?
    else {
        anyhow::bail!("assemble unavailable for {app_id}/{scene_id}");
    };
    let compiled = &outcome.compiled;

    let structure_key = crate::structure_full_cache_key(&semantic_core, layout_rev.as_str());
    let (_doc, structure_pref, _) = crate::structure_full_from_compiled(
        workspace_root,
        compiled,
        &semantic_core,
        layout_rev.as_str(),
    )?;
    let structure_bytes = serde_json::to_vec(&_doc)?;
    crate::layer_store::store_layer(
        structure_key.clone(),
        crate::view_artifact::STRUCTURE_FULL_KIND,
        structure_pref.content_hash.as_str(),
        structure_bytes.as_slice(),
    );
    let structure_ref = LayerRef {
        artifact_id: structure_key,
        content_hash: structure_pref.content_hash,
        bytes: Some(structure_bytes.len() as u64),
        encoding: Some("json".to_string()),
    };

    let (theme_doc, _) = crate::layer_overlay::ensure_theme_tokens_cached(
        workspace_root,
        app_id,
        layout_rev.as_str(),
    )?;
    let theme_key = crate::view_artifact::theme_tokens_cache_key(layout_rev.as_str());
    let theme_doc_value = json!({
        "artifact_id": theme_key,
        "content_hash": format!("theme:{}", layout_rev),
        "document": theme_doc,
    });

    let (overlay_doc, _) = crate::layer_overlay::ensure_layout_overlay_cached(
        workspace_root,
        app_id,
        layout_rev.as_str(),
        None,
        None,
        None,
    )?;
    let overlay_key = crate::view_artifact::layout_overlay_persisted_cache_key(layout_rev.as_str());
    let overlay_doc_value = json!({
        "artifact_id": overlay_key,
        "content_hash": format!("overlay:persisted:{}", layout_rev),
        "document": overlay_doc,
    });

    let runtime_doc = crate::runtime_plans::runtime_plans_from_outcome(&outcome, workspace_root);
    let app_root = resolve_app_root(workspace_root, app_id);
    let runtime_pref =
        crate::runtime_plans::persist_runtime_plans(app_root.as_path(), &runtime_doc)?;
    let runtime_key =
        crate::runtime_plans::runtime_plans_cache_key(&semantic_core, layout_rev.as_str());
    let runtime_bytes = serde_json::to_vec(&runtime_doc)?;
    crate::layer_store::store_layer(
        runtime_key.clone(),
        crate::runtime_plans::RUNTIME_PLANS_KIND,
        runtime_pref.content_hash.as_str(),
        runtime_bytes.as_slice(),
    );
    let runtime_doc_value = json!({
        "artifact_id": runtime_key,
        "content_hash": runtime_pref.content_hash,
        "document": runtime_doc,
    });

    let mut semantic_layer_refs = BTreeMap::new();
    semantic_layer_refs.insert("structure.full".to_string(), structure_ref);
    if let Some(layer_ref) = layer_ref_from_materialized(&theme_doc_value) {
        semantic_layer_refs.insert("theme.tokens".to_string(), layer_ref);
    }
    if let Some(layer_ref) = layer_ref_from_materialized(&overlay_doc_value) {
        semantic_layer_refs.insert("layout.overlay".to_string(), layer_ref);
    }
    if let Some(layer_ref) = layer_ref_from_materialized(&runtime_doc_value) {
        semantic_layer_refs.insert("runtime.plans".to_string(), layer_ref);
    }

    let structure_full: StructureFullDocument =
        crate::structure_full::build_structure_full_document(compiled, "warm");
    let eval_slot_group_ids = collect_slot_groups(&structure_full);
    for group_id in &eval_slot_group_ids {
        let (eval_doc, eval_pref, _) = crate::eval_slot_group::ensure_eval_slot_group_cached(
            workspace_root,
            compiled,
            &semantic_core,
            group_id.as_str(),
            data_mode,
            layout_rev.as_str(),
        )?;
        let eval_key = crate::view_artifact::eval_slot_group_cache_key(
            &semantic_core,
            group_id.as_str(),
            data_mode.slug(),
            "default",
        );
        let eval_doc_value = json!({
            "artifact_id": eval_key,
            "content_hash": eval_pref.content_hash,
            "document": eval_doc,
        });
        if let Some(layer_ref) = layer_ref_from_materialized(&eval_doc_value) {
            semantic_layer_refs.insert(format!("eval.slot_group.{group_id}"), layer_ref);
        }
    }

    let semantic_manifest = SceneViewManifest {
        schema_version: SCENE_VIEW_MANIFEST_SCHEMA.to_string(),
        app_id: app_id.to_string(),
        scene_id: scene_id.to_string(),
        semantic_core: semantic_core.clone(),
        revision_digest: String::new(),
        layers: semantic_layers_from_refs(&semantic_layer_refs),
        compose_defaults: None,
        surface_revision_digest: None,
    };
    let manifest_revision_digest = manifest_revision_digest(&semantic_manifest, None);

    let mut surfaces = Vec::new();
    let route_mode = "app";
    let tab = "scene";
    let shell_layer_name = format!("shell.{route_mode}");
    let layers = semantic_layers_from_refs(&semantic_layer_refs);
    let compose_defaults = ComposeRequest {
        route_mode: Some(route_mode.to_string()),
        tab: Some(tab.to_string()),
        chrome: Some("full".to_string()),
        review_projection: Some(default_review_projection(route_mode)),
        data_mode: Some(data_mode.slug().to_string()),
        focus: None,
        scope: None,
    };
    let surface_manifest = SceneViewManifest {
        schema_version: SCENE_VIEW_MANIFEST_SCHEMA.to_string(),
        app_id: app_id.to_string(),
        scene_id: scene_id.to_string(),
        semantic_core: semantic_core.clone(),
        revision_digest: manifest_revision_digest.clone(),
        layers,
        compose_defaults: Some(compose_defaults.clone()),
        surface_revision_digest: None,
    };
    let surface_revision_digest = surface_revision_digest_from_manifest(&surface_manifest);
    surfaces.push(SurfaceManifestSlice {
        route_mode: route_mode.to_string(),
        shell_layer_name,
        surface_revision_digest: surface_revision_digest.unwrap_or_default(),
        compose_defaults,
        shell_layer_ref: BTreeMap::new(),
    });

    let index = ManifestIndexDocument {
        schema_version: MANIFEST_INDEX_SCHEMA.to_string(),
        app_id: app_id.to_string(),
        scene_id: scene_id.to_string(),
        data_mode: data_mode.slug().to_string(),
        layout_policy_revision: layout_rev,
        semantic_core,
        manifest_revision_digest,
        semantic_layer_refs,
        eval_slot_group_ids,
        surfaces,
    };
    let _pref = persist_manifest_index(app_root.as_path(), &index)?;
    store_manifest_index_memory(cache_key.as_str(), &index);
    Ok(index)
}

fn default_review_projection(_route_mode: &str) -> String {
    "live_full".to_string()
}

pub fn warm_manifest_index_for_app(
    workspace_root: &Path,
    app_id: &str,
    scene_id: &str,
) -> Result<()> {
    let _ = warm_manifest_index_for_scope(workspace_root, app_id, scene_id, DataMode::Eval)?;
    Ok(())
}
