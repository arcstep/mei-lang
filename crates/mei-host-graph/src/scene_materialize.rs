//! Shared scene-view manifest / layer-batch materialization (Host + App Runtime).
//!
//! Host-specific chrome HTML is injected via [`ShellChromeRenderer`]; without a
//! renderer, shell layers fall back to the placeholder stub (same as prior Host
//! behavior when `chrome_host` was `None`).

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;
use mei_lang_kernel::{resolve_app_root, CompiledApp, DataMode, ReviewProjection};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::assemble::AssembleOutcome;
use crate::eval_slot_group::collect_slot_groups;
use crate::manifest_index::{
    manifest_index_cache_key, manifest_index_to_scene_manifest, persist_manifest_index,
    store_manifest_index_memory, take_manifest_index, ManifestIndexDocument, SurfaceManifestSlice,
    MANIFEST_INDEX_SCHEMA,
};
use crate::semantic_cache::SemanticCacheCore;
use crate::shell_layer::{
    build_shell_layer_document, ensure_shell_layer_rendered, is_placeholder_shell_document,
    store_shell_layer_document, ShellLayerDocument, SHELL_LAYER_SCHEMA,
};
use crate::view_artifact::{
    build_semantic_core_for_scene, eval_slot_group_cache_key, layer_ref_from_manifest_entry,
    layout_overlay_persisted_cache_key, layout_overlay_session_cache_key, manifest_revision_digest,
    resolve_view_revision, shell_cache_key, structure_full_cache_key,
    surface_revision_digest_from_manifest, theme_tokens_cache_key, ComposeRequest, LayerRef,
    SceneViewManifest, StructureFullDocument, ViewRevisionInput, ViewRevisionResponse,
    SCENE_VIEW_MANIFEST_SCHEMA, STRUCTURE_FULL_KIND,
};

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArtifactHitMatrix {
    #[serde(default)]
    pub structure_hit: bool,
    #[serde(default)]
    pub eval_hit: bool,
    #[serde(default)]
    pub theme_hit: bool,
    #[serde(default)]
    pub overlay_hit: bool,
    #[serde(default)]
    pub shell_hit: bool,
}

impl ArtifactHitMatrix {
    pub fn summary_tag(&self) -> String {
        format!(
            "structure={} eval={} theme={} overlay={} shell={}",
            hit(self.structure_hit),
            hit(self.eval_hit),
            hit(self.theme_hit),
            hit(self.overlay_hit),
            hit(self.shell_hit),
        )
    }
}

fn hit(value: bool) -> &'static str {
    if value {
        "hit"
    } else {
        "miss"
    }
}

/// Inputs for optional Host/Runtime chrome HTML rendering.
pub struct ShellChromeRenderArgs<'a> {
    pub workspace_root: &'a Path,
    pub app_id: &'a str,
    pub scene_id: &'a str,
    pub route_mode: &'a str,
    pub tab: &'a str,
    pub chrome: &'a str,
    pub data_mode: DataMode,
    pub compiled: Option<&'a CompiledApp>,
    pub auth_sig: Option<u64>,
}

/// When provided, may return a real `ShellLayerDocument` (non-placeholder topbar).
pub type ShellChromeRenderer<'a> =
    dyn Fn(ShellChromeRenderArgs<'_>) -> Option<ShellLayerDocument> + 'a;

struct MaterializeContext<'a> {
    workspace_root: &'a Path,
    app_id: &'a str,
    scene_id: &'a str,
    data_mode: DataMode,
    route_mode: &'a str,
    tab: &'a str,
    chrome: &'a str,
    draft_session: &'a str,
    draft_digest: &'a str,
    draft: Option<Value>,
    layout_rev: String,
    theme_digest: String,
    semantic_core: SemanticCacheCore,
    compiled: Option<CompiledApp>,
    assemble_outcome: Option<AssembleOutcome>,
}

fn layout_policy_revision(workspace_root: &Path, app_id: &str) -> String {
    let app_root = resolve_app_root(workspace_root, app_id);
    let mei_config = mei_lang_kernel::load_mei_config_for_app(app_root.as_path(), Some(workspace_root));
    let data_gen =
        mei_lang_kernel::load_cache_generation(app_root.as_path(), app_id).data_generation;
    // Phase 6: include cockpit profile policy digest (ops + profile), not only data_generation.
    let policy = mei_lang_kernel::profile_layout_policy_digest(
        mei_lang_kernel::StageProfile::Cockpit,
        mei_config.ops.strict_fill_down,
        mei_config.ops.fill_down,
    );
    format!("{data_gen}|{policy}")
}

