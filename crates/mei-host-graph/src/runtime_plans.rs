//! `runtime.plans` artifact: layer_plan / world_plan / map_projection for client compositor.

use std::path::Path;

use anyhow::Result;
use mei_lang_kernel::{load_mei_config_for_app, resolve_app_root, LayoutBudgetManifest};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use serde_json::json;

use crate::assemble::{assemble_scope_from_registry, AssembleOutcome};
use crate::content_store::put_if_absent;
use crate::layer_store::{store_layer, take_layer};
use crate::semantic_cache::SemanticCacheCore;
use crate::theme_layout_merge::theme_layout_client_patches;
use crate::types::PayloadRef;

pub const RUNTIME_PLANS_KIND: &str = "runtime_plans";
/// Bump when map_projection/world_plan exchange shape changes (e.g. L4 `heroes`).
pub const RUNTIME_PLANS_SCHEMA: &str = "runtime-plans-v3";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimePlansDocument {
    pub schema_version: String,
    pub app_id: String,
    pub scene_id: String,
    pub layer_plan: Value,
    pub world_plan: Value,
    pub map_projection: Value,
    pub overlay_defaults: Value,
    #[serde(default)]
    pub presentation_map: Value,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub component_assets: Vec<mei_lang_kernel::ComponentAsset>,
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub theme_layout: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout_budget_manifest: Option<LayoutBudgetManifest>,
}

pub fn runtime_plans_cache_key(
    semantic_core: &SemanticCacheCore,
    layout_policy_revision: &str,
) -> String {
    json!({
        "artifact": RUNTIME_PLANS_KIND,
        "semantic_core": semantic_core,
        "layout_policy_revision": layout_policy_revision,
        "schema_version": RUNTIME_PLANS_SCHEMA,
        "presentation_map_schema": mei_lang_kernel::PRESENTATION_MAP_SCHEMA_VERSION,
        // L4 map_hero → map_projection.heroes; keep in key so authoring changes invalidate.
        "map_heroes_contract": "v2",
    })
    .to_string()
}

pub fn runtime_plans_from_outcome(
    outcome: &AssembleOutcome,
    workspace_root: &Path,
) -> RuntimePlansDocument {
    let overlay_defaults = Value::Object(
        outcome
            .overlay_defaults
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect(),
    );
    let app_root = resolve_app_root(workspace_root, outcome.compiled.app_id.as_str());
    let mei_config = load_mei_config_for_app(app_root.as_path(), Some(workspace_root));
    let theme_id = outcome
        .compiled
        .scene_contract
        .as_ref()
        .and_then(|contract| contract.scene.theme.clone())
        .unwrap_or_else(|| "cockpit".to_string());
    let theme_layout = theme_layout_client_patches(&mei_config.ops.themes, theme_id.as_str());
    let layout_budget_manifest = Some(
        outcome
            .compiled
            .ui_layout_index
            .layout_budget_manifest(outcome.compile_revision.as_str()),
    );
    RuntimePlansDocument {
        schema_version: RUNTIME_PLANS_SCHEMA.to_string(),
        app_id: outcome.compiled.app_id.clone(),
        scene_id: outcome
            .compiled
            .active_scene
            .clone()
            .unwrap_or_else(|| "home".to_string()),
        layer_plan: outcome.layer_plan.clone(),
        world_plan: outcome.world_plan.clone(),
        map_projection: outcome.map_projection.clone(),
        overlay_defaults,
        presentation_map: outcome.presentation_map.clone(),
        component_assets: outcome.compiled.component_assets.clone(),
        theme_layout,
        layout_budget_manifest,
    }
}

pub fn empty_runtime_plans_document(app_id: &str, scene_id: &str) -> RuntimePlansDocument {
    RuntimePlansDocument {
        schema_version: RUNTIME_PLANS_SCHEMA.to_string(),
        app_id: app_id.to_string(),
        scene_id: scene_id.to_string(),
        layer_plan: Value::Object(Default::default()),
        world_plan: Value::Object(Default::default()),
        map_projection: Value::Object(Default::default()),
        overlay_defaults: Value::Object(Default::default()),
        presentation_map: Value::Object(Default::default()),
        component_assets: Vec::new(),
        theme_layout: Value::Object(Default::default()),
        layout_budget_manifest: None,
    }
}

pub fn build_runtime_plans_document(
    workspace_root: &Path,
    app_id: &str,
    scene_id: &str,
    _layout_policy_revision: &str,
) -> Result<RuntimePlansDocument> {
    let Some(outcome) = assemble_scope_from_registry(workspace_root, app_id, scene_id)? else {
        return Ok(empty_runtime_plans_document(app_id, scene_id));
    };
    Ok(runtime_plans_from_outcome(&outcome, workspace_root))
}

pub fn persist_runtime_plans(
    app_root: &Path,
    document: &RuntimePlansDocument,
) -> Result<PayloadRef> {
    let bytes = serde_json::to_vec(document)?;
    let put = put_if_absent(app_root, RUNTIME_PLANS_KIND, &bytes)?;
    Ok(PayloadRef::new(
        RUNTIME_PLANS_KIND,
        put.content_hash,
        RUNTIME_PLANS_SCHEMA,
    ))
}

pub fn ensure_runtime_plans_cached(
    workspace_root: &Path,
    semantic_core: &SemanticCacheCore,
    layout_policy_revision: &str,
) -> Result<(RuntimePlansDocument, PayloadRef, bool)> {
    let cache_key = runtime_plans_cache_key(semantic_core, layout_policy_revision);
    if let Some(bytes) = take_layer(cache_key.as_str()) {
        if crate::schema_gate::layer_bytes_match_schema(bytes.as_slice(), RUNTIME_PLANS_SCHEMA) {
            let doc: RuntimePlansDocument = serde_json::from_slice(bytes.as_slice())?;
            if crate::schema_gate::document_schema_ok(
                doc.schema_version.as_str(),
                RUNTIME_PLANS_SCHEMA,
            ) && mei_lang_kernel::presentation_map_schema_ok(&doc.presentation_map)
            {
                let content_hash = crate::content_store::content_hash_bytes(bytes.as_slice());
                let pref = PayloadRef::new(
                    RUNTIME_PLANS_KIND,
                    content_hash.as_str(),
                    RUNTIME_PLANS_SCHEMA,
                );
                return Ok((doc, pref, true));
            }
        }
    }
    let document = build_runtime_plans_document(
        workspace_root,
        semantic_core.app_id.as_str(),
        semantic_core.scene_id.as_str(),
        layout_policy_revision,
    )?;
    let app_root = mei_lang_kernel::resolve_app_root(workspace_root, semantic_core.app_id.as_str());
    let pref = persist_runtime_plans(app_root.as_path(), &document)?;
    let bytes = serde_json::to_vec(&document)?;
    store_layer(
        cache_key,
        RUNTIME_PLANS_KIND,
        pref.content_hash.as_str(),
        bytes.as_slice(),
    );
    Ok((document, pref, false))
}
