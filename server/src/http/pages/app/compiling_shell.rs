use mei_lang_app::UiRouteMode;

use crate::http::pages::app::query::AppQuery;

pub(crate) const COMPILE_BOOTSTRAP_PROBE_DIAG_FILTER: &str = "__mei_compile_probe__";

pub(crate) fn compile_bootstrap_probe_requested(query: &AppQuery) -> bool {
    query
        .diag_filter
        .as_deref()
        .map(str::trim)
        .is_some_and(|value| value == COMPILE_BOOTSTRAP_PROBE_DIAG_FILTER)
}

/// Routes that may serve [`render_compiling_shell`] must also handle compile-bootstrap probes.
pub(crate) fn compile_bootstrap_route_supported(route_mode: UiRouteMode) -> bool {
    matches!(
        route_mode,
        UiRouteMode::Build | UiRouteMode::App | UiRouteMode::Presentation
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::pages::app::query::AppQuery;

    #[test]
    fn compile_bootstrap_route_support_matches_access_like_modes() {
        assert!(compile_bootstrap_route_supported(UiRouteMode::Build));
        assert!(compile_bootstrap_route_supported(UiRouteMode::App));
        assert!(compile_bootstrap_route_supported(UiRouteMode::Presentation));
        assert!(!compile_bootstrap_route_supported(UiRouteMode::Config));
        assert!(!compile_bootstrap_route_supported(UiRouteMode::Upload));
    }

    #[test]
    fn compile_bootstrap_probe_flag_is_query_scoped() {
        let probe = AppQuery {
            file: None,
            scene: None,
            tab: None,
            diag_filter: Some(COMPILE_BOOTSTRAP_PROBE_DIAG_FILTER.to_string()),
            world_metric: None,
            world_dataset: None,
            explain: None,
            chrome: None,
        };
        assert!(compile_bootstrap_probe_requested(&probe));
    }
}