fn theme_digest_for_app(workspace_root: &Path, app_id: &str) -> String {
    layout_policy_revision(workspace_root, app_id)
}

fn default_tab_for_route(route_mode: &str) -> &'static str {
    match route_mode {
        "layout" | "prototype" | "runtime" => "preview",
        _ => "scene",
    }
}

fn default_ssr_review_projection(data_mode: DataMode) -> &'static str {
    match data_mode {
        DataMode::Static => ReviewProjection::StaticFull.slug(),
        _ => ReviewProjection::LiveFull.slug(),
    }
}

fn load_materialize_context<'a>(
    workspace_root: &'a Path,
    app_id: &'a str,
    scene_id: &'a str,
    data_mode: DataMode,
    route_mode: &'a str,
    tab: &'a str,
    chrome: &'a str,
    draft_session: &'a str,
    draft_digest: &'a str,
    draft: Option<Value>,
) -> Result<MaterializeContext<'a>> {
    let layout_rev = layout_policy_revision(workspace_root, app_id);
    let semantic_core = build_semantic_core_for_scene(workspace_root, app_id, scene_id);
    Ok(MaterializeContext {
        workspace_root,
        app_id,
        scene_id,
        data_mode,
        route_mode,
        tab,
        chrome,
        draft_session,
        draft_digest,
        draft,
        layout_rev,
        theme_digest: theme_digest_for_app(workspace_root, app_id),
        semantic_core,
        compiled: None,
        assemble_outcome: None,
    })
}

fn ensure_materialize_assembled(ctx: &mut MaterializeContext<'_>) -> Result<()> {
    if ctx.assemble_outcome.is_some() {
        return Ok(());
    }
    let assemble_outcome =
        crate::assemble_scope_from_registry(ctx.workspace_root, ctx.app_id, ctx.scene_id)?;
    ctx.compiled = assemble_outcome
        .as_ref()
        .map(|outcome| outcome.compiled.clone());
    ctx.assemble_outcome = assemble_outcome;
    Ok(())
}

fn materialize_structure(
    ctx: &mut MaterializeContext<'_>,
    hits: &mut ArtifactHitMatrix,
) -> Result<LayerRef> {
    let structure_key = structure_full_cache_key(&ctx.semantic_core, ctx.layout_rev.as_str());
    if let Some(bytes) = crate::take_layer(structure_key.as_str()) {
        hits.structure_hit = true;
        let hash = crate::content_hash_bytes(bytes.as_slice());
        return Ok(LayerRef {
            artifact_id: structure_key,
            content_hash: hash,
            bytes: Some(bytes.len() as u64),
            encoding: Some("json".to_string()),
        });
    }
    ensure_materialize_assembled(ctx)?;
    let compiled = ctx
        .compiled
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("assemble unavailable"))?;
    let (_doc, pref, _key) = crate::structure_full_from_compiled(
        ctx.workspace_root,
        compiled,
        &ctx.semantic_core,
        ctx.layout_rev.as_str(),
    )?;
    let document = crate::build_structure_full_document(compiled, structure_key.as_str());
    let bytes = serde_json::to_vec(&document)?;
    crate::store_layer(
        structure_key.clone(),
        STRUCTURE_FULL_KIND,
        pref.content_hash.as_str(),
        bytes.as_slice(),
    );
    hits.structure_hit = false;
    Ok(LayerRef {
        artifact_id: structure_key,
        content_hash: pref.content_hash,
        bytes: Some(bytes.len() as u64),
        encoding: Some("json".to_string()),
    })
}

fn materialize_structure_document(ctx: &mut MaterializeContext<'_>) -> Option<Value> {
    let structure_key = structure_full_cache_key(&ctx.semantic_core, ctx.layout_rev.as_str());
    if let Some(bytes) = crate::take_layer(structure_key.as_str()) {
        return serde_json::from_slice(bytes.as_slice()).ok();
    }
    ensure_materialize_assembled(ctx).ok()?;
    let compiled = ctx.compiled.as_ref()?;
    let document = crate::build_structure_full_document(compiled, structure_key.as_str());
    Some(serde_json::to_value(document).unwrap_or(Value::Null))
}

