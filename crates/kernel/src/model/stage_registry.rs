//! Phase 1 additive Stage identity (0104 / 0105 §6).
//!
//! Product Stage identity is separate from spatial Scene modules and T2 pages.
//! Legacy `CompiledSceneRoute` / `scene_id` remain the wire-compatible view.

use serde::{Deserialize, Serialize};

use super::compile_out::CompiledSceneRoute;

/// App-scoped navigable product Stage id (newtype over the legacy scene_id string).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StageId(pub String);

impl StageId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl From<&str> for StageId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for StageId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl std::fmt::Display for StageId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Stage Profile (cockpit / slides / page).
///
/// `page` is a product Stage for content-driven long documents.
/// It is distinct from T2 `page_instance` (cockpit-internal drilldown), which
/// still uses route kind `"page"` and is excluded from the Stage Registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StageProfile {
    Cockpit,
    Slides,
    /// Content-driven long page: inline constrained, block intrinsic, stage viewport scrolls.
    Page,
}

impl StageProfile {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cockpit => "cockpit",
            Self::Slides => "slides",
            Self::Page => "page",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "cockpit" => Some(Self::Cockpit),
            "slides" | "presentation" => Some(Self::Slides),
            "page" => Some(Self::Page),
            _ => None,
        }
    }

    /// Infer from legacy route kind / target_file (same rules as Access StageKind).
    ///
    /// Explicit frontmatter `profile: page` is applied by Stage Program discovery;
    /// path inference alone cannot distinguish page from cockpit scene stages.
    pub fn from_route_meta(kind: &str, target_file: &str) -> Self {
        let kind = kind.trim().to_ascii_lowercase();
        if kind == "presentation" {
            return Self::Slides;
        }
        if kind == "document" {
            return Self::Page;
        }
        let target = target_file.replace('\\', "/").to_ascii_lowercase();
        if target.contains("/presentation/")
            || target.starts_with("presentation/")
            || target.ends_with(".deck.mdx")
            || target.ends_with(".presentation.mdx")
        {
            Self::Slides
        } else {
            Self::Cockpit
        }
    }
}

impl std::fmt::Display for StageProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One registered product Stage.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StageDescriptor {
    pub id: StageId,
    pub profile: StageProfile,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub short_title: Option<String>,
    /// Source file relative to app root (legacy `target_file`).
    pub source_anchor: String,
    #[serde(default)]
    pub is_default: bool,
    /// Legacy scene_id alias (same string as `id`). Phase 9: not written to new artifacts.
    #[serde(default, skip_serializing)]
    pub legacy_scene_id: String,
}

/// Route summary derived from a StageDescriptor (additive; not a wire replacement yet).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StageRoute {
    pub stage_id: StageId,
    pub profile: StageProfile,
    pub target_file: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub short_title: Option<String>,
    #[serde(default)]
    pub is_default: bool,
    /// Legacy scene_id for URL / artifact compatibility. Phase 9: not written.
    #[serde(default, skip_serializing)]
    pub legacy_scene_id: String,
}

impl From<&StageDescriptor> for StageRoute {
    fn from(desc: &StageDescriptor) -> Self {
        Self {
            stage_id: desc.id.clone(),
            profile: desc.profile,
            target_file: desc.source_anchor.clone(),
            title: desc.title.clone(),
            short_title: desc.short_title.clone(),
            is_default: desc.is_default,
            legacy_scene_id: desc.legacy_scene_id.clone(),
        }
    }
}

/// App-level ordered Stage list (product identity registry).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct StageRegistry {
    #[serde(default)]
    pub stages: Vec<StageDescriptor>,
    #[serde(default)]
    pub default_stage_id: Option<StageId>,
}

impl StageRegistry {
    pub fn is_empty(&self) -> bool {
        self.stages.is_empty()
    }

    pub fn get(&self, id: &str) -> Option<&StageDescriptor> {
        self.stages.iter().find(|s| s.id.as_str() == id)
    }

    pub fn contains(&self, id: &str) -> bool {
        self.get(id).is_some()
    }

    pub fn stage_ids(&self) -> Vec<&str> {
        self.stages.iter().map(|s| s.id.as_str()).collect()
    }

    pub fn routes(&self) -> Vec<StageRoute> {
        self.stages.iter().map(StageRoute::from).collect()
    }

