//! Page render axes: data mode + review projection (0508).
//! Access compose defaults follow stage_kind (scene vs presentation), not layout/prototype surface.

use mei_lang_app::UiRouteMode;
use mei_lang_kernel::{DataMode, DataModeCeiling, ReviewProjection};

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

    /// Infer from route kind / target_file (same rules as Access topbar).
    pub fn from_route_meta(kind: &str, target_file: &str) -> Self {
        let kind = kind.trim().to_ascii_lowercase();
        if kind == "presentation" {
            return Self::Presentation;
        }
        let target = target_file.replace('\\', "/").to_ascii_lowercase();
        if target.contains("/presentation/") || target.starts_with("presentation/") {
            Self::Presentation
        } else {
            Self::Scene
        }
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
}
