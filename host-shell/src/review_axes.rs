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
    resolve_page_render_axes_with_ceiling(shell.data_mode_ceiling, query, route_mode)
}

pub fn resolve_page_render_axes_with_ceiling(
    ceiling: DataModeCeiling,
    query: &AppQuery,
    route_mode: UiRouteMode,
) -> PageRenderAxes {
    let requested = query
        .data_mode
        .as_deref()
        .and_then(DataMode::parse)
        .unwrap_or_else(|| default_data_mode_for_route(route_mode, ceiling));
    let data_mode = DataMode::clamp_to_ceiling(requested, ceiling).unwrap_or(DataMode::Static);
    let review_projection = query
        .review_projection
        .as_deref()
        .and_then(ReviewProjection::parse)
        .unwrap_or_else(|| default_projection_for_route(route_mode, data_mode));
    PageRenderAxes {
        data_mode,
        review_projection,
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
        let shell = ShellState::new(
            PathBuf::from("/tmp"),
            "app".to_string(),
            PathBuf::from("/pkg"),
            Default::default(),
            false,
        );
        let mut shell = shell;
        shell.data_mode_ceiling = DataModeCeiling::Static;
        let query = AppQuery {
            data_mode: Some("eval".to_string()),
            ..Default::default()
        };
        let axes = resolve_page_render_axes(&shell, &query, UiRouteMode::App);
        assert_eq!(axes.data_mode, DataMode::Static);
    }
}
