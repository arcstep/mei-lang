//! Page render axes: data mode + review projection (0508).
//! Access compose defaults follow stage_kind (scene vs presentation), not layout/prototype surface.
//! Phase 1: StageKind maps to StageProfile (Scene→Cockpit, Presentation→Slides); not a cache key.

use mei_lang_app::UiRouteMode;
use mei_lang_kernel::{
    DataMode, DataModeCeiling, ReviewProjection, StageProfile, StageRegistry,
};

use crate::pages::AppQuery;
use crate::state::ShellState;

/// Access stage kind used for compose defaults (not a cache key dimension).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageKind {
    Scene,
    Presentation,
}

impl StageKind {
    #[allow(dead_code)] // public slug API for future query/debug surfaces
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Scene => "scene",
            Self::Presentation => "presentation",
        }
    }

    #[allow(dead_code)] // public parser for future query/debug surfaces
    pub fn parse(raw: Option<&str>) -> Self {
        match raw.map(str::trim).map(|v| v.to_ascii_lowercase()) {
            Some(value) if value == "presentation" => Self::Presentation,
            _ => Self::Scene,
        }
    }

    /// Map host StageKind → product StageProfile (Phase 1).
    pub fn to_stage_profile(self) -> StageProfile {
        match self {
            Self::Scene => StageProfile::Cockpit,
            Self::Presentation => StageProfile::Slides,
        }
    }

    /// Map product StageProfile → host StageKind.
    pub fn from_stage_profile(profile: StageProfile) -> Self {
        match profile {
            StageProfile::Cockpit => Self::Scene,
            StageProfile::Slides => Self::Presentation,
        }
    }

    /// Infer from route kind / target_file (same rules as StageProfile / Access topbar).
    pub fn from_route_meta(kind: &str, target_file: &str) -> Self {
        let profile = StageProfile::from_route_meta(kind, target_file);
        let kind = Self::from_stage_profile(profile);
        debug_assert_eq!(kind.to_stage_profile(), profile);
        kind
    }

    /// Resolve from compiled scene routes for `scene_id`; missing route → Scene.
    pub fn from_scene_routes(
        routes: &[mei_lang_kernel::CompiledSceneRoute],
        scene_id: &str,
    ) -> Self {
        routes
            .iter()
            .find(|route| route.scene_id == scene_id)
            .map(|route| Self::from_route_meta(route.kind.as_str(), route.target_file.as_str()))
            .unwrap_or(Self::Scene)
    }

    /// Prefer StageRegistry when available; fall back to Scene for unknown ids.
    pub fn from_stage_registry(registry: &StageRegistry, stage_id: &str) -> Self {
        registry
            .get(stage_id)
            .map(|desc| Self::from_stage_profile(desc.profile))
            .unwrap_or(Self::Scene)
    }

    /// Resolve stage kind preferring Registry, then legacy routes.
    pub fn resolve(
        registry: &StageRegistry,
        routes: &[mei_lang_kernel::CompiledSceneRoute],
        stage_or_scene_id: &str,
    ) -> Self {
        if registry.contains(stage_or_scene_id) {
            return Self::from_stage_registry(registry, stage_or_scene_id);
        }
        Self::from_scene_routes(routes, stage_or_scene_id)
    }
}

