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
    let review_projection = query
        .review_projection
        .as_deref()
        .and_then(ReviewProjection::parse)
        .unwrap_or_else(|| default_projection_for_route(route_mode, data_mode));
    PageRenderAxesResolution {
        axes: PageRenderAxes {
            data_mode,
            review_projection,
        },
        requested_data_mode: from_query,
        data_mode_clamped: data_mode != requested,
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
    use std::path::PathBuf;

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
}