    /// Build from legacy compiled routes, excluding T2/page/board entries.
    pub fn from_compiled_routes(routes: &[CompiledSceneRoute]) -> Self {
        let mut stages = Vec::new();
        for route in routes {
            if !is_stage_registry_candidate(route) {
                continue;
            }
            let id = route.scene_id.trim();
            if id.is_empty() {
                continue;
            }
            if stages.iter().any(|s: &StageDescriptor| s.id.as_str() == id) {
                continue;
            }
            let profile =
                StageProfile::from_route_meta(route.kind.as_str(), route.target_file.as_str());
            stages.push(StageDescriptor {
                id: StageId::new(id),
                profile,
                title: route.title.clone(),
                short_title: route.short_title.clone(),
                source_anchor: route.target_file.replace('\\', "/"),
                is_default: route.is_default,
                legacy_scene_id: id.to_string(),
            });
        }

        let mut default_stage_id = stages.iter().find(|s| s.is_default).map(|s| s.id.clone());
        if default_stage_id.is_none() {
            default_stage_id = stages.first().map(|s| s.id.clone());
            if let Some(default_id) = default_stage_id.as_ref() {
                for stage in &mut stages {
                    stage.is_default = stage.id == *default_id;
                }
            }
        }

        // Preserve navigation route order (Gate 1: topbar behavior unchanged).

        Self {
            stages,
            default_stage_id,
        }
    }
}

/// Whether a legacy route is a product Stage (not T2 page / board / internal).
///
/// Mirrors Access topbar `is_top_level_stage_route` so Registry and chrome stay aligned.
pub fn is_stage_registry_candidate(route: &CompiledSceneRoute) -> bool {
    if !route.access_export {
        return false;
    }
    let kind = route.kind.trim().to_ascii_lowercase();
    if kind == "page" || kind == "board" || kind == "scene_first_board" || kind == "board_capsule" {
        return false;
    }
    let target = route.target_file.replace('\\', "/").to_ascii_lowercase();
    // Legacy central t2/ plus colocated T2 page-planes (`.../plane-{id}/...` per 025004).
    if target.contains("/t2/") || target.contains("/overlay/") || target.contains("/plane-") {
        return false;
    }
    kind == "scene"
        || kind == "presentation"
        || kind == "document"
        || kind == "file_ref"
        || kind == "declarative"
        || kind.is_empty()
        || target.contains("/presentation/")
        || target.contains("/scene/")
        || target.ends_with(".deck.mdx")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn route(
        scene_id: &str,
        kind: &str,
        target: &str,
        access_export: bool,
        is_default: bool,
    ) -> CompiledSceneRoute {
        CompiledSceneRoute {
            scene_id: scene_id.to_string(),
            frame_id: None,
            target_file: target.to_string(),
            kind: kind.to_string(),
            title: None,
            short_title: None,
            is_default,
            access_export,
        }
    }

    #[test]
    fn registry_excludes_t2_pages_keeps_cockpit_and_slides() {
        let routes = vec![
            route(
                "warnings_analytics_page",
                "scene",
                "src/scene/home/t2/r-warnings/c-warnings-analytics/content.mei",
                true,
                false,
            ),
            route(
                "park_point_1_page",
                "scene",
                "src/scene/home/t1/region-left-rail/section-lake-pavilion/plane-park-point-1/plane.mei",
                true,
                false,
            ),
            route("home", "scene", "src/scene/home.mei", true, true),
            route(
                "supervision",
                "presentation",
                "src/presentation/supervision/supervision.deck.mdx",
                true,
                false,
            ),
            route(
                "t2_page",
                "page",
                "src/scene/home/t2/r-x/content.mei",
                false,
                false,
            ),
        ];
        let registry = StageRegistry::from_compiled_routes(&routes);
        assert_eq!(registry.stage_ids(), vec!["home", "supervision"]);
        assert_eq!(
            registry.get("home").map(|s| s.profile),
            Some(StageProfile::Cockpit)
        );
        assert_eq!(
            registry.get("supervision").map(|s| s.profile),
            Some(StageProfile::Slides)
        );
        assert_eq!(
            registry.default_stage_id.as_ref().map(StageId::as_str),
            Some("home")
        );
    }

    #[test]
    fn slides_inferred_from_deck_path() {
        assert_eq!(
            StageProfile::from_route_meta("scene", "src/presentation/intro/intro.deck.mdx"),
            StageProfile::Slides
        );
    }
}
