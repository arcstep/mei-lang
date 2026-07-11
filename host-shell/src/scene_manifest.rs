//! Scene view manifest and layer batch APIs.

use axum::{
    extract::{Query, State},
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode},
    response::{IntoResponse, Json, Response},
};
use mei_host_auth::AuthServeState;
use mei_lang_app::{load_topbar_menu_context, UiRouteMode};
use mei_lang_kernel::{resolve_app_root, DataMode};

use crate::pages::AppQuery;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::artifact_observability::{ArtifactHitMatrix, LayerArtifactObservability};
use crate::landing::{discover_workspace_apps, enrich_discovered_apps};
use crate::review_axes::{resolve_page_render_axes_for_stage, StageKind};
use crate::state::SharedState;

#[derive(Debug, Deserialize, Default)]
pub struct SceneManifestQuery {
    pub app_id: String,
    pub scene: Option<String>,
    #[serde(default)]
    pub surface: Option<String>,
    pub data_mode: Option<String>,
    pub review_projection: Option<String>,
    pub tab: Option<String>,
    pub chrome: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LayerBatchRequest {
    pub app_id: String,
    pub scene: Option<String>,
    pub layers: Vec<String>,
    #[serde(default)]
    pub data_mode: Option<String>,
    #[serde(default)]
    pub local_miss: bool,
    #[serde(default)]
    pub client_layers: Vec<mei_host_graph::ClientLayerHolding>,
    #[serde(default)]
    pub surface: Option<String>,
    #[serde(default)]
    pub review_projection: Option<String>,
    #[serde(default)]
    pub tab: Option<String>,
    #[serde(default)]
    pub chrome: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct LayerBatchResponse {
    pub layers: std::collections::BTreeMap<String, Value>,
    pub hits: ArtifactHitMatrix,
}

/// Host chrome inputs for `shell.*` layer (real topbar/statusbar SSR).
pub(crate) struct SceneChromeHostContext<'a> {
    pub apps: &'a [mei_lang_kernel::WorkspaceAppMeta],
    pub topbar_menu: Option<&'a mei_lang_app::TopbarMenuContext>,
    pub auth_enabled: bool,
    pub auth_account: Option<&'a mei_lang_app::HostAccountView>,
}

struct MaterializeContext<'a> {
    workspace_root: &'a std::path::Path,
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
    semantic_core: mei_host_graph::SemanticCacheCore,
    compiled: Option<mei_lang_kernel::CompiledApp>,
    assemble_outcome: Option<mei_host_graph::AssembleOutcome>,
}

fn layout_policy_revision(workspace_root: &std::path::Path, app_id: &str) -> String {
    let app_root = resolve_app_root(workspace_root, app_id);
    mei_lang_kernel::load_cache_generation(app_root.as_path(), app_id).data_generation
}

fn theme_digest_for_app(workspace_root: &std::path::Path, app_id: &str) -> String {
    layout_policy_revision(workspace_root, app_id)
}

fn load_materialize_context<'a>(
    workspace_root: &'a std::path::Path,
    app_id: &'a str,
    scene_id: &'a str,
    data_mode: DataMode,
    route_mode: &'a str,
    tab: &'a str,
    chrome: &'a str,
    draft_session: &'a str,
    draft_digest: &'a str,
    draft: Option<Value>,
) -> anyhow::Result<MaterializeContext<'a>> {
    let layout_rev = layout_policy_revision(workspace_root, app_id);
    let semantic_core =
        mei_host_graph::build_semantic_core_for_scene(workspace_root, app_id, scene_id);
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

fn ensure_materialize_assembled(ctx: &mut MaterializeContext<'_>) -> anyhow::Result<()> {
    if ctx.assemble_outcome.is_some() {
        return Ok(());
    }
    let assemble_outcome =
        mei_host_graph::assemble_scope_from_registry(ctx.workspace_root, ctx.app_id, ctx.scene_id)?;
    ctx.compiled = assemble_outcome
        .as_ref()
        .map(|outcome| outcome.compiled.clone());
    ctx.assemble_outcome = assemble_outcome;
    Ok(())
}