fn eval_slot_group_layer_ref(
    ctx: &mut MaterializeContext<'_>,
    slot_group_id: &str,
    hits: &mut ArtifactHitMatrix,
) -> Result<LayerRef> {
    let eval_key = eval_slot_group_cache_key(
        &ctx.semantic_core,
        slot_group_id,
        ctx.data_mode.slug(),
        "default",
    );
    if crate::take_layer(eval_key.as_str()).is_some() {
        hits.eval_hit = true;
    }
    ensure_materialize_assembled(ctx)?;
    let compiled = ctx
        .compiled
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("assemble unavailable"))?;
    let (_doc, pref, _cached) = crate::ensure_eval_slot_group_cached(
        ctx.workspace_root,
        compiled,
        &ctx.semantic_core,
        slot_group_id,
        ctx.data_mode,
        ctx.layout_rev.as_str(),
    )?;
    hits.eval_hit = true;
    Ok(LayerRef {
        artifact_id: eval_key,
        content_hash: pref.content_hash,
        bytes: None,
        encoding: Some("json".to_string()),
    })
}

fn materialize_eval_group(
    ctx: &mut MaterializeContext<'_>,
    slot_group_id: &str,
    hits: &mut ArtifactHitMatrix,
) -> Result<Value> {
    let eval_key = eval_slot_group_cache_key(
        &ctx.semantic_core,
        slot_group_id,
        ctx.data_mode.slug(),
        "default",
    );
    if crate::take_layer(eval_key.as_str()).is_some() {
        hits.eval_hit = true;
    }
    ensure_materialize_assembled(ctx)?;
    let compiled = ctx
        .compiled
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("assemble unavailable"))?;
    let (doc, pref, _cached) = crate::ensure_eval_slot_group_cached(
        ctx.workspace_root,
        compiled,
        &ctx.semantic_core,
        slot_group_id,
        ctx.data_mode,
        ctx.layout_rev.as_str(),
    )?;
    hits.eval_hit = true;
    Ok(json!({
        "artifact_id": eval_key,
        "content_hash": pref.content_hash,
        "document": doc,
    }))
}

fn materialize_runtime_plans(
    ctx: &mut MaterializeContext<'_>,
    _hits: &mut ArtifactHitMatrix,
) -> Result<Value> {
    let cache_key = crate::runtime_plans_cache_key(&ctx.semantic_core, ctx.layout_rev.as_str());
    if let Some(bytes) = crate::take_layer(cache_key.as_str()) {
        let doc: crate::RuntimePlansDocument = serde_json::from_slice(bytes.as_slice())?;
        let content_hash = crate::content_hash_bytes(bytes.as_slice());
        return Ok(json!({
            "artifact_id": cache_key,
            "content_hash": content_hash,
            "document": doc,
        }));
    }
    let document = if let Some(outcome) = ctx.assemble_outcome.as_ref() {
        crate::runtime_plans_from_outcome(outcome, ctx.workspace_root)
    } else {
        ensure_materialize_assembled(ctx)?;
        if let Some(outcome) = ctx.assemble_outcome.as_ref() {
            crate::runtime_plans_from_outcome(outcome, ctx.workspace_root)
        } else {
            crate::empty_runtime_plans_document(ctx.app_id, ctx.scene_id)
        }
    };
    let app_root = resolve_app_root(ctx.workspace_root, ctx.app_id);
    let pref = crate::persist_runtime_plans(app_root.as_path(), &document)?;
    let bytes = serde_json::to_vec(&document)?;
    crate::store_layer(
        cache_key.clone(),
        crate::RUNTIME_PLANS_KIND,
        pref.content_hash.as_str(),
        bytes.as_slice(),
    );
    Ok(json!({
        "artifact_id": cache_key,
        "content_hash": pref.content_hash,
        "document": document,
    }))
}

fn materialize_theme(ctx: &MaterializeContext<'_>, hits: &mut ArtifactHitMatrix) -> Result<Value> {
    let (doc, hit) = crate::ensure_theme_tokens_cached(
        ctx.workspace_root,
        ctx.app_id,
        ctx.theme_digest.as_str(),
    )?;
    hits.theme_hit = hit;
    let key = theme_tokens_cache_key(ctx.theme_digest.as_str());
    let content_hash = format!("theme:{}", ctx.theme_digest);
    Ok(json!({
        "artifact_id": key,
        "content_hash": content_hash,
        "document": doc,
    }))
}

