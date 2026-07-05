//! Page render axes: data mode + review projection (0508).

use mei_lang_app::UiRouteMode;
use mei_lang_kernel::{DataMode, DataModeCeiling, ReviewProjection};

use crate::pages::AppQuery;
use crate::state::ShellState;

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

pub fn resolve_page_render_axes(shell: &ShellState, query: &AppQuery, route_mode: UiRouteMode) -> PageRenderAxes {
    resolve_page_render_axes_detailed(shell, query, route_mode).axes
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
    let from_query = query
        .data_mode
        .as_deref()
        .and_then(DataMode::parse);
    let requested = from_query.unwrap_or_else(|| default_data_mode_for_route(route_mode, ceiling));
    let data_mode = DataMode::clamp_to_ceiling(requested, ceiling).unwrap_or(DataMode::Static);
    let review_projection =
        resolve_client_review_projection(route_mode, data_mode, query.review_projection.as_deref());
    PageRenderAxesResolution {
        axes: PageRenderAxes {
            data_mode,
            review_projection,
        },
        requested_data_mode: from_query,
        data_mode_clamped: data_mode != requested,
    }
}

/// Client-side projection depth (URL / dim chrome). App mode ignores review_projection query params.
pub fn resolve_client_review_projection(
    route_mode: UiRouteMode,
    data_mode: DataMode,
    query_projection: Option<&str>,
) -> ReviewProjection {
    if route_mode == UiRouteMode::App {
        return default_projection_for_route(route_mode, data_mode);
    }
    query_projection
        .and_then(ReviewProjection::parse)
        .unwrap_or_else(|| default_projection_for_route(route_mode, data_mode))
}

/// SSR + host page-render-cache always materialize the fullest scene DOM for the data mode.
pub fn ssr_review_projection(route_mode: UiRouteMode, data_mode: DataMode) -> ReviewProjection {
    if route_mode == UiRouteMode::App || route_mode == UiRouteMode::Build {
        return canonical_full_projection_for_data_mode(data_mode);
    }
    default_projection_for_route(route_mode, data_mode)
}

pub fn ssr_review_projection_for_axes(
    route_mode: UiRouteMode,
    axes: PageRenderAxes,
) -> ReviewProjection {
    if route_mode == UiRouteMode::Run || route_mode == UiRouteMode::Copilot {
        axes.review_projection
    } else {
        ssr_review_projection(route_mode, axes.data_mode)
    }
}

fn canonical_full_projection_for_data_mode(data_mode: DataMode) -> ReviewProjection {
    match data_mode {
        DataMode::Static => ReviewProjection::StaticFull,
        _ => ReviewProjection::LiveFull,
    }
}

pub fn default_page_render_axes_for_route(
    route_mode: UiRouteMode,
    ceiling: DataModeCeiling,
) -> PageRenderAxes {
    let data_mode = default_data_mode_for_route(route_mode, ceiling);
    PageRenderAxes {
        data_mode,
        review_projection: default_projection_for_route(route_mode, data_mode),
    }
}

fn default_data_mode_for_route(route_mode: UiRouteMode, ceiling: DataModeCeiling) -> DataMode {
    if route_mode == UiRouteMode::Build && ceiling != DataModeCeiling::Eval {
        ceiling.as_data_mode()
    } else {
        DataMode::Eval
    }
}

fn default_projection_for_route(route_mode: UiRouteMode, data_mode: DataMode) -> ReviewProjection {
    if route_mode == UiRouteMode::Build {
        ReviewProjection::PlaneRegionSection
    } else if data_mode == DataMode::Eval {
        ReviewProjection::LiveFull
    } else {
        ReviewProjection::StaticFull
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
    fn app_warmup_axes_use_live_full_not_static_full() {
        let axes = default_page_render_axes_for_route(UiRouteMode::App, DataModeCeiling::Eval);
        assert_eq!(axes.data_mode, DataMode::Eval);
        assert_eq!(axes.review_projection, ReviewProjection::LiveFull);
    }

    #[test]
    fn build_default_axes_use_plane_region_section() {
        let axes = default_page_render_axes_for_route(UiRouteMode::Build, DataModeCeiling::Eval);
        assert_eq!(axes.review_projection, ReviewProjection::PlaneRegionSection);
    }

    #[test]
    fn app_mode_ignores_review_projection_query() {
        let query = AppQuery {
            review_projection: Some("plane_region".to_string()),
            ..Default::default()
        };
        let axes = resolve_page_render_axes_with_ceiling_detailed(
            DataModeCeiling::Eval,
            &query,
            UiRouteMode::App,
        )
        .axes;
        assert_eq!(axes.review_projection, ReviewProjection::LiveFull);
    }

    #[test]
    fn build_ssr_projection_is_live_full_for_eval() {
        assert_eq!(
            ssr_review_projection(UiRouteMode::Build, DataMode::Eval),
            ReviewProjection::LiveFull
        );
    }
}