fn materialize_structure(
    ctx: &mut MaterializeContext<'_>,
    hits: &mut ArtifactHitMatrix,
) -> anyhow::Result<mei_host_graph::LayerRef> {
    let structure_key =
        mei_host_graph::structure_full_cache_key(&ctx.semantic_core, ctx.layout_rev.as_str());
    if let Some(bytes) = mei_host_graph::take_layer(structure_key.as_str()) {
        hits.structure_hit = true;
        let hash = mei_host_graph::content_hash_bytes(bytes.as_slice());
        return Ok(mei_host_graph::LayerRef {
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
    let (_doc, pref, _key) = mei_host_graph::structure_full_from_compiled(
        ctx.workspace_root,
        compiled,
        &ctx.semantic_core,
        ctx.layout_rev.as_str(),
    )?;
    let document = mei_host_graph::build_structure_full_document(compiled, structure_key.as_str());
    let bytes = serde_json::to_vec(&document)?;
    mei_host_graph::store_layer(
        structure_key.clone(),
        mei_host_graph::STRUCTURE_FULL_KIND,
        pref.content_hash.as_str(),
        bytes.as_slice(),
    );
    hits.structure_hit = false;
    Ok(mei_host_graph::LayerRef {
        artifact_id: structure_key,
        content_hash: pref.content_hash,
        bytes: Some(bytes.len() as u64),
        encoding: Some("json".to_string()),
    })
}

fn materialize_structure_document(ctx: &mut MaterializeContext<'_>) -> Option<Value> {
    let structure_key =
        mei_host_graph::structure_full_cache_key(&ctx.semantic_core, ctx.layout_rev.as_str());
    if let Some(bytes) = mei_host_graph::take_layer(structure_key.as_str()) {
        return serde_json::from_slice(bytes.as_slice()).ok();
    }
    ensure_materialize_assembled(ctx).ok()?;
    let compiled = ctx.compiled.as_ref()?;
    let document = mei_host_graph::build_structure_full_document(compiled, structure_key.as_str());
    Some(serde_json::to_value(document).unwrap_or(Value::Null))
}

fn eval_slot_group_layer_ref(
    ctx: &mut MaterializeContext<'_>,
    slot_group_id: &str,
    hits: &mut ArtifactHitMatrix,
) -> anyhow::Result<mei_host_graph::LayerRef> {
    let eval_key = mei_host_graph::eval_slot_group_cache_key(
        &ctx.semantic_core,
        slot_group_id,
        ctx.data_mode.slug(),
        "default",
    );
    if mei_host_graph::take_layer(eval_key.as_str()).is_some() {
        hits.eval_hit = true;
    }
    ensure_materialize_assembled(ctx)?;
    let compiled = ctx
        .compiled
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("assemble unavailable"))?;
    let (_doc, pref, _cached) = mei_host_graph::ensure_eval_slot_group_cached(
        ctx.workspace_root,
        compiled,
        &ctx.semantic_core,
        slot_group_id,
        ctx.data_mode,
        ctx.layout_rev.as_str(),
    )?;
    hits.eval_hit = true;
    Ok(mei_host_graph::LayerRef {
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
) -> anyhow::Result<Value> {
    let eval_key = mei_host_graph::eval_slot_group_cache_key(
        &ctx.semantic_core,
        slot_group_id,
        ctx.data_mode.slug(),
        "default",
    );
    if mei_host_graph::take_layer(eval_key.as_str()).is_some() {
        hits.eval_hit = true;
    }
    ensure_materialize_assembled(ctx)?;
    let compiled = ctx
        .compiled
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("assemble unavailable"))?;
    let (doc, pref, _cached) = mei_host_graph::ensure_eval_slot_group_cached(
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
) -> anyhow::Result<Value> {
    let cache_key =
        mei_host_graph::runtime_plans_cache_key(&ctx.semantic_core, ctx.layout_rev.as_str());
    if let Some(bytes) = mei_host_graph::take_layer(cache_key.as_str()) {
        let doc: mei_host_graph::RuntimePlansDocument = serde_json::from_slice(bytes.as_slice())?;
        let content_hash = mei_host_graph::content_hash_bytes(bytes.as_slice());
        return Ok(json!({
            "artifact_id": cache_key,
            "content_hash": content_hash,
            "document": doc,
        }));
    }
    let document = if let Some(outcome) = ctx.assemble_outcome.as_ref() {
        mei_host_graph::runtime_plans_from_outcome(outcome, ctx.workspace_root)
    } else {
        ensure_materialize_assembled(ctx)?;
        if let Some(outcome) = ctx.assemble_outcome.as_ref() {
            mei_host_graph::runtime_plans_from_outcome(outcome, ctx.workspace_root)
        } else {
            mei_host_graph::empty_runtime_plans_document(ctx.app_id, ctx.scene_id)
        }
    };
    let app_root = mei_lang_kernel::resolve_app_root(ctx.workspace_root, ctx.app_id);
    let pref = mei_host_graph::persist_runtime_plans(app_root.as_path(), &document)?;
    let bytes = serde_json::to_vec(&document)?;
    mei_host_graph::store_layer(
        cache_key.clone(),
        mei_host_graph::RUNTIME_PLANS_KIND,
        pref.content_hash.as_str(),
        bytes.as_slice(),
    );
    Ok(json!({
        "artifact_id": cache_key,
        "content_hash": pref.content_hash,
        "document": document,
    }))
}

fn materialize_theme(
    ctx: &MaterializeContext<'_>,
    hits: &mut ArtifactHitMatrix,
) -> anyhow::Result<Value> {
    let (doc, hit) = mei_host_graph::ensure_theme_tokens_cached(
        ctx.workspace_root,
        ctx.app_id,
        ctx.theme_digest.as_str(),
    )?;
    hits.theme_hit = hit;
    let key = mei_host_graph::theme_tokens_cache_key(ctx.theme_digest.as_str());
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
) -> anyhow::Result<Value> {
    let draft_ref = ctx.draft.as_ref();
    let (doc, hit) = mei_host_graph::ensure_layout_overlay_cached(
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
    let persisted = mei_host_graph::layout_overlay_persisted_cache_key(ctx.layout_rev.as_str());
    let session_key = if ctx.draft_digest.is_empty() {
        None
    } else {
        Some(mei_host_graph::layout_overlay_session_cache_key(
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
    route_mode: UiRouteMode,
    chrome_host: Option<&SceneChromeHostContext<'_>>,
    auth_sig: Option<u64>,
) -> Value {
    let route_slug = ctx.route_mode.to_string();
    if chrome_host.is_some() {
        if let Err(error) = ensure_materialize_assembled(ctx) {
            tracing::warn!(
                target: "mei.scene_manifest",
                app_id = %ctx.app_id,
                scene_id = %ctx.scene_id,
                error = %error,
                "assemble unavailable for shell chrome"
            );
        }
    }
    if let (Some(host), Some(compiled)) = (chrome_host, ctx.compiled.as_ref()) {
        let stage_kind = StageKind::from_scene_routes(&compiled.scene_routes, ctx.scene_id);
        let review_projection =
            crate::review_axes::ssr_review_projection(route_mode, stage_kind, ctx.data_mode).slug();
        let (mut topbar_html, mut statusbar_html) = mei_lang_app::render_access_shell_chrome_html(
            host.apps,
            compiled,
            ctx.app_id,
            host.topbar_menu,
            route_mode,
            Some(ctx.scene_id),
            None,
            Some(ctx.tab),
            host.auth_enabled,
            host.auth_account,
            Some(ctx.data_mode.slug()),
            Some(review_projection),
            ctx.chrome == "none",
        );
        topbar_html =
            crate::build_info::fill_page_shell_placeholders(topbar_html, ctx.workspace_root);
        statusbar_html =
            crate::build_info::fill_page_shell_placeholders(statusbar_html, ctx.workspace_root);
        let doc = mei_host_graph::ShellLayerDocument {
            schema_version: mei_host_graph::SHELL_LAYER_SCHEMA.to_string(),
            route_mode: route_slug.clone(),
            tab: ctx.tab.to_string(),
            chrome: ctx.chrome.to_string(),
            topbar_html,
            statusbar_html,
        };
        mei_host_graph::store_shell_layer_document(
            ctx.app_id,
            ctx.scene_id,
            ctx.route_mode,
            ctx.tab,
            ctx.chrome,
            auth_sig,
            &doc,
        );
        hits.shell_hit = false;
        let key = mei_host_graph::shell_cache_key(
            ctx.app_id,
            ctx.scene_id,
            ctx.route_mode,
            ctx.tab,
            ctx.chrome,
            auth_sig,
            mei_host_graph::SHELL_LAYER_SCHEMA,
        );
        let content_hash = mei_host_graph::content_hash_bytes(
            serde_json::to_vec(&doc).unwrap_or_default().as_slice(),
        );
        return json!({
            "artifact_id": key,
            "content_hash": content_hash,
            "document": doc,
        });
    }
    let (doc, hit) = mei_host_graph::ensure_shell_layer_rendered(
        ctx.app_id,
        ctx.scene_id,
        ctx.route_mode,
        ctx.tab,
        ctx.chrome,
        auth_sig,
        || mei_host_graph::build_shell_layer_document(ctx.route_mode, ctx.tab, ctx.chrome),
    );
    hits.shell_hit = hit;
    let key = mei_host_graph::shell_cache_key(
        ctx.app_id,
        ctx.scene_id,
        ctx.route_mode,
        ctx.tab,
        ctx.chrome,
        auth_sig,
        mei_host_graph::SHELL_LAYER_SCHEMA,
    );
    let content_hash =
        mei_host_graph::content_hash_bytes(serde_json::to_vec(&doc).unwrap_or_default().as_slice());
    json!({
        "artifact_id": key,
        "content_hash": content_hash,
        "document": doc,
    })
}

fn layer_ref_from_materialized(value: &Value) -> Option<mei_host_graph::LayerRef> {
    mei_host_graph::layer_ref_from_manifest_entry("layer", value)
}

fn manifest_index_needs_shell_rebuild(index: &mei_host_graph::ManifestIndexDocument) -> bool {
    if index.surfaces.is_empty() {
        return true;
    }
    for surface in &index.surfaces {
        if surface.shell_layer_ref.is_empty() {
            return true;
        }
        for (_, layer_ref) in &surface.shell_layer_ref {
            let Some(bytes) = mei_host_graph::take_layer(layer_ref.artifact_id.as_str()) else {
                return true;
            };
            let Ok(doc) =
                serde_json::from_slice::<mei_host_graph::ShellLayerDocument>(bytes.as_slice())
            else {
                return true;
            };
            if mei_host_graph::is_placeholder_shell_document(&doc) {
                return true;
            }
        }
    }
    false
}

pub(crate) fn ensure_manifest_index(
    workspace_root: &std::path::Path,
    app_id: &str,
    scene_id: &str,
    data_mode: DataMode,
    hits: &mut ArtifactHitMatrix,
    chrome_host: Option<&SceneChromeHostContext<'_>>,
) -> anyhow::Result<mei_host_graph::ManifestIndexDocument> {
    let layout_rev = layout_policy_revision(workspace_root, app_id);
    let semantic_core =
        mei_host_graph::build_semantic_core_for_scene(workspace_root, app_id, scene_id);
    let cache_key = mei_host_graph::manifest_index_cache_key(
        &semantic_core,
        layout_rev.as_str(),
        data_mode.slug(),
    );
    if let Some(index) = mei_host_graph::take_manifest_index(cache_key.as_str()) {
        let rebuild = chrome_host.is_some() && manifest_index_needs_shell_rebuild(&index);
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
        chrome_host,
    )
}

fn build_and_store_manifest_index(
    workspace_root: &std::path::Path,
    app_id: &str,
    scene_id: &str,
    data_mode: DataMode,
    layout_rev: String,
    semantic_core: mei_host_graph::SemanticCacheCore,
    cache_key: String,
    hits: &mut ArtifactHitMatrix,
    chrome_host: Option<&SceneChromeHostContext<'_>>,
) -> anyhow::Result<mei_host_graph::ManifestIndexDocument> {
    let mut ctx = load_materialize_context(
        workspace_root,
        app_id,
        scene_id,
        data_mode,
        UiRouteMode::App.slug(),
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

    let mut semantic_layer_refs = std::collections::BTreeMap::new();
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
        if let Ok(structure) =
            serde_json::from_value::<mei_host_graph::StructureFullDocument>(doc_value)
        {
            eval_slot_group_ids = mei_host_graph::collect_slot_groups(&structure);
            for group_id in &eval_slot_group_ids {
                let layer_ref = eval_slot_group_layer_ref(&mut ctx, group_id.as_str(), hits)?;
                semantic_layer_refs.insert(format!("eval.slot_group.{group_id}"), layer_ref);
            }
        }
    }

    let mut semantic_layers = std::collections::BTreeMap::new();
    for (name, layer_ref) in &semantic_layer_refs {
        semantic_layers.insert(
            name.clone(),
            serde_json::to_value(layer_ref).unwrap_or(Value::Null),
        );
    }
    let semantic_manifest = mei_host_graph::SceneViewManifest {
        schema_version: mei_host_graph::SCENE_VIEW_MANIFEST_SCHEMA.to_string(),
        app_id: app_id.to_string(),
        scene_id: scene_id.to_string(),
        semantic_core: semantic_core.clone(),
        revision_digest: String::new(),
        layers: semantic_layers,
        compose_defaults: None,
        surface_revision_digest: None,
    };
    let manifest_revision_digest =
        mei_host_graph::manifest_revision_digest(&semantic_manifest, None);

    let mut surfaces = Vec::new();
    let stage_kind = ctx
        .compiled
        .as_ref()
        .map(|compiled| StageKind::from_scene_routes(&compiled.scene_routes, scene_id))
        .unwrap_or(StageKind::Scene);
    let route_mode = UiRouteMode::App;
    let route_slug = route_mode.slug();
    let tab = "scene";
    let axes = crate::review_axes::resolve_page_render_axes_with_ceiling_detailed_for_stage(
        mei_lang_kernel::DataModeCeiling::Eval,
        &AppQuery {
            data_mode: Some(data_mode.slug().to_string()),
            ..Default::default()
        },
        route_mode,
        stage_kind,
    )
    .axes;
    ctx.route_mode = route_slug;
    ctx.tab = tab;
    ctx.chrome = "full";
    let compose_defaults = mei_host_graph::ComposeRequest {
        route_mode: Some(route_slug.to_string()),
        tab: Some(tab.to_string()),
        chrome: Some("full".to_string()),
        review_projection: Some(axes.review_projection.slug().to_string()),
        data_mode: Some(axes.data_mode.slug().to_string()),
        focus: None,
        scope: None,
    };
    let shell_doc = materialize_shell(&mut ctx, hits, route_mode, chrome_host, None);
    let shell_layer_name = format!("shell.{route_slug}");
    let mut shell_layer_ref = std::collections::BTreeMap::new();
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
    let surface_manifest = mei_host_graph::SceneViewManifest {
        schema_version: mei_host_graph::SCENE_VIEW_MANIFEST_SCHEMA.to_string(),
        app_id: app_id.to_string(),
        scene_id: scene_id.to_string(),
        semantic_core: semantic_core.clone(),
        revision_digest: manifest_revision_digest.clone(),
        layers,
        compose_defaults: Some(compose_defaults.clone()),
        surface_revision_digest: None,
    };
    let surface_revision_digest =
        mei_host_graph::surface_revision_digest_from_manifest(&surface_manifest);
    surfaces.push(mei_host_graph::SurfaceManifestSlice {
        route_mode: route_slug.to_string(),
        shell_layer_name,
        surface_revision_digest: surface_revision_digest.unwrap_or_default(),
        compose_defaults,
        shell_layer_ref,
    });

    let index = mei_host_graph::ManifestIndexDocument {
        schema_version: mei_host_graph::MANIFEST_INDEX_SCHEMA.to_string(),
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
    let _pref = mei_host_graph::persist_manifest_index(app_root.as_path(), &index)?;
    mei_host_graph::store_manifest_index_memory(cache_key.as_str(), &index);
    Ok(index)
}

fn semantic_layers_from_refs(
    refs: &std::collections::BTreeMap<String, mei_host_graph::LayerRef>,
) -> std::collections::BTreeMap<String, Value> {
    refs.iter()
        .map(|(name, layer_ref)| {
            (
                name.clone(),
                serde_json::to_value(layer_ref).unwrap_or(Value::Null),
            )
        })
        .collect()
}

pub(crate) fn manifest_for_surface(
    index: &mei_host_graph::ManifestIndexDocument,
    route_mode: UiRouteMode,
) -> Option<mei_host_graph::SceneViewManifest> {
    mei_host_graph::manifest_index_to_scene_manifest(index, route_mode.slug())
}

pub(crate) fn build_scene_view_manifest(
    workspace_root: &std::path::Path,
    app_id: &str,
    scene_id: &str,
    route_mode: UiRouteMode,
    data_mode: DataMode,
    compose: &mei_host_graph::ComposeRequest,
    draft_session: &str,
    _draft_digest: &str,
    hits: &mut ArtifactHitMatrix,
    chrome_host: Option<&SceneChromeHostContext<'_>>,
) -> anyhow::Result<mei_host_graph::SceneViewManifest> {
    let route_slug = route_mode.slug();
    let tab = compose
        .tab
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(if route_mode.uses_workspace_tree() {
            "preview"
        } else {
            "scene"
        });
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
        .unwrap_or_else(|| {
            let stage_kind = stage_kind_for_scene(workspace_root, app_id, scene_id);
            crate::review_axes::ssr_review_projection(route_mode, stage_kind, data_mode)
                .slug()
                .to_string()
        });
    let review_projection = review_projection.as_str();
    let index = ensure_manifest_index(
        workspace_root,
        app_id,
        scene_id,
        data_mode,
        hits,
        chrome_host,
    )?;
    let mut manifest = manifest_for_surface(&index, route_mode)
        .ok_or_else(|| anyhow::anyhow!("manifest surface missing for route `{route_slug}`"))?;
    let effective_draft_digest = String::new();
    let mut ctx = load_materialize_context(
        workspace_root,
        app_id,
        scene_id,
        data_mode,
        route_slug,
        tab,
        chrome,
        draft_session,
        effective_draft_digest.as_str(),
        None,
    )?;
    let shell_doc = materialize_shell(&mut ctx, hits, route_mode, chrome_host, None);
    manifest
        .layers
        .insert(format!("shell.{route_slug}"), shell_doc);
    manifest.compose_defaults = Some(mei_host_graph::ComposeRequest {
        route_mode: Some(route_slug.to_string()),
        tab: Some(tab.to_string()),
        chrome: Some(chrome.to_string()),
        review_projection: Some(review_projection.to_string()),
        data_mode: Some(data_mode.slug().to_string()),
        focus: compose.focus.clone(),
        scope: compose.scope.clone(),
    });
    let digest = mei_host_graph::manifest_revision_digest(
        &manifest,
        if effective_draft_digest.is_empty() {
            None
        } else {
            Some(effective_draft_digest.as_str())
        },
    );
    let surface_digest = mei_host_graph::surface_revision_digest_from_manifest(&manifest);
    Ok(mei_host_graph::SceneViewManifest {
        revision_digest: digest,
        surface_revision_digest: surface_digest,
        ..manifest
    })
}

fn materialize_layer_name(
    ctx: &mut MaterializeContext<'_>,
    layer: &str,
    hits: &mut ArtifactHitMatrix,
    route_mode: UiRouteMode,
    chrome_host: Option<&SceneChromeHostContext<'_>>,
    auth_sig: Option<u64>,
) -> anyhow::Result<Value> {
    match layer {
        "structure.full" => {
            if let Some(doc) = materialize_structure_document(ctx) {
                hits.structure_hit = mei_host_graph::take_layer(
                    mei_host_graph::structure_full_cache_key(
                        &ctx.semantic_core,
                        ctx.layout_rev.as_str(),
                    )
                    .as_str(),
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
            Ok(materialize_shell(
                ctx,
                hits,
                route_mode,
                chrome_host,
                auth_sig,
            ))
        }
        _ => Ok(Value::Null),
    }
}

pub async fn api_host_scene_manifest(
    State(state): State<SharedState>,
    State(_auth): State<AuthServeState>,
    _headers: HeaderMap,
    Query(query): Query<SceneManifestQuery>,
) -> Response {
    let app_id = query.app_id.trim();
    if app_id.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "app_id is required"})),
        )
            .into_response();
    }
    let scene_id = query
        .scene
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("home")
        .to_string();
    let guard = state.read().expect("state lock");
    let workspace_root = guard.ctx.workspace_root.as_path();
    let route_mode = resolve_route_mode_from_surface(query.surface.as_deref());
    let stage_kind = stage_kind_for_scene(workspace_root, app_id, scene_id.as_str());
    let axes = resolve_page_render_axes_for_stage(
        &guard,
        &AppQuery {
            data_mode: query.data_mode.clone(),
            review_projection: query.review_projection.clone(),
            ..Default::default()
        },
        route_mode,
        stage_kind,
    );
    let draft_digest = String::new();

    let compose = mei_host_graph::ComposeRequest {
        route_mode: Some(route_mode.slug().to_string()),
        tab: query.tab.clone(),
        chrome: query.chrome.clone(),
        review_projection: Some(axes.review_projection.slug().to_string()),
        data_mode: Some(axes.data_mode.slug().to_string()),
        focus: None,
        scope: None,
    };
    let mut hits = ArtifactHitMatrix::default();
    let manifest = match build_scene_view_manifest(
        workspace_root,
        app_id,
        scene_id.as_str(),
        route_mode,
        axes.data_mode,
        &compose,
        "",
        draft_digest.as_str(),
        &mut hits,
        None,
    ) {
        Ok(value) => value,
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": err.to_string()})),
            )
                .into_response();
        }
    };

    let obs = LayerArtifactObservability { hits };
    let mut response = Json(json!({
        "manifest": manifest,
        "hits": obs.hits,
    }))
    .into_response();
    for (name, value) in obs.response_headers() {
        if let Ok(header_value) = HeaderValue::from_str(value.as_str()) {
            response
                .headers_mut()
                .insert(HeaderName::from_static(name), header_value);
        }
    }
    response
}

#[cfg(test)]
mod tests {
    use axum::http::HeaderMap;

    #[test]
    fn parse_artifact_hits_roundtrip() {
        let mut headers = HeaderMap::new();
        headers.insert("x-mei-structure-hit", "1".parse().unwrap());
        headers.insert("x-mei-eval-hit", "0".parse().unwrap());
        let hits = crate::artifact_observability::parse_artifact_hits_from_headers(&headers);
        assert!(hits.structure_hit);
        assert!(!hits.eval_hit);
    }
}

pub(crate) fn materialize_layers_for_request(
    workspace_root: &std::path::Path,
    app_id: &str,
    scene_id: &str,
    route_mode: UiRouteMode,
    data_mode: DataMode,
    compose: &mei_host_graph::ComposeRequest,
    draft_session: &str,
    _draft_digest: &str,
    layer_names: &[String],
    hits: &mut ArtifactHitMatrix,
    chrome_host: Option<&SceneChromeHostContext<'_>>,
) -> anyhow::Result<std::collections::BTreeMap<String, Value>> {
    let route_slug = route_mode.slug();
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
        route_slug,
        tab,
        chrome,
        draft_session,
        effective_draft_digest.as_str(),
        draft,
    )?;
    let mut layers = std::collections::BTreeMap::new();
    for layer in layer_names {
        let value = materialize_layer_name(
            &mut ctx,
            layer.as_str(),
            hits,
            route_mode,
            chrome_host,
            None,
        )?;
        layers.insert(layer.clone(), value);
    }
    Ok(layers)
}

fn stage_kind_for_scene(
    workspace_root: &std::path::Path,
    app_id: &str,
    scene_id: &str,
) -> StageKind {
    match mei_host_graph::assemble_scope_from_registry(workspace_root, app_id, scene_id) {
        Ok(Some(outcome)) => {
            StageKind::from_scene_routes(&outcome.compiled.scene_routes, scene_id)
        }
        _ => StageKind::Scene,
    }
}

pub(crate) fn resolve_route_mode_from_surface(surface: Option<&str>) -> UiRouteMode {
    match surface.map(str::trim).filter(|value| !value.is_empty()) {
        Some("build") | Some("manage") | Some("layout") | Some("prototype") => UiRouteMode::App,
        Some("run") | Some("copilot") | Some("speaker") | Some("presentation") | Some("slides") => {
            UiRouteMode::App
        }
        Some("app") | None => UiRouteMode::App,
        Some(other) => UiRouteMode::from_slug(other),
    }
}

pub async fn api_host_layer_batch(
    State(state): State<SharedState>,
    State(_auth): State<AuthServeState>,
    _headers: HeaderMap,
    Json(body): Json<LayerBatchRequest>,
) -> Response {
    let app_id = body.app_id.trim();
    if app_id.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "app_id is required"})),
        )
            .into_response();
    }
    let scene_id = body
        .scene
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("home")
        .to_string();
    let guard = state.read().expect("state lock");
    let workspace_root = guard.ctx.workspace_root.as_path();
    let route_mode = resolve_route_mode_from_surface(body.surface.as_deref());
    let stage_kind = stage_kind_for_scene(workspace_root, app_id, scene_id.as_str());
    let axes = resolve_page_render_axes_for_stage(
        &guard,
        &AppQuery {
            data_mode: body.data_mode.clone(),
            review_projection: body.review_projection.clone(),
            ..Default::default()
        },
        route_mode,
        stage_kind,
    );
    let draft_digest = String::new();
    let compose = mei_host_graph::ComposeRequest {
        route_mode: Some(route_mode.slug().to_string()),
        tab: body.tab.clone(),
        chrome: body.chrome.clone(),
        review_projection: body.review_projection.clone(),
        data_mode: Some(axes.data_mode.slug().to_string()),
        focus: None,
        scope: None,
    };

    let mut hits = ArtifactHitMatrix::default();
    let topbar_menu = load_topbar_menu_context(workspace_root);
    let discovered = discover_workspace_apps(workspace_root).unwrap_or_default();
    let apps = enrich_discovered_apps(discovered.as_slice(), &topbar_menu);
    let chrome_host = SceneChromeHostContext {
        apps: apps.as_slice(),
        topbar_menu: Some(&topbar_menu),
        auth_enabled: false,
        auth_account: None,
    };
    let layers = match materialize_layers_for_request(
        workspace_root,
        app_id,
        scene_id.as_str(),
        route_mode,
        axes.data_mode,
        &compose,
        "",
        draft_digest.as_str(),
        body.layers.as_slice(),
        &mut hits,
        Some(&chrome_host),
    ) {
        Ok(value) => value,
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": err.to_string()})),
            )
                .into_response();
        }
    };

    let obs = LayerArtifactObservability { hits };
    let status = if body.local_miss {
        "refetch"
    } else {
        "refetch"
    };
    let mut response = Json(LayerBatchResponse {
        layers,
        hits: obs.hits,
    })
    .into_response();
    for (name, value) in obs.response_headers() {
        if let Ok(header_value) = HeaderValue::from_str(value.as_str()) {
            response
                .headers_mut()
                .insert(HeaderName::from_static(name), header_value);
        }
    }
    if let Ok(header_value) = HeaderValue::from_str(status) {
        response.headers_mut().insert(
            HeaderName::from_static("x-mei-view-revision-status"),
            header_value,
        );
    }
    if body.local_miss {
        if let Ok(header_value) = HeaderValue::from_str("1") {
            response
                .headers_mut()
                .insert(HeaderName::from_static("x-mei-local-miss"), header_value);
        }
    }
    if !body.client_layers.is_empty() {
        if let Ok(header_value) =
            HeaderValue::from_str(body.client_layers.len().to_string().as_str())
        {
            response.headers_mut().insert(
                HeaderName::from_static("x-mei-client-holdings-count"),
                header_value,
            );
        }
    }
    response
}