fn materialize_overlay(
    ctx: &MaterializeContext<'_>,
    hits: &mut ArtifactHitMatrix,
) -> Result<Value> {
    let draft_ref = ctx.draft.as_ref();
    let (doc, hit) = crate::ensure_layout_overlay_cached(
        ctx.workspace_root,
        ctx.app_id,
        ctx.layout_rev.as_str(),
        if ctx.draft_digest.is_empty() {
            None
        } else {
            Some(ctx.draft_session)
        },
        if ctx.draft_digest.is_empty() {
            None
        } else {
            Some(ctx.draft_digest)
        },
        draft_ref,
    )?;
    hits.overlay_hit = hit;
    let persisted = layout_overlay_persisted_cache_key(ctx.layout_rev.as_str());
    let session_key = if ctx.draft_digest.is_empty() {
        None
    } else {
        Some(layout_overlay_session_cache_key(
            ctx.app_id,
            ctx.draft_session,
            ctx.draft_digest,
        ))
    };
    let artifact_id = session_key.clone().unwrap_or_else(|| persisted.clone());
    let content_hash = if ctx.draft_digest.is_empty() {
        format!("overlay:persisted:{}", ctx.layout_rev)
    } else {
        format!("overlay:session:{}", ctx.draft_digest)
    };
    Ok(json!({
        "artifact_id": artifact_id,
        "content_hash": content_hash,
        "persisted": persisted,
        "session": session_key.map(|value| json!(value)).unwrap_or(Value::Null),
        "document": doc,
    }))
}

fn materialize_shell(
    ctx: &mut MaterializeContext<'_>,
    hits: &mut ArtifactHitMatrix,
    shell_chrome: Option<&ShellChromeRenderer<'_>>,
    auth_sig: Option<u64>,
) -> Value {
    if shell_chrome.is_some() {
        let _ = ensure_materialize_assembled(ctx);
    }
    if let Some(render) = shell_chrome {
        let args = ShellChromeRenderArgs {
            workspace_root: ctx.workspace_root,
            app_id: ctx.app_id,
            scene_id: ctx.scene_id,
            route_mode: ctx.route_mode,
            tab: ctx.tab,
            chrome: ctx.chrome,
            data_mode: ctx.data_mode,
            compiled: ctx.compiled.as_ref(),
            auth_sig,
        };
        if let Some(doc) = render(args) {
            store_shell_layer_document(
                ctx.app_id,
                ctx.scene_id,
                ctx.route_mode,
                ctx.tab,
                ctx.chrome,
                auth_sig,
                &doc,
            );
            hits.shell_hit = false;
            let key = shell_cache_key(
                ctx.app_id,
                ctx.scene_id,
                ctx.route_mode,
                ctx.tab,
                ctx.chrome,
                auth_sig,
                SHELL_LAYER_SCHEMA,
            );
            let content_hash =
                crate::content_hash_bytes(serde_json::to_vec(&doc).unwrap_or_default().as_slice());
            return json!({
                "artifact_id": key,
                "content_hash": content_hash,
                "document": doc,
            });
        }
    }
    let (doc, hit) = ensure_shell_layer_rendered(
        ctx.app_id,
        ctx.scene_id,
        ctx.route_mode,
        ctx.tab,
        ctx.chrome,
        auth_sig,
        || build_shell_layer_document(ctx.route_mode, ctx.tab, ctx.chrome),
    );
    hits.shell_hit = hit;
    let key = shell_cache_key(
        ctx.app_id,
        ctx.scene_id,
        ctx.route_mode,
        ctx.tab,
        ctx.chrome,
        auth_sig,
        SHELL_LAYER_SCHEMA,
    );
    let content_hash =
        crate::content_hash_bytes(serde_json::to_vec(&doc).unwrap_or_default().as_slice());
    json!({
        "artifact_id": key,
        "content_hash": content_hash,
        "document": doc,
    })
}

fn layer_ref_from_materialized(value: &Value) -> Option<LayerRef> {
    layer_ref_from_manifest_entry("layer", value)
}

