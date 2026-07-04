//! Scene view manifest and layer batch APIs.

use axum::{
    extract::{Query, State},
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode},
    response::{IntoResponse, Json, Response},
};
use mei_host_auth::AuthServeState;
use mei_lang_app::UiRouteMode;
use mei_lang_kernel::{resolve_app_root, DataMode};

use crate::pages::AppQuery;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::artifact_observability::{ArtifactHitMatrix, LayerArtifactObservability};
use crate::review_axes::resolve_page_render_axes;
use crate::state::SharedState;

#[derive(Debug, Deserialize, Default)]
pub struct SceneManifestQuery {
    pub app_id: String,
    pub scene: Option<String>,
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
}

#[derive(Debug, Serialize)]
pub struct LayerBatchResponse {
    pub layers: std::collections::BTreeMap<String, Value>,
    pub hits: ArtifactHitMatrix,
}

fn layout_policy_revision(workspace_root: &std::path::Path, app_id: &str) -> String {
    let app_root = resolve_app_root(workspace_root, app_id);
    mei_lang_kernel::load_cache_generation(app_root.as_path(), app_id).data_generation
}

fn theme_digest(workspace_root: &std::path::Path, app_id: &str) -> String {
    let app_root = resolve_app_root(workspace_root, app_id);
    format!(
        "theme:{}",
        mei_lang_kernel::load_cache_generation(app_root.as_path(), app_id).updated_at_ms
    )
}

pub(crate) fn build_scene_view_manifest(
    workspace_root: &std::path::Path,
    app_id: &str,
    scene_id: &str,
    data_mode: DataMode,
    review_projection: &str,
    tab: &str,
    chrome: &str,
    draft_session: &str,
    draft_digest: &str,
    hits: &mut ArtifactHitMatrix,
) -> anyhow::Result<mei_host_graph::SceneViewManifest> {
    let semantic_core = mei_host_graph::build_semantic_core_for_scene(
        workspace_root,
        app_id,
        scene_id,
    );
    let layout_rev = layout_policy_revision(workspace_root, app_id);
    let structure_key =
        mei_host_graph::structure_full_cache_key(&semantic_core, layout_rev.as_str());

    let structure_ref = if let Some(bytes) = mei_host_graph::take_layer(structure_key.as_str()) {
        hits.structure_hit = true;
        let hash = mei_host_graph::content_hash_bytes(bytes.as_slice());
        mei_host_graph::LayerRef {
            artifact_id: structure_key.clone(),
            content_hash: hash,
            bytes: Some(bytes.len() as u64),
            encoding: Some("json".to_string()),
        }
    } else {
        let outcome = mei_host_graph::assemble_scope_from_registry(
            workspace_root,
            app_id,
            scene_id,
        )?
        .ok_or_else(|| anyhow::anyhow!("assemble unavailable"))?;
        let (_doc, pref, key) = mei_host_graph::structure_full_from_compiled(
            workspace_root,
            &outcome.compiled,
            &semantic_core,
            layout_rev.as_str(),
        )?;
        let bytes = serde_json::to_vec(&mei_host_graph::build_structure_full_document(
            &outcome.compiled,
            key.as_str(),
        ))?;
        mei_host_graph::store_layer(
            structure_key.clone(),
            mei_host_graph::STRUCTURE_FULL_KIND,
            pref.content_hash.as_str(),
            bytes.as_slice(),
        );
        hits.structure_hit = false;
        mei_host_graph::LayerRef {
            artifact_id: structure_key,
            content_hash: pref.content_hash,
            bytes: Some(bytes.len() as u64),
            encoding: Some("json".to_string()),
        }
    };

    let theme_key = mei_host_graph::theme_tokens_cache_key(theme_digest(workspace_root, app_id).as_str());
    hits.theme_hit = mei_host_graph::take_layer(theme_key.as_str()).is_some();

    let overlay_persisted_key =
        mei_host_graph::layout_overlay_persisted_cache_key(layout_rev.as_str());
    hits.overlay_hit = mei_host_graph::take_layer(overlay_persisted_key.as_str()).is_some()
        || (!draft_digest.is_empty()
            && mei_host_graph::take_layer(
                mei_host_graph::layout_overlay_session_cache_key(
                    app_id,
                    draft_session,
                    draft_digest,
                )
                .as_str(),
            )
            .is_some());

    let eval_key = mei_host_graph::eval_slot_group_cache_key(
        &semantic_core,
        "scene:default",
        data_mode.slug(),
        "default",
    );
    hits.eval_hit = mei_host_graph::take_layer(eval_key.as_str()).is_some();

    let shell_key = mei_host_graph::shell_cache_key(
        UiRouteMode::Build.slug(),
        tab,
        chrome,
        None,
        "shell-v1",
    );
    hits.shell_hit = mei_host_graph::take_layer(shell_key.as_str()).is_some();

    let mut layers = std::collections::BTreeMap::new();
    layers.insert("structure.full".to_string(), json!(structure_ref));
    layers.insert(
        "eval.slot_group.scene:default".to_string(),
        json!({
            "artifact_id": eval_key,
            "data_mode": data_mode.slug(),
        }),
    );
    layers.insert(
        "theme.tokens".to_string(),
        json!({ "artifact_id": theme_key }),
    );
    layers.insert(
        "layout.overlay".to_string(),
        json!({
            "persisted": overlay_persisted_key,
            "session": if draft_digest.is_empty() {
                Value::Null
            } else {
                json!(mei_host_graph::layout_overlay_session_cache_key(
                    app_id,
                    draft_session,
                    draft_digest,
                ))
            },
        }),
    );
    layers.insert(
        "shell.build".to_string(),
        json!({ "artifact_id": shell_key }),
    );

    let manifest = mei_host_graph::SceneViewManifest {
        schema_version: mei_host_graph::SCENE_VIEW_MANIFEST_SCHEMA.to_string(),
        app_id: app_id.to_string(),
        scene_id: scene_id.to_string(),
        semantic_core,
        revision_digest: String::new(),
        layers,
        compose_defaults: Some(mei_host_graph::ComposeRequest {
            route_mode: Some(UiRouteMode::Build.slug().to_string()),
            tab: Some(tab.to_string()),
            chrome: Some(chrome.to_string()),
            review_projection: Some(review_projection.to_string()),
            data_mode: Some(data_mode.slug().to_string()),
            focus: None,
            scope: None,
        }),
    };
    let digest = mei_host_graph::manifest_revision_digest(&manifest);
    Ok(mei_host_graph::SceneViewManifest {
        revision_digest: digest,
        ..manifest
    })
}