#[cfg(test)]
mod cross_surface_manifest_tests {
    use super::*;
    use mei_lang_kernel::DataMode;
    use std::path::PathBuf;

    fn ws_demo_workspace() -> Option<PathBuf> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../workspaces/ws-demo-v2")
            .canonicalize()
            .ok()?;
        let registry = root.join("apps/data-demo/build/active/registry/mcg-registry.json");
        if !registry.is_file() {
            return None;
        }
        match mei_host_graph::assemble_scope_from_registry(root.as_path(), "data-demo", "home") {
            Ok(Some(_)) => Some(root),
            _ => None,
        }
    }

    fn structure_artifact_id(manifest: &mei_host_graph::SceneViewManifest) -> String {
        manifest
            .layers
            .get("structure.full")
            .and_then(|value| value.get("artifact_id"))
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .to_string()
    }

    #[test]
    fn app_surface_emits_structure_full_artifact_id() {
        let Some(workspace_root) = ws_demo_workspace() else {
            return;
        };
        let app_id = "data-demo";
        let scene_id = "home";
        let compose = mei_host_graph::ComposeRequest {
            route_mode: Some("app".to_string()),
            tab: Some("scene".to_string()),
            chrome: Some("full".to_string()),
            review_projection: None,
            data_mode: Some("static".to_string()),
            focus: None,
            scope: None,
        };
        let mut hits = ArtifactHitMatrix::default();
        let manifest = build_scene_view_manifest(
            workspace_root.as_path(),
            app_id,
            scene_id,
            UiRouteMode::App,
            DataMode::Static,
            &compose,
            "",
            "",
            &mut hits,
            None,
        )
        .expect("scene manifest");
        let artifact_id = structure_artifact_id(&manifest);
        assert!(
            !artifact_id.is_empty(),
            "missing structure.full for App surface"
        );
    }

    fn layer_content_hash(manifest: &mei_host_graph::SceneViewManifest, name: &str) -> String {
        manifest
            .layers
            .get(name)
            .and_then(|value| value.get("content_hash"))
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .to_string()
    }

    fn static_compose(route_slug: &str) -> mei_host_graph::ComposeRequest {
        mei_host_graph::ComposeRequest {
            route_mode: Some(route_slug.to_string()),
            tab: Some("scene".to_string()),
            chrome: Some("full".to_string()),
            review_projection: None,
            data_mode: Some("static".to_string()),
            focus: None,
            scope: None,
        }
    }

    #[test]
    fn app_surface_has_semantic_layer_hashes_and_digest() {
        let Some(workspace_root) = ws_demo_workspace() else {
            return;
        };
        let app_id = "data-demo";
        let scene_id = "home";
        let mut hits = ArtifactHitMatrix::default();
        let compose = static_compose("app");
        let manifest = build_scene_view_manifest(
            workspace_root.as_path(),
            app_id,
            scene_id,
            UiRouteMode::App,
            DataMode::Static,
            &compose,
            "",
            "",
            &mut hits,
            None,
        )
        .expect("scene manifest");
        for layer_name in [
            "structure.full",
            "theme.tokens",
            "layout.overlay",
            "runtime.plans",
            "eval.slot_group.scene:default",
        ] {
            let hash = layer_content_hash(&manifest, layer_name);
            assert!(!hash.is_empty(), "missing {layer_name}");
        }
        let semantic = mei_host_graph::semantic_revision_digest(&manifest, None);
        assert!(!semantic.is_empty());
        let surface = mei_host_graph::surface_revision_digest_from_manifest(&manifest);
        assert!(surface.is_some());
    }

    #[test]
    fn app_compose_defaults_follow_stage_kind_scene() {
        let Some(workspace_root) = ws_demo_workspace() else {
            return;
        };
        let mut hits = ArtifactHitMatrix::default();
        let compose = mei_host_graph::ComposeRequest {
            route_mode: Some("app".to_string()),
            tab: Some("scene".to_string()),
            chrome: Some("full".to_string()),
            review_projection: Some("live_full".to_string()),
            data_mode: Some("static".to_string()),
            focus: None,
            scope: None,
        };
        let manifest = build_scene_view_manifest(
            workspace_root.as_path(),
            "data-demo",
            "home",
            UiRouteMode::App,
            DataMode::Static,
            &compose,
            "",
            "",
            &mut hits,
            None,
        )
        .expect("scene manifest");
        let defaults = manifest
            .compose_defaults
            .as_ref()
            .expect("compose_defaults");
        assert_eq!(defaults.review_projection.as_deref(), Some("live_full"));
        assert_eq!(
            StageKind::from_route_meta("scene", "src/scene/home.mei"),
            StageKind::Scene
        );
    }

    #[test]
    fn data_demo_ssr_manifest_refs_stay_compact() {
        let Some(workspace_root) = ws_demo_workspace() else {
            return;
        };
        let mut hits = ArtifactHitMatrix::default();
        let compose = static_compose("app");
        let manifest = build_scene_view_manifest(
            workspace_root.as_path(),
            "data-demo",
            "home",
            UiRouteMode::App,
            DataMode::Eval,
            &compose,
            "",
            "",
            &mut hits,
            None,
        )
        .expect("scene manifest");
        let serialized = serde_json::to_string(&manifest).expect("manifest should serialize");
        assert!(
            serialized.len() < 512 * 1024,
            "SSR manifest refs should be compact (got {} bytes)",
            serialized.len()
        );
        for (name, layer) in &manifest.layers {
            if name.starts_with("eval.slot_group.") {
                assert!(
                    layer.get("document").is_none(),
                    "eval layer `{name}` should be ref-only in SSR manifest"
                );
                assert!(
                    layer.get("bootstrap_seed").is_none(),
                    "eval layer `{name}` must not duplicate bootstrap_seed"
                );
            }
        }
    }

    #[test]
    fn resolve_route_mode_from_surface_maps_legacy_slugs() {
        assert_eq!(
            resolve_route_mode_from_surface(Some("build")),
            UiRouteMode::App
        );
        assert_eq!(
            resolve_route_mode_from_surface(Some("manage")),
            UiRouteMode::App
        );
        assert_eq!(
            resolve_route_mode_from_surface(Some("run")),
            UiRouteMode::App
        );
        assert_eq!(
            resolve_route_mode_from_surface(Some("copilot")),
            UiRouteMode::App
        );
        assert_eq!(
            resolve_route_mode_from_surface(Some("layout")),
            UiRouteMode::App
        );
        assert_eq!(
            resolve_route_mode_from_surface(Some("prototype")),
            UiRouteMode::App
        );
    }

    #[test]
    fn shell_app_topbar_non_placeholder_with_chrome_host() {
        let Some(workspace_root) = ws_demo_workspace() else {
            return;
        };
        let topbar_menu = load_topbar_menu_context(workspace_root.as_path());
        let discovered = discover_workspace_apps(workspace_root.as_path()).unwrap_or_default();
        let apps = enrich_discovered_apps(discovered.as_slice(), &topbar_menu);
        let chrome_host = SceneChromeHostContext {
            apps: apps.as_slice(),
            topbar_menu: Some(&topbar_menu),
            auth_enabled: false,
            auth_account: None,
        };
        let mut hits = ArtifactHitMatrix::default();
        let compose = static_compose("app");
        let manifest = build_scene_view_manifest(
            workspace_root.as_path(),
            "data-demo",
            "home",
            UiRouteMode::App,
            DataMode::Static,
            &compose,
            "",
            "",
            &mut hits,
            Some(&chrome_host),
        )
        .expect("scene manifest");
        let shell = manifest
            .layers
            .get("shell.app")
            .and_then(|value| value.get("document"))
            .expect("shell.app document");
        let topbar = shell
            .get("topbar_html")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        assert!(
            topbar.len() > 500,
            "expected real topbar html, got len {}",
            topbar.len()
        );
        let doc: mei_host_graph::ShellLayerDocument =
            serde_json::from_value(shell.clone()).expect("shell document");
        assert!(
            !mei_host_graph::is_placeholder_shell_document(&doc),
            "shell.app must not be placeholder"
        );
    }

    #[test]
    fn materialize_shell_replaces_placeholder_cache() {
        let Some(workspace_root) = ws_demo_workspace() else {
            return;
        };
        let app_id = "data-demo";
        let route_mode = UiRouteMode::App;
        let tab = "scene";
        let chrome = "full";
        let placeholder =
            mei_host_graph::build_shell_layer_document(route_mode.slug(), tab, chrome);
        assert!(mei_host_graph::is_placeholder_shell_document(&placeholder));
        mei_host_graph::store_shell_layer_document(
            app_id,
            "home",
            route_mode.slug(),
            tab,
            chrome,
            None,
            &placeholder,
        );
        let topbar_menu = load_topbar_menu_context(workspace_root.as_path());
        let discovered = discover_workspace_apps(workspace_root.as_path()).unwrap_or_default();
        let apps = enrich_discovered_apps(discovered.as_slice(), &topbar_menu);
        let chrome_host = SceneChromeHostContext {
            apps: apps.as_slice(),
            topbar_menu: Some(&topbar_menu),
            auth_enabled: false,
            auth_account: None,
        };
        let mut hits = ArtifactHitMatrix::default();
        let compose = static_compose("app");
        let manifest = build_scene_view_manifest(
            workspace_root.as_path(),
            app_id,
            "home",
            route_mode,
            DataMode::Static,
            &compose,
            "",
            "",
            &mut hits,
            Some(&chrome_host),
        )
        .expect("scene manifest");
        let shell = manifest
            .layers
            .get("shell.app")
            .and_then(|value| value.get("document"))
            .expect("shell.app document");
        let doc: mei_host_graph::ShellLayerDocument =
            serde_json::from_value(shell.clone()).expect("shell document");
        assert!(
            !mei_host_graph::is_placeholder_shell_document(&doc),
            "placeholder cache must be replaced by real chrome"
        );
        assert!(doc.topbar_html.len() > 500);
    }
}
