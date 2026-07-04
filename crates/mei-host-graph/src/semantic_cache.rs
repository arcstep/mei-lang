//! MCG/MRG/bootstrap semantic cache key (view-agnostic compile/assemble chain).

use serde::{Deserialize, Serialize};

/// True-source dimensions shared by Build / App / Run / shell-less on the same scene scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticCacheCore {
    pub app_id: String,
    pub scene_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview_scope: Option<String>,
    pub registry_revision: String,
    pub client_revision: String,
    pub data_generation: String,
    pub compile_epoch: String,
}

/// View-derived dimensions that must not split MCG/MRG/bootstrap assembly keys.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageRenderViewAxes {
    pub route_mode: String,
    pub data_mode: String,
    pub review_projection: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_sig: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overlay_revision: Option<String>,
}

pub fn build_semantic_cache_core(
    app_id: impl Into<String>,
    scene_id: impl Into<String>,
    preview_scope: Option<String>,
    registry_revision: impl Into<String>,
    client_revision: impl Into<String>,
    data_generation: impl Into<String>,
    compile_epoch: impl Into<String>,
) -> SemanticCacheCore {
    SemanticCacheCore {
        app_id: app_id.into(),
        scene_id: scene_id.into(),
        preview_scope,
        registry_revision: registry_revision.into(),
        client_revision: client_revision.into(),
        data_generation: data_generation.into(),
        compile_epoch: compile_epoch.into(),
    }
}

pub fn build_page_render_view_axes(
    route_mode: &str,
    data_mode: &str,
    review_projection: &str,
    auth_sig: Option<u64>,
    overlay_revision: Option<String>,
) -> PageRenderViewAxes {
    PageRenderViewAxes {
        route_mode: route_mode.to_string(),
        data_mode: data_mode.to_string(),
        review_projection: review_projection.to_string(),
        auth_sig,
        overlay_revision,
    }
}

pub fn semantic_cache_core_signature(core: &SemanticCacheCore) -> Option<String> {
    serde_json::to_string(core).ok()
}

pub fn page_render_view_signature(view: &PageRenderViewAxes) -> Option<String> {
    serde_json::to_string(view).ok()
}