fn manifest_index_needs_shell_rebuild(index: &ManifestIndexDocument) -> bool {
    if index.surfaces.is_empty() {
        return true;
    }
    for surface in &index.surfaces {
        if surface.shell_layer_ref.is_empty() {
            return true;
        }
        for (_, layer_ref) in &surface.shell_layer_ref {
            let Some(bytes) = crate::take_layer(layer_ref.artifact_id.as_str()) else {
                return true;
            };
            let Ok(doc) = serde_json::from_slice::<ShellLayerDocument>(bytes.as_slice()) else {
                return true;
            };
            if is_placeholder_shell_document(&doc) {
                return true;
            }
        }
    }
    false
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

/// Ensure (or rebuild) the manifest index for an app/scene/data_mode.
pub fn ensure_manifest_index(
    workspace_root: &Path,
    app_id: &str,
    scene_id: &str,
    data_mode: DataMode,
    hits: &mut ArtifactHitMatrix,
    shell_chrome: Option<&ShellChromeRenderer<'_>>,
) -> Result<ManifestIndexDocument> {
    let layout_rev = layout_policy_revision(workspace_root, app_id);
    let semantic_core = build_semantic_core_for_scene(workspace_root, app_id, scene_id);
    let cache_key = manifest_index_cache_key(&semantic_core, layout_rev.as_str(), data_mode.slug());
    if let Some(index) = take_manifest_index(cache_key.as_str()) {
        let rebuild = shell_chrome.is_some() && manifest_index_needs_shell_rebuild(&index);
        if !rebuild {
            return Ok(index);
        }
    }
    build_and_store_manifest_index(
        workspace_root,
        app_id,
        scene_id,
        data_mode,
        layout_rev,
        semantic_core,
        cache_key,
        hits,
        shell_chrome,
    )
}

fn build_and_store_manifest_index(
    workspace_root: &Path,
    app_id: &str,
    scene_id: &str,
    data_mode: DataMode,
    layout_rev: String,
    semantic_core: SemanticCacheCore,
    cache_key: String,
    hits: &mut ArtifactHitMatrix,
    shell_chrome: Option<&ShellChromeRenderer<'_>>,
) -> Result<ManifestIndexDocument> {
    let mut ctx = load_materialize_context(
        workspace_root,
        app_id,
        scene_id,
        data_mode,
        "app",
        "scene",
        "full",
        "",
        "",
        None,
    )?;
    ensure_materialize_assembled(&mut ctx)?;

    let structure_ref = materialize_structure(&mut ctx, hits)?;
    let theme_doc = materialize_theme(&ctx, hits)?;
    let overlay_doc = materialize_overlay(&ctx, hits)?;
    let runtime_plans_doc = materialize_runtime_plans(&mut ctx, hits)?;

    let mut semantic_layer_refs = BTreeMap::new();
    semantic_layer_refs.insert("structure.full".to_string(), structure_ref);
    if let Some(layer_ref) = layer_ref_from_materialized(&theme_doc) {
        semantic_layer_refs.insert("theme.tokens".to_string(), layer_ref);
    }
    if let Some(layer_ref) = layer_ref_from_materialized(&overlay_doc) {
        semantic_layer_refs.insert("layout.overlay".to_string(), layer_ref);
    }
    if let Some(layer_ref) = layer_ref_from_materialized(&runtime_plans_doc) {
        semantic_layer_refs.insert("runtime.plans".to_string(), layer_ref);
    }

    let mut eval_slot_group_ids = vec!["scene:default".to_string()];
    if let Some(doc_value) = materialize_structure_document(&mut ctx) {
        if let Ok(structure) = serde_json::from_value::<StructureFullDocument>(doc_value) {
            eval_slot_group_ids = collect_slot_groups(&structure);
            for group_id in &eval_slot_group_ids {
                let layer_ref = eval_slot_group_layer_ref(&mut ctx, group_id.as_str(), hits)?;
                semantic_layer_refs.insert(format!("eval.slot_group.{group_id}"), layer_ref);
            }
        }
    }

    let semantic_layers = semantic_layers_from_refs(&semantic_layer_refs);
    let semantic_manifest = SceneViewManifest {
        schema_version: SCENE_VIEW_MANIFEST_SCHEMA.to_string(),
        app_id: app_id.to_string(),
        scene_id: scene_id.to_string(),
        semantic_core: semantic_core.clone(),
        revision_digest: String::new(),
        layers: semantic_layers,
        compose_defaults: None,
        surface_revision_digest: None,
    };
    let manifest_revision_digest = manifest_revision_digest(&semantic_manifest, None);

    let mut surfaces = Vec::new();
    let route_slug = "app";
    let tab = "scene";
    let review_projection = default_ssr_review_projection(data_mode);
    ctx.route_mode = route_slug;
    ctx.tab = tab;
    ctx.chrome = "full";
    let compose_defaults = ComposeRequest {
        route_mode: Some(route_slug.to_string()),
        tab: Some(tab.to_string()),
        chrome: Some("full".to_string()),
        review_projection: Some(review_projection.to_string()),
        data_mode: Some(data_mode.slug().to_string()),
        focus: None,
        scope: None,
    };
    let shell_doc = materialize_shell(&mut ctx, hits, shell_chrome, None);
    let shell_layer_name = format!("shell.{route_slug}");
    let mut shell_layer_ref = BTreeMap::new();
    if let Some(layer_ref) = layer_ref_from_materialized(&shell_doc) {
        shell_layer_ref.insert(shell_layer_name.clone(), layer_ref);
    }
    let mut layers = semantic_layers_from_refs(&semantic_layer_refs);
    if let Some(shell_ref) = layer_ref_from_materialized(&shell_doc) {
        layers.insert(
            shell_layer_name.clone(),
            serde_json::to_value(shell_ref).unwrap_or(Value::Null),
        );
    }
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
        route_mode: route_slug.to_string(),
        shell_layer_name,
        surface_revision_digest: surface_revision_digest.unwrap_or_default(),
        compose_defaults,
        shell_layer_ref,
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
    let app_root = resolve_app_root(workspace_root, app_id);
    let _pref = persist_manifest_index(app_root.as_path(), &index)?;
    store_manifest_index_memory(cache_key.as_str(), &index);
    Ok(index)
}

/// Project a manifest index surface slice into a [`SceneViewManifest`].
pub fn manifest_for_surface(
    index: &ManifestIndexDocument,
    route_mode: &str,
) -> Option<SceneViewManifest> {
    manifest_index_to_scene_manifest(index, route_mode)
}

/// Build a full scene-view manifest (semantic layers + shell for the surface).
pub fn build_scene_view_manifest(
    workspace_root: &Path,
    app_id: &str,
    scene_id: &str,
    route_mode: &str,
    data_mode: DataMode,
    compose: &ComposeRequest,
    draft_session: &str,
    _draft_digest: &str,
    hits: &mut ArtifactHitMatrix,
    shell_chrome: Option<&ShellChromeRenderer<'_>>,
) -> Result<SceneViewManifest> {
    let tab = compose
        .tab
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default_tab_for_route(route_mode));
    let chrome = compose
        .chrome
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("full");
    let review_projection = compose
        .review_projection
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| default_ssr_review_projection(data_mode).to_string());
    let review_projection = review_projection.as_str();
    let index = ensure_manifest_index(
        workspace_root,
        app_id,
        scene_id,
        data_mode,
        hits,
        shell_chrome,
    )?;
    let mut manifest = manifest_for_surface(&index, route_mode)
        .ok_or_else(|| anyhow::anyhow!("manifest surface missing for route `{route_mode}`"))?;
    let effective_draft_digest = String::new();
    let mut ctx = load_materialize_context(
        workspace_root,
        app_id,
        scene_id,
        data_mode,
        route_mode,
        tab,
        chrome,
        draft_session,
        effective_draft_digest.as_str(),
        None,
    )?;
    let shell_doc = materialize_shell(&mut ctx, hits, shell_chrome, None);
    manifest
        .layers
        .insert(format!("shell.{route_mode}"), shell_doc);
    manifest.compose_defaults = Some(ComposeRequest {
        route_mode: Some(route_mode.to_string()),
        tab: Some(tab.to_string()),
        chrome: Some(chrome.to_string()),
        review_projection: Some(review_projection.to_string()),
        data_mode: Some(data_mode.slug().to_string()),
        focus: compose.focus.clone(),
        scope: compose.scope.clone(),
    });
    let digest = manifest_revision_digest(
        &manifest,
        if effective_draft_digest.is_empty() {
            None
        } else {
            Some(effective_draft_digest.as_str())
        },
    );
    let surface_digest = surface_revision_digest_from_manifest(&manifest);
    Ok(SceneViewManifest {
        revision_digest: digest,
        surface_revision_digest: surface_digest,
        ..manifest
    })
}

