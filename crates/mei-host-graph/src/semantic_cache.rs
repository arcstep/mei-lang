//! MCG/MRG/bootstrap semantic cache key (view-agnostic compile/assemble chain).

use serde::Serialize;

/// True-source dimensions shared by Build / App / Run / shell-less on the same scene scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PageRenderViewAxes {
    pub route_mode: String,
    pub data_mode: String,
    pub review_projection: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_sig: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overlay_revision: Option<String>,
}

pub fn semantic_cache_core_signature(core: &SemanticCacheCore) -> Option<String> {
    serde_json::to_string(core).ok()
}

pub fn page_render_view_signature(view: &PageRenderViewAxes) -> Option<String> {
    serde_json::to_string(view).ok()
}