pub async fn api_host_scene_manifest(
    State(state): State<SharedState>,
    State(_auth): State<AuthServeState>,
    headers: HeaderMap,
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
    let axes = resolve_page_render_axes(
        &guard,
        &AppQuery {
            data_mode: query.data_mode.clone(),
            review_projection: query.review_projection.clone(),
            ..Default::default()
        },
        UiRouteMode::Build,
    );
    let draft_session = mei_host_core::resolve_draft_session_id(&headers);
    let draft_digest = crate::build_layout_tuning::build_session_layout_tuning_draft(
        workspace_root,
        app_id,
        mei_host_core::layout_tuning_draft_storage_key(app_id, draft_session.as_str()).as_str(),
    )
    .as_ref()
    .map(|draft| crate::build_fragment_cache::draft_digest_for_tuning(Some(draft)))
    .unwrap_or_default();

    let mut hits = ArtifactHitMatrix::default();
    let manifest = match build_scene_view_manifest(
        workspace_root,
        app_id,
        scene_id.as_str(),
        axes.data_mode,
        crate::review_axes::ssr_review_projection(UiRouteMode::Build, axes.data_mode).slug(),
        query.tab.as_deref().unwrap_or("scene"),
        query.chrome.as_deref().unwrap_or("full"),
        draft_session.as_str(),
        draft_digest.as_str(),
        &mut hits,
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

pub async fn api_host_layer_batch(
    State(state): State<SharedState>,
    State(_auth): State<AuthServeState>,
    headers: HeaderMap,
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
    let axes = resolve_page_render_axes(
        &guard,
        &AppQuery {
            data_mode: body.data_mode.clone(),
            ..Default::default()
        },
        UiRouteMode::Build,
    );
    let draft_session = mei_host_core::resolve_draft_session_id(&headers);
    let draft_digest = crate::build_layout_tuning::build_session_layout_tuning_draft(
        workspace_root,
        app_id,
        mei_host_core::layout_tuning_draft_storage_key(app_id, draft_session.as_str()).as_str(),
    )
    .as_ref()
    .map(|draft| crate::build_fragment_cache::draft_digest_for_tuning(Some(draft)))
    .unwrap_or_default();
    let layout_rev = layout_policy_revision(workspace_root, app_id);
    let semantic_core =
        mei_host_graph::build_semantic_core_for_scene(workspace_root, app_id, scene_id.as_str());
    let structure_key =
        mei_host_graph::structure_full_cache_key(&semantic_core, layout_rev.as_str());

    let mut hits = ArtifactHitMatrix::default();
    let mut layers = std::collections::BTreeMap::new();
    for layer in body.layers {
        match layer.as_str() {
            "structure.full" => {
                if let Some(bytes) = mei_host_graph::take_layer(structure_key.as_str()) {
                    hits.structure_hit = true;
                    layers.insert(
                        layer,
                        serde_json::from_slice(bytes.as_slice()).unwrap_or(Value::Null),
                    );
                } else if let Ok(Some(outcome)) = mei_host_graph::assemble_scope_from_registry(
                    workspace_root,
                    app_id,
                    scene_id.as_str(),
                ) {
                    let (doc, pref, _) = mei_host_graph::structure_full_from_compiled(
                        workspace_root,
                        &outcome.compiled,
                        &semantic_core,
                        layout_rev.as_str(),
                    )
                    .unwrap_or_else(|_| {
                        (
                            mei_host_graph::build_structure_full_document(
                                &outcome.compiled,
                                structure_key.as_str(),
                            ),
                            mei_host_graph::PayloadRef::new(
                                mei_host_graph::STRUCTURE_FULL_KIND,
                                "missing",
                                mei_host_graph::STRUCTURE_FULL_SCHEMA,
                            ),
                            structure_key.clone(),
                        )
                    });
                    let bytes = serde_json::to_vec(&doc).unwrap_or_default();
                    mei_host_graph::store_layer(
                        structure_key.clone(),
                        mei_host_graph::STRUCTURE_FULL_KIND,
                        pref.content_hash.as_str(),
                        bytes.as_slice(),
                    );
                    layers.insert(layer, serde_json::to_value(doc).unwrap_or(Value::Null));
                }
            }
            "theme.tokens" => {
                let key = mei_host_graph::theme_tokens_cache_key(
                    theme_digest(workspace_root, app_id).as_str(),
                );
                hits.theme_hit = mei_host_graph::take_layer(key.as_str()).is_some();
                layers.insert(layer, json!({ "artifact_id": key }));
            }
            "layout.overlay" => {
                let persisted =
                    mei_host_graph::layout_overlay_persisted_cache_key(layout_rev.as_str());
                hits.overlay_hit = mei_host_graph::take_layer(persisted.as_str()).is_some();
                layers.insert(
                    layer,
                    json!({
                        "persisted": persisted,
                        "session": if draft_digest.is_empty() {
                            Value::Null
                        } else {
                            json!(mei_host_graph::layout_overlay_session_cache_key(
                                app_id,
                                draft_session.as_str(),
                                draft_digest.as_str(),
                            ))
                        },
                    }),
                );
            }
            name if name.starts_with("eval.slot_group.") => {
                let slot_group_id = name
                    .strip_prefix("eval.slot_group.")
                    .unwrap_or("scene:default");
                let eval_key = mei_host_graph::eval_slot_group_cache_key(
                    &semantic_core,
                    slot_group_id,
                    axes.data_mode.slug(),
                    "default",
                );
                hits.eval_hit = mei_host_graph::take_layer(eval_key.as_str()).is_some();
                layers.insert(layer, json!({ "artifact_id": eval_key }));
            }
            _ => {
                layers.insert(layer, Value::Null);
            }
        }
    }

    let obs = LayerArtifactObservability { hits };
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
    response
}