fn materialize_layer_name(
    ctx: &mut MaterializeContext<'_>,
    layer: &str,
    hits: &mut ArtifactHitMatrix,
    shell_chrome: Option<&ShellChromeRenderer<'_>>,
    auth_sig: Option<u64>,
) -> Result<Value> {
    match layer {
        "structure.full" => {
            if let Some(doc) = materialize_structure_document(ctx) {
                hits.structure_hit = crate::take_layer(
                    structure_full_cache_key(&ctx.semantic_core, ctx.layout_rev.as_str()).as_str(),
                )
                .is_some();
                if !hits.structure_hit {
                    let _ = materialize_structure(ctx, hits)?;
                    hits.structure_hit = true;
                }
                return Ok(doc);
            }
            materialize_structure(ctx, hits)?;
            Ok(materialize_structure_document(ctx).unwrap_or(Value::Null))
        }
        "theme.tokens" => materialize_theme(ctx, hits),
        "layout.overlay" => materialize_overlay(ctx, hits),
        "runtime.plans" => materialize_runtime_plans(ctx, hits),
        name if name.starts_with("eval.slot_group.") => {
            let slot_group_id = name
                .strip_prefix("eval.slot_group.")
                .unwrap_or("scene:default");
            materialize_eval_group(ctx, slot_group_id, hits)
        }
        name if name.starts_with("shell.") => {
            ensure_materialize_assembled(ctx)?;
            Ok(materialize_shell(ctx, hits, shell_chrome, auth_sig))
        }
        _ => Ok(Value::Null),
    }
}

