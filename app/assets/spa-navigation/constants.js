  const RELOAD_APP_SCRIPTS = new Set([
    "/app-assets/frame-stage.js",
    "/app-assets/statusbar.js",
    "/app-assets/manage-tabs.js",
    "/app-assets/manage-diagnostics.js",
    "/app-assets/workspace-splitters.js",
    "/app-assets/source-tree-controls.js",
    "/app-assets/source-highlight.js",
    "/app-assets/agent-panel-utils.js",
    "/app-assets/agent-panel-routing.js",
    "/app-assets/agent-panel-access-float.js",
    "/app-assets/agent-panel-source.js",
    "/app-assets/agent-panel-session.js",
    "/app-assets/agent-panel-context.js",
    "/app-assets/agent-panel-chrome.js",
    "/app-assets/agent-panel-messages-model.js",
    "/app-assets/agent-panel-messages.js",
    "/app-assets/agent-panel-layout.js",
    "/app-assets/agent-panel-delta-debug.js",
    "/app-assets/agent-panel-bindings.js",
    "/app-assets/agent-panel.js",
  ]);
  const RELOAD_BUNDLE_SCRIPTS = new Set([
    "/app-bundles/manage.js",
    "/app-bundles/access.js",
  ]);
  const SPA_NAV_SCRIPT = "/app-assets/spa-navigation.js";
  const LOADING_DELAY_MS = 140;
  const LOADING_MIN_VISIBLE_MS = 180;
  const SCRIPT_LOAD_TIMEOUT_MS = 15000;
  const SPA_FETCH_TIMEOUT_MS = 120000;
  const METRIC_DRILLDOWN_EVENT = "mei:metric-drilldown";
  const ANALYSIS_OPEN_EVENT = "mei:analysis-open";
  const POPUP_OPEN_EVENT = "mei:popup-open";
  const PREFETCH_PANEL_METRICS_EVENT = "meilang:prefetch-panel-metrics";
  const DRILLDOWN_OVERLAY_ROOT_ID = "mei-access-drilldown-overlay";
  const DRILLDOWN_CONTEXT_BANNER_ID = "mei-drilldown-context-banner";