/// Read Phase 2 StageProgram for a stage_id (diagnostic / Inspector). Not a cache key.
pub fn stage_program_for<'a>(
    compiled: &'a mei_lang_kernel::CompiledApp,
    stage_id: &str,
) -> Option<&'a mei_lang_kernel::StageProgram> {
    compiled.stage_programs.get(stage_id)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageRenderAxes {
    pub data_mode: DataMode,
    pub review_projection: ReviewProjection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageRenderAxesResolution {
    pub axes: PageRenderAxes,
    pub requested_data_mode: Option<DataMode>,
    pub data_mode_clamped: bool,
}

impl Default for PageRenderAxes {
    fn default() -> Self {
        Self {
            data_mode: DataMode::Eval,
            review_projection: ReviewProjection::StaticFull,
        }
    }
}

pub fn parse_data_mode_ceiling_arg(value: &str) -> Result<DataModeCeiling, String> {
    DataModeCeiling::parse(value).ok_or_else(|| {
        format!("invalid data mode ceiling `{value}` (expected eval|fixture|static)")
    })
}

pub fn resolve_page_render_axes(
    shell: &ShellState,
    query: &AppQuery,
    route_mode: UiRouteMode,
) -> PageRenderAxes {
    resolve_page_render_axes_detailed(shell, query, route_mode).axes
}

pub fn resolve_page_render_axes_for_stage(
    shell: &ShellState,
    query: &AppQuery,
    route_mode: UiRouteMode,
    stage_kind: StageKind,
) -> PageRenderAxes {
    resolve_page_render_axes_with_ceiling_detailed_for_stage(
        shell.data_mode_ceiling,
        query,
        route_mode,
        stage_kind,
    )
    .axes
}

pub fn resolve_page_render_axes_detailed(
    shell: &ShellState,
    query: &AppQuery,
    route_mode: UiRouteMode,
) -> PageRenderAxesResolution {
    resolve_page_render_axes_with_ceiling_detailed(shell.data_mode_ceiling, query, route_mode)
}

pub fn resolve_page_render_axes_with_ceiling_detailed(
    ceiling: DataModeCeiling,
    query: &AppQuery,
    route_mode: UiRouteMode,
) -> PageRenderAxesResolution {
    resolve_page_render_axes_with_ceiling_detailed_for_stage(
        ceiling,
        query,
        route_mode,
        StageKind::Scene,
    )
}

pub fn resolve_page_render_axes_with_ceiling_detailed_for_stage(
    ceiling: DataModeCeiling,
    query: &AppQuery,
    route_mode: UiRouteMode,
    stage_kind: StageKind,
) -> PageRenderAxesResolution {
    let from_query = query.data_mode.as_deref().and_then(DataMode::parse);
    let requested =
        from_query.unwrap_or_else(|| default_data_mode_for_route(route_mode, stage_kind, ceiling));
    let data_mode = DataMode::clamp_to_ceiling(requested, ceiling).unwrap_or(DataMode::Static);
    let review_projection = resolve_client_review_projection(
        route_mode,
        stage_kind,
        data_mode,
        query.review_projection.as_deref(),
    );
    PageRenderAxesResolution {
        axes: PageRenderAxes {
            data_mode,
            review_projection,
        },
        requested_data_mode: from_query,
        data_mode_clamped: data_mode != requested,
    }
}

/// Client-side projection depth. App mode ignores review_projection query; uses stage_kind defaults.
pub fn resolve_client_review_projection(
    route_mode: UiRouteMode,
    stage_kind: StageKind,
    data_mode: DataMode,
    query_projection: Option<&str>,
) -> ReviewProjection {
    if route_mode == UiRouteMode::App {
        return default_projection_for_stage(stage_kind, data_mode);
    }
    // Deprecated Layout/Prototype (mei-host-web): query may override.
    if let Some(parsed) = query_projection.and_then(ReviewProjection::parse) {
        return parsed;
    }
    default_projection_for_route(route_mode, stage_kind, data_mode)
}

/// SSR page-render-cache projection slug passed into preview runtime context.
pub fn ssr_review_projection(
    route_mode: UiRouteMode,
    stage_kind: StageKind,
    data_mode: DataMode,
) -> ReviewProjection {
    match route_mode {
        UiRouteMode::App => canonical_full_projection_for_data_mode(data_mode),
        // Deprecated surfaces: keep old SSR mapping for mei-host-web until that stack is retired.
        UiRouteMode::Layout => ReviewProjection::PlaneRegionSectionSlot,
        UiRouteMode::Prototype => ReviewProjection::StaticFull,
        _ => default_projection_for_route(route_mode, stage_kind, data_mode),
    }
}

pub fn ssr_review_projection_for_axes(
    route_mode: UiRouteMode,
    stage_kind: StageKind,
    axes: PageRenderAxes,
) -> ReviewProjection {
    match route_mode {
        UiRouteMode::Run | UiRouteMode::Copilot => axes.review_projection,
        UiRouteMode::Layout | UiRouteMode::Prototype => axes.review_projection,
        _ => ssr_review_projection(route_mode, stage_kind, axes.data_mode),
    }
}

fn canonical_full_projection_for_data_mode(data_mode: DataMode) -> ReviewProjection {
    match data_mode {
        DataMode::Static => ReviewProjection::StaticFull,
        _ => ReviewProjection::LiveFull,
    }
}

#[cfg(test)]
pub fn default_page_render_axes_for_stage(
    route_mode: UiRouteMode,
    stage_kind: StageKind,
    ceiling: DataModeCeiling,
) -> PageRenderAxes {
    let data_mode = default_data_mode_for_route(route_mode, stage_kind, ceiling);
    PageRenderAxes {
        data_mode,
        review_projection: default_projection_for_route(route_mode, stage_kind, data_mode),
    }
}

fn default_data_mode_for_route(
    route_mode: UiRouteMode,
    stage_kind: StageKind,
    _ceiling: DataModeCeiling,
) -> DataMode {
    match route_mode {
        // Deprecated: manage-shell static defaults (server may still call).
        UiRouteMode::Layout | UiRouteMode::Prototype => DataMode::Static,
        UiRouteMode::App => match stage_kind {
            StageKind::Presentation => DataMode::Static,
            StageKind::Scene => DataMode::Eval,
        },
        _ => DataMode::Eval,
    }
}

fn default_projection_for_stage(stage_kind: StageKind, data_mode: DataMode) -> ReviewProjection {
    match stage_kind {
        StageKind::Presentation => ReviewProjection::StaticFull,
        StageKind::Scene => canonical_full_projection_for_data_mode(data_mode),
    }
}

fn default_projection_for_route(
    route_mode: UiRouteMode,
    stage_kind: StageKind,
    data_mode: DataMode,
) -> ReviewProjection {
    match route_mode {
        UiRouteMode::Layout => ReviewProjection::PlaneRegionSectionSlot,
        UiRouteMode::Prototype => ReviewProjection::StaticFull,
        UiRouteMode::App => default_projection_for_stage(stage_kind, data_mode),
        _ if data_mode == DataMode::Eval => ReviewProjection::LiveFull,
        _ => ReviewProjection::StaticFull,
    }
}

pub fn access_readiness_requires_plug_ds(axes: PageRenderAxes) -> bool {
    axes.data_mode.allows_eval_api()
}

pub fn access_readiness_requires_bootstrap(axes: PageRenderAxes) -> bool {
    axes.data_mode.allows_fixture_api()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_readiness_requires_bootstrap_not_plug_ds() {
        let axes = PageRenderAxes {
            data_mode: DataMode::Fixture,
            review_projection: ReviewProjection::StaticFull,
        };
        assert!(access_readiness_requires_bootstrap(axes));
        assert!(!access_readiness_requires_plug_ds(axes));
    }

    #[test]
    fn static_ceiling_clamps_eval_request() {
        let query = AppQuery {
            data_mode: Some("eval".to_string()),
            ..Default::default()
        };
        let resolution = resolve_page_render_axes_with_ceiling_detailed(
            DataModeCeiling::Static,
            &query,
            UiRouteMode::App,
        );
        assert_eq!(resolution.axes.data_mode, DataMode::Static);
        assert!(resolution.data_mode_clamped);
    }

    #[test]
    fn scene_stage_defaults_live_full_eval() {
        let axes = default_page_render_axes_for_stage(
            UiRouteMode::App,
            StageKind::Scene,
            DataModeCeiling::Eval,
        );
        assert_eq!(axes.data_mode, DataMode::Eval);
        assert_eq!(axes.review_projection, ReviewProjection::LiveFull);
    }

    #[test]
    fn presentation_stage_defaults_static_full() {
        let axes = default_page_render_axes_for_stage(
            UiRouteMode::App,
            StageKind::Presentation,
            DataModeCeiling::Eval,
        );
        assert_eq!(axes.data_mode, DataMode::Static);
        assert_eq!(axes.review_projection, ReviewProjection::StaticFull);
    }

    #[test]
    fn app_mode_ignores_review_projection_query() {
        let query = AppQuery {
            review_projection: Some("plane_region".to_string()),
            ..Default::default()
        };
        let axes = resolve_page_render_axes_with_ceiling_detailed_for_stage(
            DataModeCeiling::Eval,
            &query,
            UiRouteMode::App,
            StageKind::Scene,
        )
        .axes;
        assert_eq!(axes.review_projection, ReviewProjection::LiveFull);
    }

    #[test]
    fn stage_kind_from_presentation_target() {
        assert_eq!(
            StageKind::from_route_meta(
                "scene",
                "src/presentation/supervision/supervision.deck.mdx"
            ),
            StageKind::Presentation
        );
        assert_eq!(
            StageKind::from_route_meta("presentation", "src/presentation/x/x.deck.mdx"),
            StageKind::Presentation
        );
        assert_eq!(
            StageKind::from_route_meta("scene", "src/scene/home.mei"),
            StageKind::Scene
        );
    }

    #[test]
    fn stage_kind_maps_to_stage_profile() {
        assert_eq!(
            StageKind::Scene.to_stage_profile(),
            mei_lang_kernel::StageProfile::Cockpit
        );
        assert_eq!(
            StageKind::Presentation.to_stage_profile(),
            mei_lang_kernel::StageProfile::Slides
        );
        assert_eq!(
            StageKind::from_stage_profile(mei_lang_kernel::StageProfile::Slides),
            StageKind::Presentation
        );
    }

    #[test]
    fn stage_kind_from_stage_registry_prefers_profile() {
        use mei_lang_kernel::{StageDescriptor, StageId, StageProfile, StageRegistry};
        let registry = StageRegistry {
            stages: vec![StageDescriptor {
                id: StageId::new("intro"),
                profile: StageProfile::Slides,
                title: None,
                source_anchor: "src/presentation/intro/intro.deck.mdx".to_string(),
                is_default: true,
                legacy_scene_id: "intro".to_string(),
            }],
            default_stage_id: Some(StageId::new("intro")),
        };
        assert_eq!(
            StageKind::from_stage_registry(&registry, "intro"),
            StageKind::Presentation
        );
        assert_eq!(
            StageKind::from_stage_registry(&registry, "missing"),
            StageKind::Scene
        );
    }
}