/// Materialize named layers for a layer-batch request.
pub fn materialize_layers_for_request(
    workspace_root: &Path,
    app_id: &str,
    scene_id: &str,
    route_mode: &str,
    data_mode: DataMode,
    compose: &ComposeRequest,
    draft_session: &str,
    _draft_digest: &str,
    layer_names: &[String],
    hits: &mut ArtifactHitMatrix,
    shell_chrome: Option<&ShellChromeRenderer<'_>>,
) -> Result<BTreeMap<String, Value>> {
    let tab = compose
        .tab
        .as_deref()
        .filter(|value| !value.is_empty())
        .unwrap_or("scene");
    let chrome = compose
        .chrome
        .as_deref()
        .filter(|value| !value.is_empty())
        .unwrap_or("full");
    let draft = None;
    let effective_draft_digest = String::new();
    let mut ctx = load_materialize_context(
        workspace_root,
        app_id,
        scene_id,
        data_mode,
        route_mode,
        tab,
        chrome,
        draft_session,
        effective_draft_digest.as_str(),
        draft,
    )?;
    let mut layers = BTreeMap::new();
    for layer in layer_names {
        let value = materialize_layer_name(&mut ctx, layer.as_str(), hits, shell_chrome, None)?;
        layers.insert(layer.clone(), value);
    }
    Ok(layers)
}

/// Resolve view-revision from the shared manifest index (Host/Runtime parity).
pub fn resolve_view_revision_for_surface(
    workspace_root: &Path,
    app_id: &str,
    scene_id: &str,
    route_mode: &str,
    data_mode: DataMode,
    client_manifest_digest: Option<String>,
    client_surface_digest: Option<String>,
    recover: bool,
    local_miss: bool,
    hits: &mut ArtifactHitMatrix,
    shell_chrome: Option<&ShellChromeRenderer<'_>>,
) -> Result<ViewRevisionResponse> {
    let index = ensure_manifest_index(
        workspace_root,
        app_id,
        scene_id,
        data_mode,
        hits,
        shell_chrome,
    )?;
    let manifest = manifest_for_surface(&index, route_mode)
        .ok_or_else(|| anyhow::anyhow!("manifest index missing surface {route_mode}"))?;
    let surface_digest = surface_revision_digest_from_manifest(&manifest);
    Ok(resolve_view_revision(&ViewRevisionInput {
        manifest: manifest.clone(),
        client_manifest_digest,
        client_surface_digest,
        recover,
        local_miss,
        client_layers: Vec::new(),
        missing_layers: Vec::new(),
        surface_revision_digest: surface_digest,
    }))
}
