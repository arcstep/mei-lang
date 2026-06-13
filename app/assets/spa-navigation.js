(() => {
  const boot = (window.__meiLangBoot = window.__meiLangBoot || {});
  if (boot.spaNavigationMounted) return;
  boot.spaNavigationMounted = true;

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

  function dispatchPreviewUpdated(scope = "page", detail = {}) {
    window.dispatchEvent(
      new CustomEvent("meilang:preview-updated", {
        detail: {
          scope: String(scope || "page"),
          ...detail,
        },
      }),
    );
  }

  function dispatchPanelMetricPrefetch() {
    window.dispatchEvent(new CustomEvent(PREFETCH_PANEL_METRICS_EVENT));
  }

  function wakeRuntimeAfterSceneBundleLoaded() {
    dispatchPreviewUpdated("page", {
      resetRuntimeQueryCache: false,
      source: "scene_bundle_ready",
    });
    requestAnimationFrame(() => {
      dispatchPanelMetricPrefetch();
    });
  }
  const DRILLDOWN_SCENE_BY_FILE = {
    "templates/cockpit/drilldown/metric-explain-board.mei": "metric_explain_board",
    "templates/cockpit/drilldown/generic-drilldown-board.mei": "generic_drilldown_board",
  };
  const BOARD_TEMPLATE_SCENE_FILES = {
    metric_board_default: "templates/cockpit/drilldown/metric-explain-board.mei",
  };
  const SCENE_LOCAL_NAV_BY_FILE = {
    "templates/cockpit/drilldown/metric-explain-board.mei": {
      sceneId: "metric_explain_board",
      kind: "metric_explain_board",
      defaultEntry: "definition",
      items: [
        { id: "hero", role: "hero", label: "概览" },
        { id: "definition", role: "explain", label: "口径" },
        { id: "composition", role: "explain", label: "构成" },
        { id: "trend", role: "explain", label: "趋势" },
        { id: "numerator_denominator", role: "explain", label: "分子分母" },
        { id: "detail", role: "table", label: "明细" },
      ],
    },
  };
  const SCENE_KIND_ORDER_FALLBACK = ["definition", "composition", "trend", "numerator_denominator", "detail"];
  const SCENE_PROJECTION_CONTEXT_KEY = "mei.scene_projection_context";

  function sceneParamRowsetDatasetId(params) {
    if (!params || typeof params !== "object" || Array.isArray(params)) return "";
    return nonEmptyString(params.rowset_dataset_id, params.rowsetDatasetId);
  }

  function inferDrilldownColumnFormats(columns) {
    const formats = {};
    (Array.isArray(columns) ? columns : []).forEach((col) => {
      const name = String(col || "").trim();
      if (!name) return;
      if (/等级/.test(name)) {
        formats[name] = { tag: true };
        return;
      }
      if (/承办部门|主责单位/.test(name)) {
        formats[name] = { truncate: true, maxChars: 14 };
        return;
      }
      if (/部门|单位|主责/.test(name)) {
        formats[name] = { truncate: true, maxChars: 18 };
        return;
      }
      if (/描述|事项|问题|表现|情况|名称|规则|依据|文件/.test(name)) {
        formats[name] = { truncate: true, maxChars: 24 };
      }
    });
    return formats;
  }

  function inferDrilldownColumnState(columns) {
    return {
      columns: (Array.isArray(columns) ? columns : []).map((key, order) => {
        const name = String(key || "").trim();
        if (!name) return { key: name, order };
        if (/等级/.test(name)) {
          return { key: name, order, width: 76, width_mode: "fixed", align: "center" };
        }
        if (/承办部门|主责单位/.test(name)) {
          return { key: name, order, align: "left" };
        }
        return { key: name, order };
      }),
    };
  }
  let currentNavigationId = 0;
  let spaNavigationInFlight = 0;
  let loadingTimer = null;
  let loadingVisibleAt = 0;
  let drilldownContextRetryTimer = null;

  // Keep in sync with `UiRouteMode::from_slug` (app/src/ui/route.rs).
  const ACCESS_LIKE_ROUTE_SLUGS = new Set([
    "app",
    "access",
    "run",
    "access-only",
    "access_only",
    "presentation",
    "slides",
  ]);
  const BUILD_ROUTE_SLUGS = new Set(["build", "manage"]);

  function appRouteSlugFromPathname(pathname = window.location.pathname) {
    const path = String(pathname || "");
    const match = path.match(/^\/apps\/([^/]+)\//);
    return match ? String(match[1] || "").trim().toLowerCase() : "";
  }

  function appRoutePrefixesFromSlugs(slugs) {
    return Array.from(slugs, (slug) => `/apps/${slug}/`);
  }

  function isAppRoute(pathname = window.location.pathname) {
    return ACCESS_LIKE_ROUTE_SLUGS.has(appRouteSlugFromPathname(pathname));
  }

  function isBuildRoute(pathname = window.location.pathname) {
    return BUILD_ROUTE_SLUGS.has(appRouteSlugFromPathname(pathname));
  }

  function isConfigRoute(pathname = window.location.pathname) {
    return String(pathname || "").startsWith("/apps/config/");
  }

  function isUploadRoute(pathname = window.location.pathname) {
    return String(pathname || "").startsWith("/apps/upload/");
  }

  function isStandaloneViewRoute(pathname = window.location.pathname) {
    return isConfigRoute(pathname) || isUploadRoute(pathname);
  }

  function isAccessRoute(pathname = window.location.pathname) {
    return isAppRoute(pathname);
  }

  function isManageRoute(pathname = window.location.pathname) {
    return isBuildRoute(pathname);
  }

  /** Preview-capable host routes: access-like scene shells + build/manage editors. */
  function shouldMountDrilldownHost(pathname = window.location.pathname) {
    const slug = appRouteSlugFromPathname(pathname);
    return ACCESS_LIKE_ROUTE_SLUGS.has(slug) || BUILD_ROUTE_SLUGS.has(slug);
  }

  function isBoardLinkConfig(popup) {
    if (!popup || typeof popup !== "object") return false;
    return popup.mode === "board_link" || popup.__kind === "board_link";
  }

  function isPanelPopupConfig(popup) {
    if (!popup || typeof popup !== "object") return false;
    return popup.mode === "popup_panel" || popup.__kind === "popup_panel";
  }

  function normalizeProjection(value) {
    const raw = String(value || "overlay")
      .trim()
      .toLowerCase();
    if (raw === "route" || raw === "navigate" || raw === "spa" || raw === "page") {
      return "route";
    }
    return "overlay";
  }

  function normalizeShellLayout(raw) {
    if (!raw || typeof raw !== "object" || Array.isArray(raw)) return null;
    const columns = Array.isArray(raw.columns)
      ? raw.columns.map((entry) => String(entry || "").trim()).filter(Boolean)
      : [];
    const rows = Array.isArray(raw.rows)
      ? raw.rows.map((entry) => String(entry || "").trim()).filter(Boolean)
      : [];
    const areas = Array.isArray(raw.areas)
      ? raw.areas
          .map((row) =>
            Array.isArray(row)
              ? row.map((entry) => String(entry || "").trim()).filter(Boolean)
              : []
          )
          .filter((row) => row.length > 0)
      : [];
    const gap = nonEmptyString(raw.gap);
    const padding = nonEmptyString(raw.padding);
    if (!columns.length && !rows.length && !areas.length && !gap && !padding) return null;
    return {
      columns,
      rows,
      areas,
      gap,
      padding,
    };
  }

  function inferSceneShellLayoutMode(zones = []) {
    const roles = new Set(
      (Array.isArray(zones) ? zones : []).map((zone) => nonEmptyString(zone?.role)).filter(Boolean),
    );
    if (roles.has("tab_bar") && roles.has("tab_content")) return "generic_tabs";
    if (roles.has("row_preview")) return "list_preview";
    if (roles.has("filter") && roles.has("slots")) return "analytics";
    return "";
  }

  function normalizeSceneShellZone(raw, parent = "") {
    if (!raw || typeof raw !== "object" || Array.isArray(raw)) return null;
    const props = raw.props && typeof raw.props === "object" && !Array.isArray(raw.props) ? raw.props : {};
    const slot = raw.slot && typeof raw.slot === "object" && !Array.isArray(raw.slot) ? raw.slot : {};
    const id = nonEmptyString(raw.id, props.projection_id, props.zone_id);
    const role = nonEmptyString(slot.kind, slot.role, props.projection_role, props.zone_role);
    if (!id || !role) return null;
    const accepts = Array.isArray(slot.accepts)
      ? slot.accepts.map((entry) => nonEmptyString(entry)).filter(Boolean)
      : Array.isArray(props.projection_accepts)
        ? props.projection_accepts.map((entry) => nonEmptyString(entry)).filter(Boolean)
      : [];
    return {
      id,
      role,
      area: nonEmptyString(raw.area, props.area),
      parent,
      source: nonEmptyString(slot.source, props.projection_source, props.source),
      selectionSource: nonEmptyString(
        slot.selection_from,
        slot.selectionFrom,
        props.selection_source,
        props.selectionSource,
      ),
      required: boolValue(slot.required, props.projection_required, false),
      max: positiveInt(slot.max, props.projection_max, props.max),
      accepts,
      layout: normalizeShellLayout(raw.layout),
    };
  }

  function normalizeSceneShellContract(rawFrame, rawPanels) {
    const frame =
      rawFrame && typeof rawFrame === "object" && !Array.isArray(rawFrame) ? rawFrame : null;
    const panels = Array.isArray(rawPanels) ? rawPanels : [];
    const zones = [];
    const collectZones = (items, parent = "") => {
      (Array.isArray(items) ? items : []).forEach((item) => {
        if (!item || typeof item !== "object" || Array.isArray(item) || item.kind !== "panel") return;
        const zone = normalizeSceneShellZone(item, parent);
        if (zone) {
          zones.push(zone);
        }
        const childPanels = Array.isArray(item.blocks)
          ? item.blocks.filter((block) => block && typeof block === "object" && block.kind === "panel")
          : [];
        collectZones(childPanels, nonEmptyString(item.id));
      });
    };
    collectZones(panels);
    const layout = normalizeShellLayout(frame?.layout);
    const layoutMode = inferSceneShellLayoutMode(zones);
    if (!layoutMode && !zones.length && !layout) return null;
    return {
      layoutMode,
      layout,
      zones,
    };
  }

  function normalizeSceneLocalNav(raw) {
    if (!raw || typeof raw !== "object" || Array.isArray(raw)) return null;
    const itemsRaw = Array.isArray(raw.items)
      ? raw.items
      : Array.isArray(raw.tabs)
        ? raw.tabs
        : [];
    const items = itemsRaw
      .filter((entry) => entry && typeof entry === "object")
      .map((entry) => ({
        id: normalizeTabId(entry.id || entry.tab || entry.key),
        kind: normalizeTabId(entry.kind || entry.role || entry.id || entry.tab || entry.key),
        role: nonEmptyString(entry.role),
        label: nonEmptyString(entry.label),
      }))
      .filter((entry) => entry.id || entry.kind);
    const kindOrder = Array.from(
      new Set(
        [
          ...(Array.isArray(raw.order_by_kind) ? raw.order_by_kind : []),
          ...(Array.isArray(raw.kind_order) ? raw.kind_order : []),
          ...(Array.isArray(raw.kindOrder) ? raw.kindOrder : []),
          ...items.map((entry) => entry.kind || entry.id),
        ]
          .map((entry) => normalizeTabId(entry))
          .filter(Boolean),
      ),
    );
    const overlaySize = normalizeOverlaySize(
      nonEmptyString(raw.overlay_size, raw.overlaySize),
      "",
    );
    const kind = nonEmptyString(raw.kind);
    const sceneId = nonEmptyString(raw.scene_id, raw.sceneId);
    if (!items.length && !kindOrder.length && !overlaySize && !kind && !sceneId) return null;
    return {
      kind,
      sceneId,
      defaultEntry: normalizeTabId(nonEmptyString(raw.default_entry, raw.defaultEntry, raw.defaultEntryTab)),
      includeHero: boolValue(raw.include_hero, raw.includeHero, true),
      overlaySize,
      items,
      kindOrder,
    };
  }

  const DRILLDOWN_OVERLAY_SIZE_CLASSES = [
    "access-drilldown-overlay--size-comfortable",
    "access-drilldown-overlay--size-large",
    "access-drilldown-overlay--size-fullscreen",
  ];
  const DRILLDOWN_PANEL_SIZE_CLASSES = [
    "access-drilldown-overlay-panel--size-comfortable",
    "access-drilldown-overlay-panel--size-large",
    "access-drilldown-overlay-panel--size-fullscreen",
  ];

  function normalizeOverlaySize(raw, fallback = "comfortable") {
    const value = String(raw || fallback || "comfortable")
      .trim()
      .toLowerCase();
    if (value === "large" || value === "fullscreen" || value === "comfortable") {
      return value;
    }
    if (value === "full" || value === "max" || value === "maximum") {
      return "fullscreen";
    }
    if (value === "medium" || value === "default" || value === "moderate") {
      return "comfortable";
    }
    return fallback ? normalizeOverlaySize(fallback, "comfortable") : "comfortable";
  }

  function resolveDrilldownOverlaySize({ popup, boardFields, structuredBoard, sceneShell = null }) {
    const sceneRef =
      popup?.scene && typeof popup.scene === "object" && !Array.isArray(popup.scene) ? popup.scene : {};
    const localNav =
      boardFields?.localNav ||
      normalizeSceneLocalNav(
        popup?.local_nav ||
          popup?.localNav ||
          sceneRef?.local_nav ||
          sceneRef?.localNav,
      );
    const explicit = nonEmptyString(
      popup?.overlay_size,
      popup?.overlaySize,
      sceneRef?.overlay_size,
      sceneRef?.overlaySize,
      localNav?.overlaySize,
    );
    const largeDefault = structuredBoard && sceneShell?.layoutMode !== "generic_tabs";
    if (explicit) {
      return normalizeOverlaySize(explicit, largeDefault ? "large" : "comfortable");
    }
    return largeDefault ? "large" : "comfortable";
  }

  function applyDrilldownOverlaySize(root, config) {
    const overlayEl = root.classList.contains("access-drilldown-overlay")
      ? root
      : root.closest(".access-drilldown-overlay");
    const panelEl = root.querySelector(".access-drilldown-overlay-panel");
    const size = normalizeOverlaySize(
      config?.overlaySize,
      config?.structuredBoard && config?.sceneShell?.layoutMode !== "generic_tabs"
        ? "large"
        : "comfortable",
    );
    if (overlayEl instanceof HTMLElement) {
      overlayEl.classList.remove(...DRILLDOWN_OVERLAY_SIZE_CLASSES);
      overlayEl.classList.add(`access-drilldown-overlay--size-${size}`);
    }
    if (panelEl instanceof HTMLElement) {
      panelEl.classList.remove(...DRILLDOWN_PANEL_SIZE_CLASSES);
      panelEl.classList.add(`access-drilldown-overlay-panel--size-${size}`);
      panelEl.dataset.drilldownOverlaySize = size;
    }
  }

  function resolveSceneLocalNav(sceneFile, runtimeMap = null) {
    const normalized = normalizeDrilldownScenePath(sceneFile);
    if (!normalized) return null;
    if (runtimeMap && typeof runtimeMap === "object" && !Array.isArray(runtimeMap)) {
      const dynamic = normalizeSceneLocalNav(runtimeMap[normalized]);
      if (dynamic) return dynamic;
    }
    return normalizeSceneLocalNav(SCENE_LOCAL_NAV_BY_FILE[normalized]);
  }

  function sceneLocalNavTabIds(localNav) {
    if (!localNav || !Array.isArray(localNav.items)) return [];
    return localNav.items
      .map((entry) => normalizeTabId(entry?.id))
      .filter((tab) => tab && tab !== "hero");
  }

  function normalizeSceneParams(raw) {
    if (!raw || typeof raw !== "object" || Array.isArray(raw)) return {};
    const normalized = {};
    Object.entries(raw).forEach(([key, value]) => {
      const id = String(key || "").trim();
      if (!id) return;
      normalized[id] = value;
    });
    return normalized;
  }

  function resolveBoardLinkFields(popup, runtimeSceneNavMap = null) {
    if (!popup || typeof popup !== "object") return null;
    const boardLink = isBoardLinkConfig(popup);
    const panelPopup = isPanelPopupConfig(popup);
    if (!boardLink && !panelPopup) return null;
    const legacyTemplate = panelPopup && !boardLink ? normalizePanelTemplateId(popup?.template) : "";
    const sceneRef =
      popup?.scene && typeof popup.scene === "object" && !Array.isArray(popup.scene) ? popup.scene : {};
    const params = normalizeSceneParams(popup?.params);
    const sceneFile = normalizeDrilldownScenePath(
      nonEmptyString(
        popup?.scene_file,
        popup?.sceneFile,
        sceneRef?.scene_file,
        sceneRef?.sceneFile,
        boardLink ? "" : BOARD_TEMPLATE_SCENE_FILES[legacyTemplate],
      ),
    );
    const localNav = normalizeSceneLocalNav(
      popup?.local_nav ||
        popup?.localNav ||
        sceneRef?.local_nav ||
        sceneRef?.localNav ||
        resolveSceneLocalNav(sceneFile, runtimeSceneNavMap),
    );
    const sceneId = nonEmptyString(
      popup?.scene_id,
      popup?.sceneId,
      sceneRef?.scene_id,
      sceneRef?.sceneId,
      localNav?.sceneId,
      sceneFile ? DRILLDOWN_SCENE_BY_FILE[sceneFile] : "",
    );
    const entry = normalizeTabId(
      nonEmptyString(
        popup?.entry,
        popup?.entry_tab,
        popup?.entryTab,
        sceneRef?.entry,
        sceneRef?.entry_tab,
        sceneRef?.entryTab,
        popup?.focus,
        params?.entry,
        params?.entry_tab,
        params?.entryTab,
        params?.focus,
        localNav?.defaultEntry,
      ),
    );
    return {
      boardLink: boardLink || Boolean(sceneFile),
      panelPopup,
      legacyTemplate,
      sceneRef,
      sceneFile,
      sceneId,
      projection: normalizeProjection(popup?.projection),
      entry,
      localNav,
      params,
    };
  }

  function normalizePanelTemplateId(template) {
    const raw = String(template || "").trim();
    if (!raw || raw === "metric_default") return "metric_board_default";
    return raw;
  }

  function normalizeDrilldownScenePath(raw) {
    let path = String(raw || "")
      .trim()
      .replace(/\\/g, "/");
    while (path.startsWith("../")) {
      path = path.slice(3);
    }
    return path.replace(/^\.?\/*/, "");
  }

  function nonEmptyString(...values) {
    for (const value of values) {
      const text = String(value || "").trim();
      if (text) return text;
    }
    return "";
  }

  function isWorldMetricsOwnerDatasetId(datasetId) {
    const id = String(datasetId || "").trim();
    return id === "__world_metrics__" || id.startsWith("__world_metrics__::");
  }

  function importedCapsuleScenePathFromWorldMetricsDatasetId(datasetId) {
    const text = String(datasetId || "").trim();
    const prefix = "__world_metrics__::";
    const suffix = "::metrics";
    if (!text.startsWith(prefix) || !text.endsWith(suffix)) {
      return "";
    }
    return text.slice(prefix.length, text.length - suffix.length);
  }

  function importedCapsuleScenePathFromMetricId(metricId) {
    const text = String(metricId || "").trim();
    const marker = ".mei::";
    const idx = text.indexOf(marker);
    if (idx <= 0) {
      return "";
    }
    return text.slice(0, idx + 4);
  }

  function resolveMetricOwnerScenePath(projectionSlots, detail) {
    if (Array.isArray(projectionSlots)) {
      for (const slot of projectionSlots) {
        const fromDataset = importedCapsuleScenePathFromWorldMetricsDatasetId(
          slot?.datasetId ?? slot?.dataset_id,
        );
        if (fromDataset) {
          return fromDataset;
        }
        const fromMetric = importedCapsuleScenePathFromMetricId(slot?.metricId ?? slot?.metric_id);
        if (fromMetric) {
          return fromMetric;
        }
      }
    }
    return nonEmptyString(
      importedCapsuleScenePathFromMetricId(detail?.metric_id),
      importedCapsuleScenePathFromWorldMetricsDatasetId(detail?.dataset_id),
      detail?.host_scene_file,
    );
  }

  function cloneArray(value) {
    return Array.isArray(value) ? value.slice() : [];
  }

  function positiveInt(...values) {
    for (const value of values) {
      const parsed = Number(value);
      if (Number.isFinite(parsed) && parsed > 0) {
        return Math.floor(parsed);
      }
    }
    return 0;
  }

  function boolValue(...values) {
    for (const value of values) {
      if (typeof value === "boolean") return value;
    }
    return undefined;
  }

  function runtimeDrilldownConfig(detail) {
    const value =
      detail?.analysis_contract &&
      typeof detail.analysis_contract === "object" &&
      !Array.isArray(detail.analysis_contract)
        ? detail.analysis_contract
        : detail?.__mei_runtime_ref?.analysis_contract &&
            typeof detail.__mei_runtime_ref.analysis_contract === "object" &&
            !Array.isArray(detail.__mei_runtime_ref.analysis_contract)
          ? detail.__mei_runtime_ref.analysis_contract
          : null;
    if (!value) {
      return {};
    }
    return value;
  }

  function disabledDrilldownConfig(errorCode, errorMessage) {
    return {
      enabled: false,
      errorCode: String(errorCode || "").trim(),
      errorMessage: String(errorMessage || "").trim(),
      sceneId: "",
      hostSceneId: "",
      boardSceneId: "",
      popup: {},
    };
  }

  function runtimeTabIds(...values) {
    const raw = values.find((value) => Array.isArray(value));
    if (!raw) return [];
    return Array.from(
      new Set(
        raw
      .map((entry) => {
        if (entry && typeof entry === "object") {
          return nonEmptyString(entry.id, entry.tab, entry.key, entry.name);
        }
        return String(entry || "").trim();
      })
          .map((entry) => normalizeTabId(entry))
          .filter(Boolean),
      ),
    );
  }

  function normalizeTabId(value) {
    const raw = String(value || "").trim().toLowerCase();
    if (!raw) return "";
    if (["口径", "definition", "def", "metric_definition", "metric-definition"].includes(raw)) {
      return "definition";
    }
    if (["构成", "composition", "breakdown", "group"].includes(raw)) {
      return "composition";
    }
    if (["趋势", "trend", "timeseries", "time_series", "time-series"].includes(raw)) {
      return "trend";
    }
    if (
      ["分子分母", "ratio", "numerator_denominator", "numerator-denominator", "numerator"].includes(
        raw,
      )
    ) {
      return "numerator_denominator";
    }
    if (["归因", "attribution", "reason"].includes(raw)) {
      return "attribution";
    }
    if (["明细", "detail", "details"].includes(raw)) {
      return "detail";
    }
    return raw.replaceAll(/[\s-]+/g, "_");
  }

  function normalizeExplainMetrics(...values) {
    const byId = {};
    const order = [];
    const pushEntry = (entry, fallbackId = "", fallbackLabel = "") => {
      if (!entry || typeof entry !== "object" || Array.isArray(entry)) return;
      const id = normalizeTabId(entry.id || fallbackId || "");
      const kind = normalizeTabId(entry.kind || id);
      if (!id || !kind || kind === "note") return;
      if (byId[id]) return;
      byId[id] = {
        id,
        kind,
        label: nonEmptyString(entry.label, fallbackLabel, fallbackId),
        by: nonEmptyString(entry.by),
        dateField: nonEmptyString(entry.date_field, entry.dateField),
        grain: nonEmptyString(entry.grain),
        fields: cloneArray(entry.fields),
        headers: cloneArray(entry.headers),
        mapping: entry.mapping && typeof entry.mapping === "object" ? entry.mapping : null,
        chartKind: nonEmptyString(entry.chart_kind, entry.chartKind),
        source: entry.source && typeof entry.source === "object" && !Array.isArray(entry.source) ? entry.source : null,
        tableMetricId: nonEmptyString(entry.table_metric_id, entry.tableMetricId, entry.metric_id, entry.metricId),
        datasetId: nonEmptyString(entry.dataset_id, entry.datasetId),
        topN: positiveInt(entry.top_n, entry.topN),
        numerator: nonEmptyString(entry.numerator),
        denominator: nonEmptyString(entry.denominator),
        formula: nonEmptyString(entry.formula),
      };
      order.push(id);
    };
    values.forEach((value) => {
      if (Array.isArray(value)) {
        value.forEach((entry) => pushEntry(entry));
        return;
      }
      if (value && typeof value === "object") {
        Object.entries(value).forEach(([key, entry]) => pushEntry(entry, key, key));
      }
    });
    return { byId, order };
  }

  function explainMetricForTab(config, tabId) {
    const key = normalizeTabId(tabId);
    if (!key) return null;
    const byId = config?.explainMetrics;
    if (!byId || typeof byId !== "object") return null;
    if (byId[key]) return byId[key];
    const genericKinds = new Set([
      "definition",
      "composition",
      "trend",
      "numerator_denominator",
      "attribution",
      "detail",
    ]);
    if (!genericKinds.has(key)) return null;
    const order = Array.isArray(config?.explainMetricOrder) ? config.explainMetricOrder : Object.keys(byId);
    for (const candidateId of order) {
      const entry = byId[normalizeTabId(candidateId)];
      if (!entry) continue;
      if (normalizeTabId(entry.kind || entry.id) === key) return entry;
    }
    return null;
  }

  function compositionFieldsFromOverride(override) {
    if (!override || typeof override !== "object") return [];
    return cloneArray(override.compositionBy).length
      ? cloneArray(override.compositionBy)
      : cloneArray(override.composition_by);
  }

  function compositionFieldForTab(config, tabId, override = null) {
    const exactTab = normalizeTabId(tabId);
    const slot = config?.slotByTab?.[exactTab];
    if (slot) {
      const fromSlot = compositionFieldsFromOverride(slot)[0];
      if (fromSlot) return fromSlot;
    }
    const explainMetric =
      explainMetricForTab(config, exactTab) || explainMetricForTab(config, explainMetricKind(config, tabId));
    const fromExplain = nonEmptyString(explainMetric?.by);
    if (fromExplain) return fromExplain;
    const fromOverride = nonEmptyString(compositionFieldsFromOverride(override)[0]);
    if (fromOverride) return fromOverride;
    const fromConfig = nonEmptyString(
      Array.isArray(config?.compositionBy) ? config.compositionBy[0] : "",
      Array.isArray(config?.recommendedDimensions) ? config.recommendedDimensions[0] : "",
    );
    return fromConfig;
  }

  function rowFieldValue(row, field, columns = []) {
    const name = String(field || "").trim();
    if (!name || !row || typeof row !== "object") return "";
    if (Object.prototype.hasOwnProperty.call(row, name)) {
      return row[name];
    }
    const trimmed = name.trim();
    for (const [key, value] of Object.entries(row)) {
      if (String(key).trim() === trimmed) return value;
    }
    const column = columns.find((entry) => String(entry || "").trim() === trimmed);
    if (column && Object.prototype.hasOwnProperty.call(row, column)) {
      return row[column];
    }
    return "";
  }

  function explainMetricKind(config, tabId) {
    const slot = config?.slotByTab?.[normalizeTabId(tabId)];
    if (slot?.supportRole) return normalizeTabId(slot.supportRole);
    const explainMetric = explainMetricForTab(config, tabId);
    if (explainMetric?.kind) return normalizeTabId(explainMetric.kind);
    return normalizeTabId(tabId);
  }

  function defaultDrilldownTabs(explainKind, options = {}) {
    const kind = String(explainKind || "").trim().toLowerCase();
    const hasDetail = options.hasDetail !== false;
    const aggregateKinds = new Set(["count", "sum", "avg", "average", "median"]);
    const ratioKinds = new Set(["ratio", "percent", "yoy", "mom"]);
    const trendKinds = new Set(["trend", "dataframe", "timeseries", "series"]);
    const breakdownKinds = new Set(["ranking", "breakdown", "group", "group_by", "groupby"]);

    if (aggregateKinds.has(kind)) {
      return hasDetail
        ? ["definition", "composition", "trend", "detail"]
        : ["definition", "composition", "trend"];
    }
    if (ratioKinds.has(kind)) {
      return hasDetail
        ? ["definition", "numerator_denominator", "trend", "detail"]
        : ["definition", "numerator_denominator", "trend"];
    }
    if (trendKinds.has(kind)) {
      return hasDetail
        ? ["definition", "trend", "composition", "detail"]
        : ["definition", "trend", "composition"];
    }
    if (breakdownKinds.has(kind)) {
      return hasDetail
        ? ["definition", "composition", "attribution", "detail"]
        : ["definition", "composition", "attribution"];
    }
    return hasDetail ? ["definition", "detail"] : ["definition"];
  }

  function buildRatioExplainNote({ numerator = "", denominator = "", formula = "" } = {}) {
    const n = String(numerator || "").trim();
    const d = String(denominator || "").trim();
    const f = String(formula || "").trim();
    if (f) return `分子分母口径：${f}`;
    if (n && d) return `分子分母口径：${n} / ${d}`;
    return "";
  }

  function panelPopupSlotSources(popup) {
    if (!popup || typeof popup !== "object") return null;
    if (popup.entry_overrides && typeof popup.entry_overrides === "object" && !Array.isArray(popup.entry_overrides)) {
      return popup.entry_overrides;
    }
    if (popup.entryOverrides && typeof popup.entryOverrides === "object" && !Array.isArray(popup.entryOverrides)) {
      return popup.entryOverrides;
    }
    if (popup.slots && typeof popup.slots === "object" && !Array.isArray(popup.slots)) {
      return popup.slots;
    }
    if (popup.metrics && typeof popup.metrics === "object" && !Array.isArray(popup.metrics)) {
      return popup.metrics;
    }
    return null;
  }

  function resolveDrilldownTabs({ detail, runtime, explainKind, hasDetail, localNav }) {
    const resolvedLocalNav =
      normalizeSceneLocalNav(localNav) ||
      resolveSceneLocalNav(
        nonEmptyString(detail?.board_scene_file, detail?.scene_path, detail?.popup?.scene_file),
        detail?.scene_local_nav_by_target,
      );
    const explainMetrics = normalizeExplainMetrics(
      detail?.analysis_contract?.blocks,
      detail?.analysis_contract?.explain_metrics,
      detail?.analysis_contract?.explainMetrics,
      detail?.explain_metrics,
      runtime?.blocks,
      runtime?.explain_metrics,
      runtime?.explainMetrics,
    );
    const explicitExplainTabs = explainMetrics.order.filter((tab) => normalizeTabId(tab) !== "hero");
    const popup = detail?.popup && typeof detail.popup === "object" ? detail.popup : {};
    const popupFocus = normalizeTabId(popup?.entry || popup?.entry_tab || popup?.focus);
    const overrideTabs = Object.keys(
      normalizeTabMetricOverrides(
        popup?.entry_overrides,
        popup?.entryOverrides,
        panelPopupSlotSources(popup),
        popup?.metrics,
      ),
    );
    let merged = explicitExplainTabs.length
      ? explicitExplainTabs
      : defaultDrilldownTabs(explainKind, { hasDetail });
    overrideTabs.forEach((tab) => {
      const normalized = normalizeTabId(tab);
      if (!normalized || merged.includes(normalized)) return;
      merged.push(normalized);
    });
    if (popupFocus && !merged.includes(popupFocus)) {
      merged.push(popupFocus);
    }
    merged = Array.from(new Set(merged)).filter((tab) => tab !== "hero");
    if (merged.length) {
      const kindOrder =
        Array.isArray(resolvedLocalNav?.kindOrder) && resolvedLocalNav.kindOrder.length
          ? resolvedLocalNav.kindOrder
          : SCENE_KIND_ORDER_FALLBACK;
      const kindRank = new Map(kindOrder.map((kind, index) => [normalizeTabId(kind), index]));
      merged.sort((left, right) => {
        const leftKind = normalizeTabId(explainMetrics.byId[left]?.kind || left);
        const rightKind = normalizeTabId(explainMetrics.byId[right]?.kind || right);
        const leftRank = kindRank.has(leftKind) ? kindRank.get(leftKind) : Number.MAX_SAFE_INTEGER;
        const rightRank = kindRank.has(rightKind) ? kindRank.get(rightKind) : Number.MAX_SAFE_INTEGER;
        if (leftRank !== rightRank) return leftRank - rightRank;
        return left.localeCompare(right);
      });
      return merged;
    }
    const explicit = runtimeTabIds(
      detail?.analysis_contract?.tabs,
      runtime?.tabs,
      detail?.analysis_tabs,
      runtime?.analysis_tabs,
    );
    const defaults = defaultDrilldownTabs(explainKind, { hasDetail });
    if (!explicit.length) return defaults;
    const normalizedExplicit = Array.from(new Set(explicit.map((tab) => normalizeTabId(tab)).filter(Boolean)));
    const basicTabs = new Set(["definition", "detail", "numerator_denominator"]);
    const hasOnlyBasicTabs = normalizedExplicit.every((tab) => basicTabs.has(tab));
    if (!hasOnlyBasicTabs) return normalizedExplicit;
    const mergedTabs = normalizedExplicit.filter((tab) => tab !== "detail");
    defaults.forEach((tab) => {
      const normalized = normalizeTabId(tab);
      if (!normalized || normalized === "detail" || mergedTabs.includes(normalized)) return;
      mergedTabs.push(normalized);
    });
    if (normalizedExplicit.includes("detail") || defaults.includes("detail")) {
      mergedTabs.push("detail");
    }
    return mergedTabs;
  }

  function normalizeTabMetricOverrides(...values) {
    const raw = values.find(
      (value) =>
        value &&
        typeof value === "object" &&
        !Array.isArray(value) &&
        Object.keys(value).length > 0,
    );
    if (!raw) return {};
    const normalized = {};
    Object.entries(raw).forEach(([key, entry]) => {
      const tabId = normalizeTabId(key);
      if (!tabId) return;
      if (typeof entry === "string") {
        const metricId = String(entry || "").trim();
        if (!metricId) return;
        normalized[tabId] = { tableMetricId: metricId };
        return;
      }
      if (!entry || typeof entry !== "object" || Array.isArray(entry)) {
        return;
      }
      if (entry.__ref === "explain_metric") {
        const explainMetricId = nonEmptyString(entry.id);
        if (!explainMetricId) return;
        normalized[tabId] = {
          explainMetricId,
          compositionBy: cloneArray(entry.composition_by).length
            ? cloneArray(entry.composition_by)
            : entry.by
              ? [String(entry.by)]
              : [],
          topN: positiveInt(entry.top_n, entry.topN),
          trendField: nonEmptyString(entry.trend_field, entry.date_field, entry.dateField),
          trendGrain: nonEmptyString(entry.grain, entry.trend_grain, entry.trendGrain),
        };
        return;
      }
      if (entry.__ref === "metric") {
        const metricId = nonEmptyString(entry.id);
        if (!metricId) return;
        normalized[tabId] = {
          runtimeRef: {
            kind: "metric",
            metricId,
            datasetId: nonEmptyString(entry.from_dataset, entry.fromDataset),
            sceneId: nonEmptyString(entry.scene_id, entry.sceneId),
            scenePath: nonEmptyString(entry.scene_file, entry.sceneFile),
          },
        };
        return;
      }
      const runtimeRef =
        entry.__mei_runtime_ref && typeof entry.__mei_runtime_ref === "object"
          ? entry.__mei_runtime_ref
          : null;
      if (runtimeRef?.kind === "metric") {
        const metricId = nonEmptyString(runtimeRef.metric_id, runtimeRef.metricId);
        if (!metricId) return;
        normalized[tabId] = {
          runtimeRef: {
            kind: "metric",
            metricId,
            datasetId: nonEmptyString(runtimeRef.dataset_id, runtimeRef.datasetId),
            sceneId: nonEmptyString(runtimeRef.scene_id, runtimeRef.sceneId),
            scenePath: nonEmptyString(runtimeRef.scene_path, runtimeRef.scenePath),
          },
        };
        return;
      }
      if (entry.__ref === "dataset") {
        const datasetId = nonEmptyString(entry.id);
        if (!datasetId) return;
        normalized[tabId] = {
          runtimeRef: {
            kind: "data",
            datasetId,
            sceneId: nonEmptyString(entry.scene_id, entry.sceneId),
            scenePath: nonEmptyString(entry.scene_file, entry.sceneFile, entry.path),
          },
        };
        return;
      }
      if (entry.source && typeof entry.source === "object" && !Array.isArray(entry.source)) {
        const source = entry.source;
        const sourceKind = nonEmptyString(source.kind);
        if (sourceKind === "metric_ref") {
          normalized[tabId] = {
            runtimeRef: {
              kind: "metric",
              metricId: nonEmptyString(source.metric_id, source.metricId, entry.table_metric_id, entry.tableMetricId),
              datasetId: nonEmptyString(source.dataset_id, source.datasetId, entry.dataset_id, entry.datasetId),
              sceneId: nonEmptyString(source.scene_id, source.sceneId),
              scenePath: nonEmptyString(source.scene_file, source.sceneFile),
            },
            tableMetricId: nonEmptyString(source.metric_id, source.metricId, entry.table_metric_id, entry.tableMetricId),
            datasetId: nonEmptyString(source.dataset_id, source.datasetId, entry.dataset_id, entry.datasetId),
            topN: positiveInt(entry.top_n, entry.topN),
          };
          return;
        }
        if (sourceKind === "dataset_ref") {
          normalized[tabId] = {
            runtimeRef: {
              kind: "data",
              datasetId: nonEmptyString(source.dataset_id, source.datasetId, entry.dataset_id, entry.datasetId),
              sceneId: nonEmptyString(source.scene_id, source.sceneId),
              scenePath: nonEmptyString(source.scene_file, source.sceneFile),
            },
            datasetId: nonEmptyString(source.dataset_id, source.datasetId, entry.dataset_id, entry.datasetId),
          };
          return;
        }
      }
      if (entry.runtime_ref && typeof entry.runtime_ref === "object" && !Array.isArray(entry.runtime_ref)) {
        const runtimeRef = entry.runtime_ref;
        normalized[tabId] = {
          runtimeRef: {
            kind: nonEmptyString(runtimeRef.kind),
            metricId: nonEmptyString(runtimeRef.metric_id, runtimeRef.metricId, entry.metric_id, entry.metricId),
            datasetId: nonEmptyString(runtimeRef.dataset_id, runtimeRef.datasetId, entry.dataset_id, entry.datasetId),
            sceneId: nonEmptyString(runtimeRef.scene_id, runtimeRef.sceneId),
            scenePath: nonEmptyString(runtimeRef.scene_path, runtimeRef.scenePath),
          },
          tableMetricId: nonEmptyString(runtimeRef.metric_id, runtimeRef.metricId, entry.metric_id, entry.metricId),
          datasetId: nonEmptyString(runtimeRef.dataset_id, runtimeRef.datasetId, entry.dataset_id, entry.datasetId),
          columns: cloneArray(entry.fields),
          headers: cloneArray(entry.headers),
          mapping: entry.mapping && typeof entry.mapping === "object" ? entry.mapping : null,
          chartKind: nonEmptyString(entry.chart_kind, entry.chartKind),
          topN: positiveInt(entry.top_n, entry.topN),
          compositionBy: cloneArray(entry.composition_by).length
            ? cloneArray(entry.composition_by)
            : cloneArray(entry.compositionBy),
          trendField: nonEmptyString(entry.date_field, entry.dateField),
          trendGrain: nonEmptyString(entry.grain),
        };
        return;
      }
      let columns = cloneArray(entry.columns);
      if (!columns.length) columns = cloneArray(entry.detail_fields);
      if (!columns.length) columns = cloneArray(entry.detailFields);
      const override = {
        title: nonEmptyString(entry.title),
        note: nonEmptyString(entry.note),
        tableMetricId: nonEmptyString(
          entry.table_metric_id,
          entry.tableMetricId,
          entry.metric_id,
          entry.metricId,
        ),
        datasetId: nonEmptyString(entry.dataset_id, entry.datasetId),
        columns,
        headers: cloneArray(entry.headers),
        layoutPreset: nonEmptyString(entry.layout_preset, entry.layoutPreset),
        chartKind: nonEmptyString(entry.chart_kind, entry.chartKind, entry.chart),
        topN: positiveInt(entry.top_n, entry.topN),
        mapping: entry.mapping && typeof entry.mapping === "object" ? entry.mapping : null,
        compositionBy: cloneArray(entry.composition_by).length
          ? cloneArray(entry.composition_by)
          : cloneArray(entry.compositionBy),
        trendField: nonEmptyString(entry.trend_field, entry.trendField),
        trendGrain: nonEmptyString(entry.trend_grain, entry.trendGrain),
      };
      if (
        !override.title &&
        !override.note &&
        !override.tableMetricId &&
        !override.datasetId &&
        !override.columns.length &&
        !override.headers.length &&
        !override.layoutPreset &&
        !override.chartKind &&
        !override.topN &&
        !override.mapping &&
        !override.compositionBy.length &&
        !override.trendField
      ) {
        return;
      }
      normalized[tabId] = override;
    });
    return normalized;
  }

  function sceneProjectionAssembly(sceneId, assemblyById) {
    const normalizedSceneId = nonEmptyString(sceneId);
    if (!normalizedSceneId) return null;
    if (
      !assemblyById ||
      typeof assemblyById !== "object" ||
      Array.isArray(assemblyById) ||
      !assemblyById[normalizedSceneId] ||
      typeof assemblyById[normalizedSceneId] !== "object" ||
      Array.isArray(assemblyById[normalizedSceneId])
    ) {
      return null;
    }
    return assemblyById[normalizedSceneId];
  }

  function sceneBindingDefaults(sceneId, bindingsById, examplesById, assemblyById = null) {
    const normalizedSceneId = nonEmptyString(sceneId);
    if (!normalizedSceneId) return {};
    const assembly = sceneProjectionAssembly(normalizedSceneId, assemblyById);
    if (assembly) {
      const direct = normalizeTabMetricOverrides(assembly.bindings);
      if (Object.keys(direct).length) return direct;
      const rawExamples = assembly.examples;
      const example = Array.isArray(rawExamples)
        ? rawExamples.find((entry) => entry && typeof entry === "object" && !Array.isArray(entry))
        : rawExamples && typeof rawExamples === "object"
          ? rawExamples
          : null;
      const bindings =
        example && typeof example === "object" && !Array.isArray(example) ? example.bindings : null;
      const normalized = normalizeTabMetricOverrides(bindings);
      if (Object.keys(normalized).length) return normalized;
    }
    if (
      bindingsById &&
      typeof bindingsById === "object" &&
      !Array.isArray(bindingsById) &&
      bindingsById[normalizedSceneId]
    ) {
      const direct = normalizeTabMetricOverrides(bindingsById[normalizedSceneId]);
      if (Object.keys(direct).length) return direct;
    }
    if (
      examplesById &&
      typeof examplesById === "object" &&
      !Array.isArray(examplesById) &&
      examplesById[normalizedSceneId]
    ) {
      const rawExamples = examplesById[normalizedSceneId];
      const example = Array.isArray(rawExamples)
        ? rawExamples.find((entry) => entry && typeof entry === "object" && !Array.isArray(entry))
        : rawExamples && typeof rawExamples === "object"
          ? rawExamples
          : null;
      const bindings =
        example && typeof example === "object" && !Array.isArray(example) ? example.bindings : null;
      const normalized = normalizeTabMetricOverrides(bindings);
      if (Object.keys(normalized).length) return normalized;
    }
    return {};
  }

  function resolveDrilldownTabConfig(config, tabId) {
    const tabMetrics = config?.tabMetrics || {};
    const exactTab = normalizeTabId(tabId);
    const kindTab = explainMetricKind(config, tabId);
    const override = tabMetrics[exactTab] || tabMetrics[kindTab];
    const explainMetricTab = normalizeTabId(
      nonEmptyString(override?.explainMetricId, exactTab, kindTab)
    );
    const explainMetric =
      explainMetricForTab(config, explainMetricTab) ||
      explainMetricForTab(config, exactTab) ||
      explainMetricForTab(config, kindTab);
    if (!override && !explainMetric) return config;
    const overrideDatasetId = nonEmptyString(override?.datasetId);
    const overrideTableMetricId = nonEmptyString(override?.tableMetricId);
    const suppressDetailMetricFallback = Boolean(overrideDatasetId && !overrideTableMetricId);
    const merged = {
      ...config,
      title: nonEmptyString(override?.title, override?.label, explainMetric?.label, config.title),
      note: nonEmptyString(override?.note, config.note),
      tableMetricId:
        overrideTableMetricId ||
        (overrideDatasetId ? "" : nonEmptyString(explainMetric?.tableMetricId, config.tableMetricId)),
      datasetId: overrideDatasetId || nonEmptyString(override?.runtimeRef?.datasetId, explainMetric?.datasetId, config.datasetId),
      suppressDetailMetricFallback,
      layoutPreset: nonEmptyString(override?.layoutPreset, config.layoutPreset),
      chartKind: nonEmptyString(override?.chartKind, explainMetric?.chartKind, config.chartKind),
      topN: positiveInt(
        override?.top_n,
        override?.topN,
        explainMetric?.topN,
        config?.top_n,
        config?.topN,
      ),
      mapping:
        override?.mapping && typeof override.mapping === "object"
          ? override.mapping
          : explainMetric?.mapping && typeof explainMetric.mapping === "object"
            ? explainMetric.mapping
          : config.mapping && typeof config.mapping === "object"
            ? config.mapping
            : null,
      runtimeRef: (() => {
        const base =
          override?.runtimeRef && typeof override.runtimeRef === "object"
            ? { ...override.runtimeRef }
            : explainMetric?.source && typeof explainMetric.source === "object"
              ? {
                  kind:
                    nonEmptyString(explainMetric.source.kind) === "dataset_ref"
                      ? "data"
                      : nonEmptyString(explainMetric.source.kind) === "metric_ref"
                        ? "metric"
                        : nonEmptyString(explainMetric.source.kind),
                  metricId: nonEmptyString(explainMetric.source.metric_id, explainMetric.source.metricId),
                  datasetId: nonEmptyString(explainMetric.source.dataset_id, explainMetric.source.datasetId),
                  sceneId: nonEmptyString(explainMetric.source.scene_id, explainMetric.source.sceneId),
                  scenePath: nonEmptyString(explainMetric.source.scene_file, explainMetric.source.sceneFile),
                }
            : config.runtimeRef && typeof config.runtimeRef === "object"
              ? { ...config.runtimeRef }
              : null;
        if (!base) return null;
        if (!nonEmptyString(base.sceneId)) {
          base.sceneId = nonEmptyString(config.hostSceneId, config.sceneId);
        }
        if (!nonEmptyString(base.scenePath)) {
          base.scenePath = nonEmptyString(
            config.hostSceneFile,
            resolveMetricOwnerScenePath(
              config?.slotByTab ? Object.values(config.slotByTab) : [],
              null,
            ),
          );
        }
        return base;
      })(),
      columns: cloneArray(override?.columns).length
        ? cloneArray(override.columns)
        : cloneArray(explainMetric?.fields).length
          ? cloneArray(explainMetric.fields)
          : cloneArray(config.columns),
      headers: cloneArray(override?.headers).length
        ? cloneArray(override.headers)
        : cloneArray(explainMetric?.headers).length
          ? cloneArray(explainMetric.headers)
          : cloneArray(config.headers),
      compositionBy: (() => {
        const fromExplain = compositionFieldForTab(config, tabId, override);
        if (fromExplain) return [fromExplain];
        const fromOverride = compositionFieldsFromOverride(override);
        if (fromOverride.length) return fromOverride;
        return cloneArray(config.compositionBy);
      })(),
      trendField: nonEmptyString(
        explainMetric?.dateField,
        override?.trendField,
        config.trendField,
      ),
      trendGrain: nonEmptyString(explainMetric?.grain, override?.trendGrain, config.trendGrain),
    };
    return merged;
  }

  function hasTabMetricDataSource(override) {
    if (!override || typeof override !== "object") return false;
    if (
      nonEmptyString(override.explainMetricId) &&
      !nonEmptyString(override.tableMetricId, override.datasetId) &&
      !(override.runtimeRef && typeof override.runtimeRef === "object") &&
      !cloneArray(override.columns).length &&
      !compositionFieldsFromOverride(override).length &&
      !nonEmptyString(override.trendField) &&
      Number(override.topN) <= 0
    ) {
      return false;
    }
    return Boolean(
      (override.runtimeRef && typeof override.runtimeRef === "object") ||
        nonEmptyString(override.tableMetricId, override.datasetId) ||
        cloneArray(override.columns).length ||
        cloneArray(override.headers).length ||
        Number(override.topN) > 0 ||
        nonEmptyString(override.layoutPreset, override.chartKind) ||
        (override.mapping && typeof override.mapping === "object")
    );
  }

  function drilldownTabLabel(tabId, config = null) {
    const id = normalizeTabId(tabId);
    const metricId = String(
      config?.tableMetricId ||
        config?.runtimeRef?.metric_id ||
        config?.runtimeRef?.metricId ||
        "",
    ).trim();
    if (id === "trend" && metricId.includes("year_month_matrix")) {
      return "汇总";
    }
    const labels = {
      definition: "口径",
      composition: "构成",
      trend: "趋势",
      numerator_denominator: "分子分母",
      attribution: "归因",
      detail: "明细",
    };
    return labels[id] || id || "明细";
  }

  function defaultActiveDrilldownTab(tabs = []) {
    const normalized = Array.isArray(tabs) ? tabs.map((tab) => normalizeTabId(tab)).filter(Boolean) : [];
    if (!normalized.length) return "detail";
    for (const preferred of ["detail", "trend", "composition", "numerator_denominator", "definition"]) {
      if (normalized.includes(preferred)) {
        return preferred;
      }
    }
    return normalized[0];
  }

  function isDrilldownSummaryTab(tabId, config = null) {
    const normalized = explainMetricKind(config, tabId);
    return normalized === "definition" || normalized === "numerator_denominator";
  }

  function isDrilldownAnalysisTab(tabId, config = null) {
    const normalized = explainMetricKind(config, tabId);
    return normalized === "composition" || normalized === "trend" || normalized === "attribution";
  }

  function unconfiguredTabNote(tabId) {
    const normalized = normalizeTabId(tabId);
    if (normalized === "composition") {
      return "未配置构成数据块，当前展示推荐维度；可通过 popup.metrics.composition 指定正式 metric。";
    }
    if (normalized === "trend") {
      return "未配置趋势数据块，当前展示推荐维度；可通过 popup.metrics.trend 指定正式 metric。";
    }
    if (normalized === "attribution") {
      return "未配置归因数据块，当前展示推荐维度；可通过 popup.metrics.attribution 指定正式 metric。";
    }
    return "";
  }

  function createDrilldownSummaryNode(config, tabId) {
    const panel = document.createElement("div");
    panel.className = "access-drilldown-summary";
    const normalizedTab = explainMetricKind(config, tabId);
    const rows = [];

    if (config.explainKind) {
      rows.push(["指标类型", config.explainKind]);
    }

    if (normalizedTab === "numerator_denominator") {
      if (config.ratioParts?.numerator) {
        rows.push(["分子", config.ratioParts.numerator]);
      }
      if (config.ratioParts?.denominator) {
        rows.push(["分母", config.ratioParts.denominator]);
      }
      if (config.ratioParts?.formula) {
        rows.push(["公式", config.ratioParts.formula]);
      }
      if (!rows.length && config.note) {
        rows.push(["说明", config.note]);
      }
    } else {
      if (config.note) {
        rows.push(["说明", config.note]);
      }
      if (Array.isArray(config.basisRefs) && config.basisRefs.length) {
        rows.push(["口径依据", config.basisRefs.join(" / ")]);
      }
      if (Array.isArray(config.recommendedDimensions) && config.recommendedDimensions.length) {
        rows.push(["推荐维度", config.recommendedDimensions.join(" / ")]);
      }
      if (Array.isArray(config.detailFields) && config.detailFields.length) {
        rows.push(["明细字段", config.detailFields.join(" / ")]);
      }
    }

    if (!rows.length) {
      const empty = document.createElement("div");
      empty.className = "access-drilldown-summary-empty";
      empty.textContent = "暂无可展示的解释信息";
      panel.appendChild(empty);
      return panel;
    }

    rows.forEach(([label, value]) => {
      const row = document.createElement("div");
      row.className = "access-drilldown-summary-row";
      const labelEl = document.createElement("div");
      labelEl.className = "access-drilldown-summary-label";
      labelEl.textContent = String(label || "");
      const valueEl = document.createElement("div");
      valueEl.className = "access-drilldown-summary-value";
      valueEl.textContent = String(value || "");
      row.append(labelEl, valueEl);
      panel.appendChild(row);
    });
    return panel;
  }

  function applyDrilldownOverlayMeta(root, config) {
    const titleEl = root.querySelector('[data-drilldown-title="true"]');
    const noteEl = root.querySelector('[data-drilldown-note="true"]');
    const panelEl = root.querySelector(".access-drilldown-overlay-panel");
    const heroEl = root.querySelector('[data-drilldown-hero="true"]');
    const headMetaEl = root.querySelector(".access-drilldown-overlay-head-meta");
    const structuredLayout = root.querySelector('[data-drilldown-structured-layout="true"]');
    if (titleEl) titleEl.textContent = String(config?.title || "");
    if (noteEl) {
      const note = String(config?.note || "").trim();
      noteEl.textContent = note;
      noteEl.toggleAttribute("hidden", !note);
    }
    const boardMode = Boolean(
      config?.boardLink || (config?.panelPopup && config?.panelTemplate),
    );
    const structuredBoardMode = Boolean(config?.structuredBoard);
    if (panelEl) {
      panelEl.classList.toggle("access-drilldown-overlay-panel--board", boardMode);
      panelEl.dataset.drilldownPanelTemplate = boardMode ? String(config.panelTemplate) : "";
      panelEl.dataset.drilldownLayoutMode = String(config?.sceneShell?.layoutMode || "");
    }
    const genericBody = root.querySelector('[data-drilldown-body-mode="generic"]');
    const structuredBody = root.querySelector('[data-drilldown-body-mode="structured"]');
    if (genericBody instanceof HTMLElement) {
      genericBody.toggleAttribute("hidden", structuredBoardMode);
    }
    if (structuredBody instanceof HTMLElement) {
      structuredBody.toggleAttribute("hidden", !structuredBoardMode);
    }
    const tabsHost = root.querySelector('[data-drilldown-tabs="true"]');
    if (tabsHost instanceof HTMLElement) {
      tabsHost.toggleAttribute("hidden", structuredBoardMode);
    }
    if (structuredLayout instanceof HTMLElement) {
      structuredLayout.dataset.shellLayoutMode = String(config?.sceneShell?.layoutMode || "");
    }
    applyDrilldownOverlaySize(root, config);
    if (headMetaEl) {
      headMetaEl.toggleAttribute("hidden", boardMode);
    }
    if (heroEl) {
      heroEl.toggleAttribute("hidden", !boardMode);
      if (boardMode) {
        const heroTitle = heroEl.querySelector('[data-drilldown-hero-title="true"]');
        const heroNote = heroEl.querySelector('[data-drilldown-hero-note="true"]');
        if (heroTitle) heroTitle.textContent = String(config?.title || "");
        if (heroNote) {
          // 口径说明留在「口径」tab；明细 tab 不再重复展示 metric_explain.note 副标题。
          heroNote.textContent = "";
          heroNote.toggleAttribute("hidden", true);
        }
      }
    }
  }

  function resolveAccessAppBasePath(pathname = window.location.pathname) {
    const slug = appRouteSlugFromPathname(pathname);
    if (!ACCESS_LIKE_ROUTE_SLUGS.has(slug)) return "";
    const prefix = `/apps/${slug}/`;
    const raw = String(pathname || "");
    if (!raw.startsWith(prefix)) return "";
    const tail = raw.slice(prefix.length);
    const marker = tail.indexOf("/scene/");
    const app = marker >= 0 ? tail.slice(0, marker) : tail;
    const trimmed = String(app || "").trim();
    return trimmed ? `${prefix}${trimmed}` : "";
  }

  function resolveDrilldownDatasetId(detail, config = {}) {
    const runtimeRefConfig = config?.runtimeRef && typeof config.runtimeRef === "object" ? config.runtimeRef : {};
    const popupParams =
      detail?.popup && typeof detail.popup === "object" && !Array.isArray(detail.popup)
        ? detail.popup.params
        : null;
    const explicitDatasetId = nonEmptyString(runtimeRefConfig.datasetId, config?.datasetId);
    const rowsetDatasetId = nonEmptyString(
      runtimeRefConfig.rowsetDatasetId,
      config?.rowsetDatasetId,
      config?.filterSchema?.rowsetDatasetId,
      sceneParamRowsetDatasetId(config?.params),
      sceneParamRowsetDatasetId(popupParams),
    );
    const detailDatasetId = nonEmptyString(detail?.dataset_id);
    const safeDetailDatasetId = isWorldMetricsOwnerDatasetId(detailDatasetId) ? "" : detailDatasetId;
    const tableMetricId = nonEmptyString(config?.tableMetricId, detail?.table_metric_id);
    if (config?.structuredBoard && rowsetDatasetId) {
      return rowsetDatasetId;
    }
    if (tableMetricId) {
      return nonEmptyString(
        explicitDatasetId,
        rowsetDatasetId,
        detail?.explain_detail_dataset,
        safeDetailDatasetId,
        detailDatasetId,
      );
    }
    return nonEmptyString(
      detail?.explain_detail_dataset,
      explicitDatasetId,
      rowsetDatasetId,
      safeDetailDatasetId,
      detailDatasetId,
    );
  }

  function resolveDrilldownSceneId(detail, runtime = {}) {
    const runtimeTargetSceneId = nonEmptyString(
      runtime?.target_scene_id,
      runtime?.targetSceneId,
      runtime?.scene_id,
      runtime?.sceneId,
    );
    if (runtimeTargetSceneId) return runtimeTargetSceneId;
    const runtimeScene = normalizeDrilldownScenePath(
      nonEmptyString(
        runtime?.scene_file,
        runtime?.sceneFile,
        runtime?.scene_path,
        runtime?.scenePath,
        runtime?.scene,
      ),
    );
    if (!runtimeScene) return "";
    return DRILLDOWN_SCENE_BY_FILE[runtimeScene] || runtimeScene;
  }

  function normalizeProjectionSlots(raw) {
    if (!Array.isArray(raw)) return [];
    return raw
      .filter((entry) => entry && typeof entry === "object" && !Array.isArray(entry))
      .map((entry, index) => {
        const byRaw = entry.by ?? entry.composition_by ?? entry.compositionBy;
        let by = [];
        if (typeof byRaw === "string" && byRaw.trim()) {
          by = [byRaw.trim()];
        } else if (Array.isArray(byRaw)) {
          by = byRaw.map((item) => String(item || "").trim()).filter(Boolean);
        }
        const fieldsRaw = entry.fields ?? entry.detail_fields ?? entry.detailFields;
        const fields = Array.isArray(fieldsRaw)
          ? fieldsRaw.map((item) => String(item || "").trim()).filter(Boolean)
          : [];
        return {
          id: nonEmptyString(
            entry.id,
            entry.explain_block_id,
            entry.explainBlockId,
            entry.support_role,
            entry.supportRole,
            String(index),
          ),
          metricId: nonEmptyString(entry.metric_id, entry.metricId),
          datasetId: nonEmptyString(entry.dataset_id, entry.datasetId),
          component: nonEmptyString(entry.component, entry.as) || "data_table",
          label: nonEmptyString(entry.label),
          supportRole: nonEmptyString(entry.support_role, entry.supportRole, entry.component),
          default: Boolean(entry.default),
          fields,
          by,
          chartKind: nonEmptyString(entry.chart_kind, entry.chartKind),
          topN: positiveInt(entry.top_n, entry.topN),
          mapping:
            entry.mapping && typeof entry.mapping === "object" && !Array.isArray(entry.mapping)
              ? entry.mapping
              : null,
          explainBlockId: nonEmptyString(entry.explain_block_id, entry.explainBlockId),
          layoutZone: nonEmptyString(entry.layout_zone, entry.layoutZone),
        };
      })
      .filter((slot) => slot.metricId || slot.datasetId);
  }

  function resolveSceneShell(sceneAssembly) {
    return normalizeSceneShellContract(sceneAssembly?.frame, sceneAssembly?.panels);
  }

  function sceneShellZoneById(sceneShell, zoneId) {
    if (!sceneShell || !Array.isArray(sceneShell.zones) || !zoneId) return null;
    return sceneShell.zones.find((zone) => zone?.id === zoneId) || null;
  }

  function sceneShellZonesByRole(sceneShell, role) {
    if (!sceneShell || !Array.isArray(sceneShell.zones)) return [];
    return sceneShell.zones.filter((zone) => zone?.role === role);
  }

  function sceneShellFirstSlotZone(sceneShell, component) {
    if (!sceneShell || !Array.isArray(sceneShell.zones)) return null;
    return (
      sceneShell.zones.find(
        (zone) =>
          (zone?.role === "slots" || zone?.role === "row_preview" || zone?.role === "tab_content") &&
          Array.isArray(zone?.accepts) &&
          zone.accepts.includes(component),
      ) || null
    );
  }

  function groupProjectionSlotsByZone(projectionSlots = []) {
    const grouped = {};
    projectionSlots.forEach((slot) => {
      const zoneId = nonEmptyString(slot?.layoutZone);
      if (!zoneId) return;
      if (!Array.isArray(grouped[zoneId])) grouped[zoneId] = [];
      grouped[zoneId].push(slot);
    });
    return grouped;
  }

  function normalizeAnalyticsFilterSchema(raw) {
    if (!raw || typeof raw !== "object" || Array.isArray(raw)) {
      return { fields: [], rowsetDatasetId: "" };
    }
    const fields = Array.isArray(raw.fields)
      ? raw.fields
          .filter((entry) => entry && typeof entry === "object" && !Array.isArray(entry))
          .map((entry) => ({
            key: nonEmptyString(entry.key),
            label: nonEmptyString(entry.label, entry.key),
            column: nonEmptyString(entry.column, entry.key),
            control: nonEmptyString(entry.control, entry.type, "text"),
          }))
          .filter((entry) => entry.key)
      : [];
    return {
      fields,
      rowsetDatasetId: nonEmptyString(raw.rowset_dataset_id, raw.rowsetDatasetId),
    };
  }

  function resolveProjectionSlotsDrilldownConfig(detail, popup, boardFields, projectionSlots) {
    const metricId = String(detail?.metric_id || "").trim();
    const defaultSlot =
      projectionSlots.find((slot) => slot.default) || projectionSlots[0] || null;
    const tabs = projectionSlots.map((slot) => slot.id);
    const slotByTab = Object.fromEntries(projectionSlots.map((slot) => [slot.id, slot]));
    const boardSceneId = nonEmptyString(
      detail?.board_scene_id,
      boardFields?.sceneId,
      popup?.scene_id,
      popup?.sceneId,
      "generic_drilldown_board",
    );
    const hostSceneId = nonEmptyString(
      detail?.host_scene_id,
      detail?.dataset_scene_id,
      detail?.scene_id !== boardSceneId ? detail?.scene_id : "",
      detail?.__mei_runtime_ref?.scene_id,
      resolveDrilldownSceneId(detail, runtimeDrilldownConfig(detail)),
    );
    const ownerScenePath = resolveMetricOwnerScenePath(projectionSlots, detail);
    const projection = normalizeProjection(
      nonEmptyString(detail?.projection, popup?.projection, boardFields?.projection, "overlay"),
    );
    const title = nonEmptyString(
      popup?.title,
      detail?.label,
      defaultSlot?.label,
      metricId,
      "指标下钻",
    );
    const sceneAssembly = sceneProjectionAssembly(boardSceneId, detail?.scene_projection_assembly_by_id);
    const sceneShell = resolveSceneShell(sceneAssembly);
    const structuredBoard = Boolean(sceneShell?.layoutMode);
    const overlaySize = resolveDrilldownOverlaySize({
      popup,
      boardFields,
      structuredBoard,
      sceneShell,
    });
    const filterSchema = normalizeAnalyticsFilterSchema(popup?.filter_schema || popup?.filterSchema);
    const paramRowsetDatasetId = sceneParamRowsetDatasetId(boardFields?.params || popup?.params);
    const queryStateId = structuredBoard
      ? nonEmptyString(
          popup?.query_state_id,
          popup?.queryStateId,
          metricId ? `drilldown::${metricId}` : "",
        )
      : "";
    const slotsByZone = groupProjectionSlotsByZone(projectionSlots);
    const defaultTableSlot =
      projectionSlots.find((slot) => slot.component === "data_table") || defaultSlot;
    const rowPreviewZone = sceneShellZonesByRole(sceneShell, "row_preview")[0] || null;
    const rowPreviewZoneId = rowPreviewZone?.id || "";
    const rowPreviewSlot = rowPreviewZoneId ? (slotsByZone[rowPreviewZoneId] || [])[0] || null : null;
    const rowPreviewSourceZoneId = nonEmptyString(
      rowPreviewZone?.selectionSource,
      sceneShellFirstSlotZone(sceneShell, "data_table")?.id,
    );
    const tabBarZoneId = sceneShellZonesByRole(sceneShell, "tab_bar")[0]?.id || "";
    const tabContentZoneId = sceneShellZonesByRole(sceneShell, "tab_content")[0]?.id || "";
    const genericSceneShell = sceneShell?.layoutMode === "generic_tabs";
    return {
      enabled: Boolean(boardSceneId),
      genericDrilldown: !structuredBoard || genericSceneShell,
      structuredBoard,
      sceneShell,
      overlaySize,
      filterSchema,
      slotsByZone,
      detailSlot: defaultTableSlot,
      rowPreviewZoneId,
      rowPreviewSlot,
      rowPreviewSourceZoneId,
      tabBarZoneId,
      tabContentZoneId,
      queryStateId,
      rowsetDatasetId: nonEmptyString(filterSchema.rowsetDatasetId, paramRowsetDatasetId),
      params: boardFields?.params || normalizeSceneParams(popup?.params),
      sceneId: hostSceneId,
      hostSceneId,
      hostSceneFile: nonEmptyString(ownerScenePath, detail?.host_scene_file),
      boardSceneId,
      boardSceneFile: nonEmptyString(
        detail?.board_scene_file,
        boardFields?.sceneFile,
        popup?.scene_file,
        popup?.sceneFile,
        "templates/cockpit/drilldown/generic-drilldown-board.mei",
      ),
      projection,
      title,
      note: "",
      tableMetricId: nonEmptyString(defaultTableSlot?.metricId, metricId),
      datasetId: nonEmptyString(
        defaultTableSlot?.datasetId,
        detail?.dataset_id,
        detail?.__mei_runtime_ref?.dataset_id,
      ),
      hasChartZone: projectionSlots.some((slot) => slot.component === "chart"),
      hasRowPreviewZone: Boolean(rowPreviewZoneId),
      tabs,
      slotByTab,
      explainMetrics: Object.fromEntries(
        projectionSlots.map((slot) => [
          slot.id,
          {
            id: slot.id,
            kind: nonEmptyString(slot.supportRole, slot.id),
            label: slot.label,
          },
        ]),
      ),
      explainMetricOrder: projectionSlots.map((slot) => slot.id),
      tabMetrics: Object.fromEntries(
        projectionSlots.map((slot) => [
          slot.id,
          {
            title: slot.label,
            label: slot.label,
            tableMetricId: slot.metricId,
            datasetId: slot.datasetId,
            chartKind: slot.chartKind,
            topN: slot.topN,
            by: slot.by[0] || "",
            fields: slot.fields,
            compositionBy: slot.by,
            supportRole: slot.supportRole,
            runtimeRef: {
              kind: "metric",
              metricId: slot.metricId,
              datasetId: slot.datasetId,
              sceneId: hostSceneId,
              scenePath: nonEmptyString(ownerScenePath, detail?.host_scene_file, detail?.scene_path),
            },
          },
        ]),
      ),
      popup: {
        ...popup,
        entry: nonEmptyString(
          popup?.entry,
          popup?.entry_tab,
          popup?.entryTab,
          popup?.focus,
          boardFields?.entry,
        ),
        focus: nonEmptyString(
          popup?.entry,
          popup?.focus,
          popup?.entry_tab,
          popup?.entryTab,
          boardFields?.entry,
        ),
        params: boardFields?.params || normalizeSceneParams(popup?.params),
        projection_slots: projectionSlots,
      },
      link: {
        defaultFocus: defaultSlot?.id || tabs[0] || "",
      },
    };
  }

  function resolveDrilldownConfig(detail) {
    const metricId = String(detail?.metric_id || "").trim();
    const popup =
      detail?.popup && typeof detail.popup === "object" && !Array.isArray(detail.popup) ? detail.popup : {};
    const boardFields = resolveBoardLinkFields(popup, detail?.scene_local_nav_by_target);
    const projectionSlots = normalizeProjectionSlots(
      popup?.projection_slots || popup?.projectionSlots,
    );
    if (projectionSlots.length) {
      return resolveProjectionSlotsDrilldownConfig(detail, popup, boardFields, projectionSlots);
    }
    const runtime = runtimeDrilldownConfig(detail);
    if (!Object.keys(runtime).length) {
      return disabledDrilldownConfig(
        "missing_analysis_contract",
        `scene projection requires analysis_contract; metric \`${metricId || "unknown"}\` is missing runtime analysis_contract`,
      );
    }
    const analysisLink =
      detail?.analysis_link && typeof detail.analysis_link === "object" ? detail.analysis_link : {};
    const boardSceneId = nonEmptyString(
      detail?.board_scene_id,
      boardFields?.sceneId,
      popup?.scene_id,
      popup?.sceneId,
    );
    const hostSceneId = nonEmptyString(
      detail?.host_scene_id,
      detail?.dataset_scene_id,
      detail?.scene_id !== boardSceneId ? detail?.scene_id : "",
      runtime?.scene_id,
      runtime?.sceneId,
      resolveDrilldownSceneId(detail, runtime),
    );
    const sceneId = hostSceneId;
    const queryStateId = nonEmptyString(
      detail?.query_state_id,
      detail?.queryStateId,
      runtime?.query_state_id,
      runtime?.queryStateId,
    );
    const runtimeEnabled = boolValue(
      detail?.analysis_enabled,
      runtime?.enabled,
    );
    const explainKind = nonEmptyString(
      detail?.analysis_kind,
      detail?.explain_kind,
      runtime?.kind,
      runtime?.explain_kind,
    );
    const explainMetrics = normalizeExplainMetrics(
      detail?.explain_metrics,
      runtime?.explain_metrics,
      runtime?.explainMetrics,
      runtime?.blocks,
    );
    let detailFields = cloneArray(detail?.explain_detail_fields);
    if (!detailFields.length) detailFields = cloneArray(runtime?.detail_fields);
    if (!detailFields.length) detailFields = cloneArray(runtime?.detailFields);
    let columns = [];
    if (!columns.length) columns = cloneArray(runtime?.columns);
    if (!columns.length) columns = cloneArray(runtime?.detail_fields);
    if (!columns.length) columns = cloneArray(runtime?.detailFields);
    if (!columns.length) columns = cloneArray(detailFields);
    let headers = [];
    if (!headers.length) headers = cloneArray(runtime?.headers);
    let basisRefs = cloneArray(detail?.explain_basis_refs);
    if (!basisRefs.length) basisRefs = cloneArray(runtime?.basis_refs);
    if (!basisRefs.length) basisRefs = cloneArray(runtime?.basisRefs);
    let recommendedDimensions = cloneArray(detail?.explain_recommended_dimensions);
    if (!recommendedDimensions.length) recommendedDimensions = cloneArray(runtime?.recommended_dimensions);
    if (!recommendedDimensions.length) recommendedDimensions = cloneArray(runtime?.recommendedDimensions);
    const ratioNumerator = nonEmptyString(
      runtime?.ratio_numerator,
      runtime?.ratioNumerator,
    );
    const ratioDenominator = nonEmptyString(
      runtime?.ratio_denominator,
      runtime?.ratioDenominator,
    );
    const ratioFormula = nonEmptyString(
      runtime?.ratio_formula,
      runtime?.ratioFormula,
    );
    const tableMetricId = nonEmptyString(
      runtime?.table_metric_id,
      runtime?.tableMetricId,
      detail?.table_metric_id,
    );
    const datasetId = resolveDrilldownDatasetId(detail, {
      sceneId,
      hostSceneId,
      boardSceneId,
      tableMetricId,
      datasetId: nonEmptyString(runtime?.dataset_id, runtime?.datasetId),
    });
    const layoutPreset = nonEmptyString(
      runtime?.layout_preset,
      runtime?.layoutPreset,
    );
    const defaultSceneBindings = sceneBindingDefaults(
      boardSceneId,
      detail?.scene_bindings_by_id,
      detail?.scene_examples_by_id,
      detail?.scene_projection_assembly_by_id,
    );
    const tabMetrics = normalizeTabMetricOverrides(
      defaultSceneBindings,
      popup?.entry_overrides,
      popup?.bindings,
      popup?.entryOverrides,
      panelPopupSlotSources(popup),
      popup?.metrics,
      detail?.analysis_tab_metrics,
      runtime?.analysis_tab_metrics,
      runtime?.tab_metrics,
      runtime?.tabMetrics,
    );
    const panelPopup = Boolean(boardFields?.panelPopup) || isPanelPopupConfig(popup);
    const boardLink = Boolean(boardFields?.boardLink);
    const panelTemplate = panelPopup
      ? normalizePanelTemplateId(nonEmptyString(popup?.template, boardFields?.legacyTemplate))
      : "";
    const boardSceneFile = nonEmptyString(
      detail?.board_scene_file,
      boardFields?.sceneFile,
      popup?.scene_file,
      popup?.sceneFile,
    );
    const sceneAssembly = sceneProjectionAssembly(boardSceneId, detail?.scene_projection_assembly_by_id);
    const sceneLocalNav =
      boardFields?.localNav ||
      normalizeSceneLocalNav(popup?.local_nav || popup?.localNav) ||
      normalizeSceneLocalNav(sceneAssembly?.local_nav || sceneAssembly?.localNav) ||
      resolveSceneLocalNav(boardSceneFile, detail?.scene_local_nav_by_target) ||
      null;
    const sceneShell = resolveSceneShell(sceneAssembly);
    const projection = normalizeProjection(
      nonEmptyString(detail?.projection, popup?.projection, boardFields?.projection, "overlay"),
    );
    const hasDetail = Boolean(
      tableMetricId ||
        columns.length ||
        detailFields.length ||
        nonEmptyString(
          detail?.explain_detail_dataset,
          runtime?.dataset_id,
          runtime?.datasetId,
        ),
    );
    const tabs = resolveDrilldownTabs({
      detail,
      runtime,
      explainKind,
      hasDetail,
      localNav: sceneLocalNav,
    });
    const ratioNote = buildRatioExplainNote({
      numerator: ratioNumerator,
      denominator: ratioDenominator,
      formula: ratioFormula,
    });
    const structuredBoard = Boolean(sceneShell?.layoutMode) && sceneShell.layoutMode !== "generic_tabs";
    const overlaySize = resolveDrilldownOverlaySize({ popup, boardFields, structuredBoard, sceneShell });
    const legacyParams = boardFields?.params || normalizeSceneParams(popup?.params);
    const legacyFilterSchema = normalizeAnalyticsFilterSchema(popup?.filter_schema || popup?.filterSchema);
    return {
      enabled:
        (boardLink && Boolean(boardSceneId)) ||
        (panelPopup && Boolean(boardSceneId) && Boolean(panelTemplate)) ||
        popup?.mode === "popup" ||
        (runtimeEnabled !== false && Boolean(hostSceneId || boardSceneId)),
      genericDrilldown: !structuredBoard,
      structuredBoard,
      sceneShell,
      hasChartZone: false,
      hasRowPreviewZone: false,
      overlaySize,
      sceneId,
      hostSceneId,
      hostSceneFile: nonEmptyString(detail?.host_scene_file, detail?.scene_path),
      queryStateId,
      params: legacyParams,
      filterSchema: legacyFilterSchema,
      rowsetDatasetId: nonEmptyString(
        legacyFilterSchema.rowsetDatasetId,
        sceneParamRowsetDatasetId(legacyParams),
      ),
      boardSceneId,
      boardLink,
      boardSceneFile,
      sceneLocalNav,
      projection,
      panelPopup,
      panelTemplate,
      panelTitle: nonEmptyString(popup?.title),
      title: nonEmptyString(
        popup?.title,
        detail?.explain_title,
        runtime?.title,
        detail?.label,
        metricId,
        "指标明细",
      ),
      note: nonEmptyString(
        runtime?.note,
        detail?.explain_note,
        detail?.analysis_note,
        ratioNote,
      ),
      tableMetricId,
      datasetId,
      columns,
      headers,
      detailFields,
      basisRefs,
      recommendedDimensions,
      ratioParts: {
        numerator: ratioNumerator,
        denominator: ratioDenominator,
        formula: ratioFormula,
      },
      compositionBy: cloneArray(detail?.explain_composition_by).length
        ? cloneArray(detail?.explain_composition_by)
        : cloneArray(runtime?.composition_by).length
          ? cloneArray(runtime?.composition_by)
          : cloneArray(recommendedDimensions),
      trendField: nonEmptyString(runtime?.trend_field, runtime?.trendField, detail?.explain_trend_field),
      trendGrain: nonEmptyString(runtime?.trend_grain, runtime?.trendGrain, detail?.explain_trend_grain, "month"),
      layoutPreset,
      explainKind,
      explainMetrics: explainMetrics.byId,
      explainMetricOrder: explainMetrics.order,
      tabs,
      tabMetrics,
      link: {
        mode: nonEmptyString(analysisLink.mode),
        template: nonEmptyString(analysisLink.template),
        entry: nonEmptyString(analysisLink.entry),
        defaultFocus: nonEmptyString(analysisLink.default_focus, analysisLink.defaultFocus),
      },
      popup: {
        mode: nonEmptyString(
          popup?.mode,
          boardLink ? "board_link" : panelPopup ? "popup_panel" : "popup",
        ),
        template: nonEmptyString(panelTemplate, popup?.template, popup?.legacy_template),
        entry: nonEmptyString(
          popup?.entry,
          popup?.entry_tab,
          popup?.entryTab,
          popup?.focus,
          boardFields?.entry,
        ),
        focus: nonEmptyString(popup?.entry, popup?.focus, popup?.entry_tab, popup?.entryTab, boardFields?.entry),
        scene_file: boardSceneFile,
        scene_id: boardSceneId,
        scene: boardFields?.sceneRef || popup?.scene || null,
        projection,
        local_nav: sceneLocalNav,
        params: boardFields?.params || normalizeSceneParams(popup?.params),
        entry_overrides: panelPopupSlotSources(popup),
        slots: panelPopupSlotSources(popup),
      },
      chartKind: nonEmptyString(runtime?.chart_kind, runtime?.chartKind),
      mapping: runtime?.mapping && typeof runtime.mapping === "object" ? runtime.mapping : null,
      pageSize:
        positiveInt(
          runtime?.page_size,
          runtime?.pageSize,
          8,
        ) || 8,
      cellPreviewMaxChars:
        positiveInt(
          runtime?.cell_preview_max_chars,
          runtime?.cellPreviewMaxChars,
        ) > 0
          ? positiveInt(
              runtime?.cell_preview_max_chars,
              runtime?.cellPreviewMaxChars,
            )
          : 0,
      columnMinWidth:
        positiveInt(
          runtime?.column_min_width,
          runtime?.columnMinWidth,
        ) > 0
          ? positiveInt(
              runtime?.column_min_width,
              runtime?.columnMinWidth,
            )
          : 0,
    };
  }

  function resolveAppPathByPrefixes(pathname, prefixes) {
    const raw = String(pathname || "");
    if (!raw || !Array.isArray(prefixes)) return "";
    for (const prefix of prefixes) {
      const normalizedPrefix = String(prefix || "");
      if (!normalizedPrefix || !raw.startsWith(normalizedPrefix)) continue;
      const tail = raw.slice(normalizedPrefix.length);
      const slash = tail.indexOf("/");
      const app = slash >= 0 ? tail.slice(0, slash) : tail;
      const trimmed = String(app || "").trim();
      if (trimmed) return trimmed;
    }
    return "";
  }

  function resolveAccessAppPath(pathname = window.location.pathname) {
    return resolveAppPathByPrefixes(pathname, appRoutePrefixesFromSlugs(ACCESS_LIKE_ROUTE_SLUGS));
  }

  function resolvePreviewAppId(pathname = window.location.pathname) {
    const slug = appRouteSlugFromPathname(pathname);
    if (ACCESS_LIKE_ROUTE_SLUGS.has(slug) || BUILD_ROUTE_SLUGS.has(slug)) {
      return resolveAppPathByPrefixes(pathname, [`/apps/${slug}/`]);
    }
    return resolveAppPathByPrefixes(pathname, ["/apps/upload/", "/apps/config/"]);
  }

  function resolvePopupDebugHost() {
    try {
      if (window.parent && window.parent !== window) {
        const host = window.parent.document.getElementById("mei-runtime-query-errors");
        if (host) return host;
      }
    } catch (_) {
      /* ignore */
    }
    return document.getElementById("mei-runtime-query-errors");
  }

  function recordPopupDebugIssue({
    level = "error",
    message = "",
    phase = "",
    detail = {},
    config = {},
    datasetId = "",
    metricId = "",
  } = {}) {
    const payload = {
      phase: String(phase || "").trim(),
      message: String(message || "").trim(),
      sceneId: nonEmptyString(config?.sceneId, detail?.scene_id),
      target: nonEmptyString(config?.runtimeRef?.scenePath, detail?.scene_path),
      datasetId: String(datasetId || "").trim(),
      metricId: String(metricId || "").trim(),
      template: nonEmptyString(config?.panelTemplate, config?.popup?.template),
    };
    const logger = level === "warn" ? console.warn : console.error;
    logger("[mei][popup-panel]", payload);
    const host = resolvePopupDebugHost();
    if (!(host instanceof HTMLElement)) return;
    const tone =
      level === "warn"
        ? "rgba(250, 204, 21, .24);border:1px solid rgba(250, 204, 21, .45);color:#fde68a;"
        : "rgba(127, 29, 29, .18);border:1px solid rgba(248, 113, 113, .4);color:#fecaca;";
    const context = [
      payload.phase ? `phase=${payload.phase}` : "",
      payload.sceneId ? `scene=${payload.sceneId}` : "",
      payload.target ? `file=${payload.target}` : "",
      payload.datasetId ? `dataset=${payload.datasetId}` : "",
      payload.metricId ? `metric=${payload.metricId}` : "",
      payload.template ? `template=${payload.template}` : "",
    ]
      .filter(Boolean)
      .join(" · ");
    host.insertAdjacentHTML(
      "afterbegin",
      `<div style="display:block;margin:6px 0;padding:8px;border-radius:8px;${tone}font-size:11px;line-height:1.45;">` +
        `<strong>scene_projection</strong>${context ? ` · ${context}` : ""}<br/>` +
        `<code style="display:block;margin-top:4px;white-space:pre-wrap;word-break:break-word;color:inherit;">${String(
          payload.message || "unknown popup error"
        )
          .replaceAll("&", "&amp;")
          .replaceAll("<", "&lt;")
          .replaceAll(">", "&gt;")}</code></div>`
    );
  }

  async function fetchPopupDatasetRows(detail, config, datasetId) {
    const appPath = resolvePreviewAppId();
    const runtimeRefConfig = config?.runtimeRef && typeof config.runtimeRef === "object" ? config.runtimeRef : {};
    const sceneId = nonEmptyString(
      runtimeRefConfig.sceneId,
      config?.hostSceneId,
      config?.sceneId,
      detail?.host_scene_id,
      detail?.scene_id,
    );
    const target = nonEmptyString(
      runtimeRefConfig.scenePath,
      config?.hostSceneFile,
      resolveMetricOwnerScenePath(
        config?.slotByTab ? Object.values(config.slotByTab) : [],
        detail,
      ),
      detail?.host_scene_file,
      detail?.scene_path,
    );
    if (!appPath || !sceneId || !datasetId) {
      recordPopupDebugIssue({
        level: "error",
        message: "缺少 popup panel 数据查询所需的 app / scene / dataset 参数",
        phase: "dataset_fetch_setup",
        detail,
        config,
        datasetId,
      });
      return null;
    }
    const runtimeQuery = window.__meiDatasetRuntime;
    const queryStateId = nonEmptyString(config?.queryStateId, detail?.query_state_id, detail?.queryStateId);
    const sharedFilters =
      runtimeQuery &&
      typeof runtimeQuery.sharedFiltersForQueryStateId === "function" &&
      queryStateId
        ? runtimeQuery.sharedFiltersForQueryStateId(queryStateId)
        : {};
    const mergedFilters = {};
    if (sharedFilters && typeof sharedFilters === "object" && !Array.isArray(sharedFilters)) {
      Object.entries(sharedFilters).forEach(([key, value]) => {
        const normalizedKey = String(key || "").trim();
        const normalizedValue = String(value ?? "").trim();
        if (!normalizedKey || !normalizedValue) return;
        mergedFilters[normalizedKey] = normalizedValue;
      });
    }
    if (runtimeQuery && typeof runtimeQuery.fetchDatasetRows === "function") {
      try {
        const result = await runtimeQuery.fetchDatasetRows(
          {
            data: {
              id: String(datasetId || "").trim(),
              __mei_runtime_ref: {
                kind: "data",
                dataset_id: String(datasetId || "").trim(),
                scene_id: sceneId,
                scene_path: target,
              },
            },
            _mei: {
              runtime_capabilities: {
                rows_query: {
                  enabled: true,
                  api: `/api/datasets/query/${appPath}`,
                  scene_qualified: true,
                },
              },
              active_scene_id: sceneId,
              active_target_file: target,
              entry_target: target,
            },
          },
          {
            page: 1,
            pageSize: 100000,
            queryStateId,
            filters: mergedFilters,
            full: true,
            summary: true,
            meta: {
              component: "mei-popup-panel",
              panel_id: String(config?.panelId || "drilldown"),
              scene_id: sceneId,
              target,
              query_state_id: queryStateId || undefined,
              filter_intent_source: "drilldown",
            },
          }
        );
        if (result) {
          return {
            rows: Array.isArray(result?.rows) ? result.rows : [],
            columns: Array.isArray(result?.columns) ? result.columns : [],
            column_meta: Array.isArray(result?.column_meta) ? result.column_meta : [],
            summary: result?.summary || null,
            query_state_echo: result?.query_state_echo || null,
          };
        }
      } catch (error) {
        recordPopupDebugIssue({
          level: "error",
          message: String(error?.message || error || "popup panel runtime-query fetch failed"),
          phase: "dataset_fetch_runtime_query",
          detail,
          config,
          datasetId,
        });
        throw error;
      }
    }
    const message =
      "popup panel dataset fetch requires shared runtime-query.js; raw dataset fetch fallback has been removed";
    recordPopupDebugIssue({
      level: "error",
      message,
      phase: "dataset_fetch_runtime_missing",
      detail,
      config,
      datasetId,
    });
    throw new Error(message);
  }

  async function fetchPopupDrilldownRows(detail, config) {
    const tableMetricId = resolveCompositionMetricId(config, detail);
    const scopedConfig = tableMetricId ? { ...config, tableMetricId } : config;
    const tableProps = buildDrilldownTableProps(detail, scopedConfig);
    const runtimeQuery = window.__meiDatasetRuntime;
    const queryStateId = nonEmptyString(config?.queryStateId, detail?.query_state_id, detail?.queryStateId);
    if (tableMetricId && tableProps && runtimeQuery && typeof runtimeQuery.fetchDatasetRows === "function") {
      try {
        const result = await runtimeQuery.fetchDatasetRows(
          {
            dataset: tableProps.dataset,
            _mei: tableProps._mei,
          },
          {
            page: 1,
            pageSize: 100000,
            queryStateId,
            full: true,
            summary: true,
            meta: {
              component: "mei-popup-panel",
              phase: "derived_metric_rowset",
              query_state_id: queryStateId || undefined,
              filter_intent_source: "drilldown",
            },
          },
        );
        if (result) {
          return {
            rows: Array.isArray(result?.rows) ? result.rows : [],
            columns: Array.isArray(result?.columns) ? result.columns : [],
            column_meta: Array.isArray(result?.column_meta) ? result.column_meta : [],
            summary: result?.summary || null,
            query_state_echo: result?.query_state_echo || null,
          };
        }
      } catch (error) {
        recordPopupDebugIssue({
          level: "error",
          message: String(error?.message || error || "popup panel metric rowset fetch failed"),
          phase: "derived_metric_rowset_fetch",
          detail,
          config,
          metricId: tableMetricId,
        });
        throw error;
      }
    }
    if (config?.hasChartZone && tableMetricId) {
      recordPopupDebugIssue({
        level: "error",
        message: "分析型构成图需要 metric 行集，但未能通过 metric 查询拿到明细行",
        phase: "derived_metric_rowset_required",
        detail,
        config: scopedConfig,
        metricId: tableMetricId,
      });
      return { rows: [], columns: [], column_meta: [], summary: null, query_state_echo: null };
    }
    const datasetId = resolveDrilldownDatasetId(detail, scopedConfig);
    return fetchPopupDatasetRows(detail, { ...scopedConfig, datasetId }, datasetId);
  }

  function monthBucketLabel(value) {
    const raw = String(value || "").trim();
    if (!raw) return "";
    const match = raw.match(/^(\d{4})[-/年](\d{1,2})/);
    if (match) {
      return `${match[1]}-${String(match[2]).padStart(2, "0")}`;
    }
    return raw.slice(0, 7);
  }

  function parseCompositionNumber(value) {
    const parsed = Number(String(value ?? "").replace(/,/g, "").trim());
    return Number.isFinite(parsed) ? parsed : 0;
  }

  function compositionWeightField(columns = [], rows = []) {
    const names = Array.isArray(columns) ? columns : [];
    if (names.includes("预警条数")) return "预警条数";
    return rows.some((row) => row && typeof row === "object" && Object.prototype.hasOwnProperty.call(row, "预警条数"))
      ? "预警条数"
      : "";
  }

  function groupRowsByCount(rows, field, columns = []) {
    const grouped = new Map();
    rows.forEach((row) => {
      const key = String(rowFieldValue(row, field, columns) ?? "").trim() || "未标注";
      grouped.set(key, (grouped.get(key) || 0) + 1);
    });
    return Array.from(grouped.entries())
      .map(([label, value]) => ({ label, value }))
      .sort((a, b) => Number(b.value || 0) - Number(a.value || 0));
  }

  function groupRowsByWeightedSum(rows, field, weightField, columns = []) {
    const grouped = new Map();
    rows.forEach((row) => {
      const key = String(rowFieldValue(row, field, columns) ?? "").trim() || "未标注";
      const weight = parseCompositionNumber(rowFieldValue(row, weightField, columns));
      grouped.set(key, (grouped.get(key) || 0) + (weight > 0 ? weight : 1));
    });
    return Array.from(grouped.entries())
      .map(([label, value]) => ({ label, value }))
      .sort((a, b) => Number(b.value || 0) - Number(a.value || 0));
  }

  function normalizeMetricLocalId(metricId) {
    const text = String(metricId || "").trim();
    if (!text) return "";
    const parts = text.split("::");
    if (parts.length >= 2 && parts[parts.length - 1] === "__scalar_rowset__") {
      return parts[parts.length - 2];
    }
    return parts.length >= 2 ? parts[parts.length - 2] : text;
  }

  function resolveCompositionMetricId(config, detail = null) {
    return nonEmptyString(
      config?.tableMetricId,
      config?.runtimeRef?.metricId,
      config?.runtimeRef?.metric_id,
      detail?.table_metric_id,
    );
  }

  function compositionUsesWarningCountSum(config, detail = null) {
    return normalizeMetricLocalId(resolveCompositionMetricId(config, detail)) === "warnings_count";
  }

  function groupRowsForComposition(rows, field, columns = [], config = null, detail = null) {
    if (compositionUsesWarningCountSum(config, detail) && compositionWeightField(columns, rows)) {
      return groupRowsByWeightedSum(rows, field, "预警条数", columns);
    }
    return groupRowsByCount(rows, field, columns);
  }

  function limitCompositionRows(rows, config = null) {
    const topN = positiveInt(config?.top_n, config?.topN, config?.topN);
    if (!Array.isArray(rows) || topN <= 0 || rows.length <= topN) {
      return rows;
    }
    return rows.slice(0, topN);
  }

  function groupRowsByMonth(rows, field, columns = []) {
    const grouped = new Map();
    rows.forEach((row) => {
      const key = monthBucketLabel(rowFieldValue(row, field, columns));
      if (!key) return;
      grouped.set(key, (grouped.get(key) || 0) + 1);
    });
    return Array.from(grouped.entries())
      .map(([month, value]) => ({ month, value }))
      .sort((a, b) => String(a.month).localeCompare(String(b.month)));
  }

  function buildStaticTablePropsFromRows(title, columns, rows) {
    return {
      title: String(title || ""),
      data: {
        title: String(title || ""),
        columns: Array.isArray(columns) ? columns : [],
        rows: Array.isArray(rows) ? rows : [],
      },
    };
  }

  function isAnalyticsChartPresentation(config) {
    return (
      Boolean(config?.hasChartZone) ||
      (Array.isArray(config?.chartSlots) && config.chartSlots.length > 0)
    );
  }

  function buildAnalyticsChartPresentationProps(config = null, overrides = {}) {
    if (!isAnalyticsChartPresentation(config)) {
      return { ...overrides };
    }
    const topN = positiveInt(
      overrides?.top_n,
      overrides?.topN,
      config?.top_n,
      config?.topN,
    );
    const props = {
      compact: true,
      gridContainLabel: true,
      label_max_chars: 6,
      showLegend: false,
      chartHeight: 300,
      color_palette: ["#38bdf8", "#34d399", "#f59e0b", "#a78bfa", "#f87171", "#facc15", "#22d3ee", "#fb7185"],
      ...overrides,
    };
    if (topN > 0) {
      props.top_n = topN;
    } else {
      delete props.top_n;
    }
    delete props.topN;
    return props;
  }

  function buildStaticChartModel(title, tabId, rows, mapping = null, config = null) {
    const normalized = normalizeTabId(tabId);
    const data = {
      columns: Array.isArray(rows) && rows.length > 0 ? Object.keys(rows[0]) : [],
      rows: Array.isArray(rows) ? rows : [],
    };
    const defaultMapping =
      normalized === "trend"
        ? { x: "month", y: "value" }
        : { x: "label", y: "value" };
    return {
      title: String(title || ""),
      data,
      mapping: mapping && typeof mapping === "object" ? mapping : defaultMapping,
      ...buildAnalyticsChartPresentationProps(config),
    };
  }

  function buildDrilldownTableProps(detail, config) {
    const runtimeRefConfig = config?.runtimeRef && typeof config.runtimeRef === "object" ? config.runtimeRef : {};
    const queryStateId = nonEmptyString(config?.queryStateId, detail?.query_state_id, detail?.queryStateId);
    const sceneId = nonEmptyString(
      runtimeRefConfig.sceneId,
      config?.hostSceneId,
      config?.sceneId,
      detail?.host_scene_id,
      detail?.scene_id,
      resolveDrilldownSceneId(detail, runtimeDrilldownConfig(detail)),
    );
    if (!sceneId) return null;
    const appPath = resolvePreviewAppId();
    if (!appPath) return null;
    const ownerScenePath = nonEmptyString(
      runtimeRefConfig.scenePath,
      config?.hostSceneFile,
      resolveMetricOwnerScenePath(
        config?.slotByTab ? Object.values(config.slotByTab) : [],
        detail,
      ),
      detail?.host_scene_file,
      detail?.scene_path,
    );
    const datasetId = resolveDrilldownDatasetId(detail, config);
    if (!datasetId) return null;
    const metricId = nonEmptyString(runtimeRefConfig.metricId, config?.tableMetricId);
    const runtimeRef = metricId
      ? {
          kind: "metric",
          scene_id: sceneId,
          scene_path: ownerScenePath,
          dataset_id: datasetId,
          metric_id: metricId,
        }
      : {
          kind: "data",
          scene_id: sceneId,
          scene_path: ownerScenePath,
          dataset_id: datasetId,
        };
    const columns = Array.isArray(config?.columns) ? config.columns : [];
    const tableScrollX =
      config?.tableScrollX === true ||
      config?.table_scroll_x === true ||
      columns.length >= 7;
    const inferredFormats = inferDrilldownColumnFormats(columns);
    const inferredColumnState = inferDrilldownColumnState(columns);
    const columnFormats =
      config?.columnFormats && typeof config.columnFormats === "object"
        ? { ...inferredFormats, ...config.columnFormats }
        : inferredFormats;
    const columnState =
      config?.columnState && typeof config.columnState === "object"
        ? config.columnState
        : config?.column_state && typeof config.column_state === "object"
          ? config.column_state
          : inferredColumnState;
    const columnMinWidth =
      Number(config?.columnMinWidth) > 0
        ? Number(config.columnMinWidth)
        : tableScrollX
          ? 88
          : 56;
    const drilldownFilters =
      detail?.drilldown_filters && typeof detail.drilldown_filters === "object" && !Array.isArray(detail.drilldown_filters)
        ? detail.drilldown_filters
        : detail?.default_filters && typeof detail.default_filters === "object" && !Array.isArray(detail.default_filters)
          ? detail.default_filters
          : null;
    return {
      columns,
      headers: Array.isArray(config?.headers) && config.headers.length > 0 ? config.headers : undefined,
      column_state: columnState,
      layoutPreset: tableScrollX ? "" : config?.layoutPreset || "default",
      default_filters: drilldownFilters || undefined,
      embedded: true,
      rowSelectionMode: nonEmptyString(config?.rowSelectionMode),
      tableScrollX,
      autoFitColumns: true,
      fitColumnsFromSample: true,
      columnWidthSampleSize: 100,
      cellOverflowMinChars: 10,
      pageSize: Number(config?.pageSize) > 0 ? Number(config.pageSize) : 8,
      cellPreviewMaxChars:
        Number(config?.cellPreviewMaxChars) > 0
          ? Number(config.cellPreviewMaxChars)
          : tableScrollX
            ? 20
            : 28,
      columnMinWidth,
      columnFormats,
      pagination: true,
      paginationMode: "client",
      dataset: {
        shape: metricId ? "dataframe" : "table",
        __mei_runtime_ref: runtimeRef,
      },
      _mei: {
        runtime_capabilities: {
          rows_query: {
            enabled: true,
            api: `/api/datasets/query/${appPath}`,
            scene_qualified: true,
          },
          metric_query: {
            enabled: true,
            api: `/api/datasets/metrics/${appPath}`,
            scene_qualified: true,
          },
          metric_batch_query: {
            enabled: true,
            api: `/api/datasets/metrics/${appPath}`,
            scene_qualified: true,
          },
        },
        active_scene_id: sceneId,
        active_target_file: ownerScenePath,
        entry_target: ownerScenePath,
      },
      query_state: queryStateId || undefined,
    };
  }

  function drilldownChartTag(chartKind, tabId) {
    const explicit = String(chartKind || "").trim().toLowerCase();
    const fallback = normalizeTabId(tabId) === "trend" ? "line" : "bar";
    const kind = explicit || fallback;
    const supported = new Set([
      "line",
      "area",
      "trend",
      "column",
      "bar",
      "scatter",
      "pie",
      "donut",
      "rose",
      "radar",
      "ranking",
      "boxplot",
    ]);
    if (!supported.has(kind)) return "";
    return `mei-chart-${kind}`;
  }

  const DRILLDOWN_CHART_SCRIPT_BY_TAG = {
    "mei-chart-line": "/workspace-components/chart/echarts/line.js",
    "mei-chart-area": "/workspace-components/chart/echarts/area.js",
    "mei-chart-trend": "/workspace-components/chart/echarts/trend.js",
    "mei-chart-column": "/workspace-components/chart/echarts/column.js",
    "mei-chart-bar": "/workspace-components/chart/echarts/bar.js",
    "mei-chart-scatter": "/workspace-components/chart/echarts/scatter.js",
    "mei-chart-pie": "/workspace-components/chart/echarts/pie.js",
    "mei-chart-donut": "/workspace-components/chart/echarts/donut.js",
    "mei-chart-rose": "/workspace-components/chart/echarts/rose.js",
    "mei-chart-radar": "/workspace-components/chart/echarts/radar.js",
    "mei-chart-ranking": "/workspace-components/chart/echarts/ranking.js",
    "mei-chart-boxplot": "/workspace-components/chart/echarts/boxplot.js",
  };

  const DRILLDOWN_TABLE_SCRIPT = "/workspace-components/cockpit/data-table.js";
  const DRILLDOWN_FILTER_BAR_SCRIPT = "/workspace-components/dataset/filter-bar.js";

  async function ensureDrilldownChartRegistered(tagName) {
    const tag = String(tagName || "").trim().toLowerCase();
    if (!tag) return false;
    if (customElements.get(tag)) return true;
    const scriptPath = DRILLDOWN_CHART_SCRIPT_BY_TAG[tag];
    if (!scriptPath) return false;
    await loadScript(scriptPath, {
      module: true,
      persistentKey: scriptPath,
      softFail: false,
    });
    return Boolean(customElements.get(tag));
  }

  async function ensureDrilldownTableRegistered() {
    const tag = "mei-cockpit-data-table";
    if (customElements.get(tag)) return true;
    await loadScript(DRILLDOWN_TABLE_SCRIPT, {
      module: true,
      persistentKey: DRILLDOWN_TABLE_SCRIPT,
      softFail: false,
    });
    return Boolean(customElements.get(tag));
  }

  async function ensureDrilldownFilterBarRegistered() {
    const tag = "mei-dataset-filter-bar";
    if (customElements.get(tag)) return true;
    await loadScript(DRILLDOWN_FILTER_BAR_SCRIPT, {
      module: true,
      persistentKey: DRILLDOWN_FILTER_BAR_SCRIPT,
      softFail: false,
    });
    return Boolean(customElements.get(tag));
  }

  function buildDrilldownChartProps(detail, config, tabId) {
    const tableProps = buildDrilldownTableProps(detail, config);
    if (!tableProps) return null;
    const chartTag = drilldownChartTag(config?.chartKind, tabId);
    if (!chartTag) return null;
    const columns = Array.isArray(config?.columns) ? config.columns : [];
    const normalizedKind = explainMetricKind(config, tabId);
    const compositionField = nonEmptyString(
      Array.isArray(config?.compositionBy) ? config.compositionBy[0] : "",
      columns[0],
      "label",
    );
    const xField =
      normalizedKind === "trend" ? "month" : normalizedKind === "composition" ? compositionField : columns[0] || "label";
    const yField = "value";
    const mapping =
      config?.mapping && typeof config.mapping === "object"
        ? config.mapping
        : {
            x: xField,
            y: yField,
          };
    return {
      chartTag,
      props: {
        title: String(config?.title || ""),
        data: tableProps.dataset,
        _mei: tableProps._mei,
        query_state: tableProps.query_state,
        mapping,
        ...buildAnalyticsChartPresentationProps(config),
      },
    };
  }

  async function mountAnalyticsChartSlot(root, detail, config, tabId, hostOverride = null) {
    const kind = explainMetricKind(config, tabId);
    const supportRole = nonEmptyString(config?.supportRole, config?.slotByTab?.[normalizeTabId(tabId)]?.supportRole);
    // 构成图需要明细行在前端按维度聚合；metric KPI 查询只返回标量，不能直接驱动图表。
    if (kind === "composition" || supportRole === "composition") {
      if (await mountDerivedDrilldownContent(root, detail, config, tabId, hostOverride)) {
        return true;
      }
    }
    return mountDrilldownChart(root, detail, config, tabId, hostOverride);
  }

  async function mountDrilldownChart(root, detail, config, tabId, hostOverride = null) {
    const host =
      hostOverride instanceof HTMLElement
        ? hostOverride
        : root.querySelector('[data-drilldown-table-host="true"]');
    if (!(host instanceof HTMLElement)) {
      return false;
    }
    const chart = buildDrilldownChartProps(detail, config, tabId);
    if (!chart) return false;
    const registered = await ensureDrilldownChartRegistered(chart.chartTag);
    if (!registered) return false;
    host.replaceChildren();
    const node = document.createElement(chart.chartTag);
    node.dataset.props = JSON.stringify(chart.props);
    host.appendChild(node);
    return true;
  }

  async function mountDrilldownTable(root, detail, config, hostOverride = null) {
    const host =
      hostOverride instanceof HTMLElement
        ? hostOverride
        : root.querySelector('[data-drilldown-table-host="true"]');
    if (!(host instanceof HTMLElement)) {
      return false;
    }
    const props = buildDrilldownTableProps(detail, config);
    if (!props) {
      recordPopupDebugIssue({
        level: "error",
        message: "未解析到下钻明细表所需 scene_id 或 dataset_id",
        phase: "table_mount_setup",
        detail,
        config,
        datasetId: resolveDrilldownDatasetId(detail, config),
        metricId: nonEmptyString(detail?.metric_id, detail?.__mei_runtime_ref?.metric_id),
      });
      return false;
    }
    const registered = await ensureDrilldownTableRegistered();
    if (!registered) {
      recordPopupDebugIssue({
        level: "error",
        message: "未注册 mei-cockpit-data-table（可能是组件脚本加载失败）",
        phase: "table_mount_register",
        detail,
        config,
        datasetId: resolveDrilldownDatasetId(detail, config),
        metricId: nonEmptyString(detail?.metric_id, detail?.__mei_runtime_ref?.metric_id),
      });
      return false;
    }
    host.replaceChildren();
    const table = document.createElement("mei-cockpit-data-table");
    table.dataset.props = JSON.stringify(props);
    host.appendChild(table);
    return true;
  }

  function buildAnalyticsFilterBarProps(config, detail) {
    const fields = Array.isArray(config?.filterSchema?.fields) ? config.filterSchema.fields : [];
    const tableProps = buildDrilldownTableProps(detail, config) || {};
    const rowsetDatasetId = nonEmptyString(
      config?.filterSchema?.rowsetDatasetId,
      tableProps?.dataset?.__mei_runtime_ref?.dataset_id,
      tableProps?.dataset?.id,
    );
    const listPreview = Boolean(config?.hasRowPreviewZone);
    return {
      title: "筛选条件",
      description: listPreview
        ? "调整条件后清单与预览将同步刷新。"
        : "调整条件后图表与明细表将同步刷新。",
      live: true,
      query_state: config?.queryStateId || undefined,
      rowset_dataset_id: rowsetDatasetId || undefined,
      dataset: rowsetDatasetId
        ? {
            id: rowsetDatasetId,
            shape: "table",
            __mei_runtime_ref: {
              dataset_id: rowsetDatasetId,
              scene_id: nonEmptyString(config?.hostSceneId, config?.sceneId),
            },
          }
        : tableProps.dataset,
      data: rowsetDatasetId ? { id: rowsetDatasetId } : tableProps.dataset,
      _mei: tableProps._mei,
      fields: fields.map((field) => {
        const column = nonEmptyString(field.column, field.key);
        const control = nonEmptyString(field.control, "text");
        const needsRowsetOptions = control === "multi_select" || control === "month_multi_select";
        return {
          key: column,
          label: field.label || field.key || column,
          column,
          control,
          options_from: needsRowsetOptions ? "rowset" : "",
          options_field: column,
        };
      }),
    };
  }

  async function mountAnalyticsFilterBar(root, detail, config, hostOverride = null) {
    const host =
      hostOverride instanceof HTMLElement
        ? hostOverride
        : root.querySelector('[data-drilldown-filter-host="true"]');
    if (!(host instanceof HTMLElement)) return false;
    const filterProps = buildAnalyticsFilterBarProps(config, detail);
    const fieldCount = Array.isArray(filterProps?.fields) ? filterProps.fields.length : 0;
    host.toggleAttribute("hidden", fieldCount === 0);
    if (fieldCount === 0) {
      host.replaceChildren();
      return false;
    }
    const registered = await ensureDrilldownFilterBarRegistered();
    if (!registered) return false;
    host.replaceChildren();
    const node = document.createElement("mei-dataset-filter-bar");
    node.dataset.props = JSON.stringify(filterProps);
    host.appendChild(node);
    return true;
  }

  function cleanupAnalyticsDrilldownWatcher(root) {
    if (!(root instanceof HTMLElement)) return;
    const cleanup = root.__meiAnalyticsQueryStateCleanup;
    if (typeof cleanup === "function") {
      cleanup();
    }
    root.__meiAnalyticsQueryStateCleanup = null;
  }

  async function mountAnalyticsChartSlots(root, detail, config, chartSlots, chartsHost) {
    const chartMounts = chartSlots.map(async (slot, index) => {
      const slotHost = chartsHost.querySelector(`[data-chart-slot-index="${index}"]`);
      const slotConfig = resolveDrilldownTabConfig(config, slot.id);
      const boardMetricId = nonEmptyString(config.tableMetricId);
      const mergedConfig = {
        ...slotConfig,
        hasChartZone: config.hasChartZone,
        rowsetDatasetId: config.rowsetDatasetId,
        hostSceneId: config.hostSceneId,
        hostSceneFile: config.hostSceneFile,
        queryStateId: config.queryStateId,
        tableMetricId: nonEmptyString(slot.metricId, slotConfig.tableMetricId, boardMetricId),
        chartKind: nonEmptyString(slot.chartKind, slotConfig.chartKind),
      };
      if (await mountAnalyticsChartSlot(root, detail, mergedConfig, slot.id, slotHost)) {
        return true;
      }
      const fallbackRuntimeRef = {
        ...(mergedConfig.runtimeRef && typeof mergedConfig.runtimeRef === "object"
          ? mergedConfig.runtimeRef
          : {}),
        kind: "metric",
        metricId: nonEmptyString(slot.metricId, mergedConfig?.runtimeRef?.metricId, mergedConfig?.runtimeRef?.metric_id),
        datasetId: nonEmptyString(slot.datasetId, mergedConfig?.runtimeRef?.datasetId, mergedConfig?.runtimeRef?.dataset_id),
        sceneId: nonEmptyString(mergedConfig?.runtimeRef?.sceneId, config.hostSceneId, config.sceneId),
        scenePath: nonEmptyString(
          mergedConfig?.runtimeRef?.scenePath,
          config.hostSceneFile,
          detail?.host_scene_file,
          detail?.scene_path
        ),
      };
      const fallbackConfig = {
        ...mergedConfig,
        supportRole: nonEmptyString(slot.supportRole, mergedConfig.supportRole, "composition"),
        tableMetricId: nonEmptyString(slot.metricId, mergedConfig.tableMetricId),
        datasetId: nonEmptyString(slot.datasetId, mergedConfig.datasetId),
        runtimeRef: fallbackRuntimeRef,
        compositionBy:
          Array.isArray(slot.by) && slot.by.length > 0
            ? slot.by
            : Array.isArray(mergedConfig.compositionBy)
              ? mergedConfig.compositionBy
              : [],
      };
      const fallbackMounted = await mountDerivedDrilldownContent(
        root,
        detail,
        fallbackConfig,
        slot.id,
        slotHost
      );
      if (!fallbackMounted) {
        recordPopupDebugIssue({
          level: "warn",
          phase: "analytics_chart_mount_fallback_failed",
          message: `chart slot ${String(slot.id || "").trim() || "unknown"} mount returned false`,
          detail,
          config: fallbackConfig,
          datasetId: fallbackConfig.datasetId,
          metricId: fallbackConfig.tableMetricId,
        });
      }
      return fallbackMounted;
    });
    const results = await Promise.all(chartMounts);
    return chartSlots.length === 0 || results.every(Boolean);
  }

  async function renderAnalyticsDrilldownContent(root, detail, config) {
    applyDrilldownOverlayMeta(root, config);
    setDrilldownOverlayStatus(root, "loading");
    cleanupAnalyticsDrilldownWatcher(root);
    const chartsHost = root.querySelector('[data-drilldown-charts-host="true"]');
    const tableHost = root.querySelector('[data-drilldown-analytics-table-host="true"]');
    if (!(chartsHost instanceof HTMLElement) || !(tableHost instanceof HTMLElement)) {
      setDrilldownOverlayStatus(root, "error");
      return false;
    }
    chartsHost.replaceChildren();
    tableHost.replaceChildren();
    const chartSlots = Array.isArray(config?.chartSlots) ? config.chartSlots : [];
    chartSlots.forEach((slot, index) => {
      const slotEl = document.createElement("div");
      slotEl.className = "access-drilldown-analytics-chart-slot";
      slotEl.dataset.chartSlotIndex = String(index);
      chartsHost.appendChild(slotEl);
    });
    chartsHost.style.gridTemplateColumns =
      chartSlots.length > 1 ? `repeat(${chartSlots.length}, minmax(0, 1fr))` : "1fr";
    chartsHost.toggleAttribute("hidden", chartSlots.length === 0);

    try {
      const filterHost = root.querySelector(
        '[data-drilldown-body-mode="analytics"] [data-drilldown-filter-host="true"]',
      );
      await mountAnalyticsFilterBar(root, detail, config, filterHost);
      const detailSlot = config?.detailSlot;
      const detailConfig = detailSlot
        ? {
            ...resolveDrilldownTabConfig(config, detailSlot.id),
            queryStateId: config.queryStateId,
            columns: cloneArray(detailSlot.fields).length
              ? cloneArray(detailSlot.fields)
              : cloneArray(resolveDrilldownTabConfig(config, detailSlot.id).columns),
          }
        : config;
      const [chartsOk, tableOk] = await Promise.all([
        mountAnalyticsChartSlots(root, detail, config, chartSlots, chartsHost),
        mountDrilldownTable(root, detail, detailConfig, tableHost),
      ]);
      if (!tableOk || !chartsOk) {
        setDrilldownOverlayStatus(root, "error");
        return false;
      }
      const queryStateId = nonEmptyString(config?.queryStateId, detail?.query_state_id, detail?.queryStateId);
      if (queryStateId) {
        let refreshSeq = 0;
        const onQueryStateChange = (event) => {
          if (event?.detail?.id !== queryStateId) return;
          if (!(root instanceof HTMLElement) || root.hasAttribute("hidden")) return;
          const currentSeq = ++refreshSeq;
          mountAnalyticsChartSlots(root, detail, config, chartSlots, chartsHost)
            .then((ok) => {
              if (!ok || currentSeq !== refreshSeq) return;
              dispatchPreviewUpdated("drilldown");
            })
            .catch((error) => {
              recordPopupDebugIssue({
                level: "error",
                message: String(error?.message || error || "分析型下钻图表刷新失败"),
                phase: "analytics_chart_refresh_error",
                detail,
                config,
              });
            });
        };
        window.addEventListener("mei:query-state-change", onQueryStateChange);
        root.__meiAnalyticsQueryStateCleanup = () => {
          window.removeEventListener("mei:query-state-change", onQueryStateChange);
        };
      }
      setDrilldownOverlayStatus(root, "ready");
      dispatchPreviewUpdated("drilldown");
      return true;
    } catch (error) {
      recordPopupDebugIssue({
        level: "error",
        message: String(error?.message || error || "分析型下钻看板渲染失败"),
        phase: "analytics_render_error",
        detail,
        config,
      });
      setDrilldownOverlayStatus(root, "error");
      return false;
    }
  }

  const LIST_PREVIEW_ROW_SELECT_EVENT = "mei:table-row-select";

  function resolveListPreviewFields(config) {
    const previewFields = cloneArray(config?.rowPreviewSlot?.fields || config?.previewSlot?.fields);
    if (previewFields.length) return previewFields;
    const listFields = cloneArray(config?.detailSlot?.fields || config?.listSlot?.fields);
    if (listFields.length) return listFields;
    return cloneArray(config?.columns);
  }

  function renderListPreviewItemPanel(host, row, config) {
    if (!(host instanceof HTMLElement)) return;
    host.replaceChildren();
    if (!row || typeof row !== "object") {
      const empty = document.createElement("div");
      empty.className = "access-drilldown-list-preview-empty";
      empty.textContent = "点击清单中的条目查看详情";
      host.appendChild(empty);
      return;
    }
    const panel = document.createElement("div");
    panel.className = "access-drilldown-list-preview-panel";
    const title = document.createElement("div");
    title.className = "access-drilldown-list-preview-title";
    title.textContent = String(
      row?.label ??
        row?.案例名称 ??
        row?.预警ID ??
        row?.问题跟踪ID ??
        row?.标题 ??
        "条目详情",
    );
    panel.appendChild(title);
    const fields = resolveListPreviewFields(config);
    const entries =
      fields.length > 0
        ? fields.map((field) => {
            const key = String(field?.column || field?.key || field || "").trim();
            if (!key) return null;
            const label = String(field?.label || key).trim();
            const value = row?.[key];
            return { label, value };
          })
        : Object.entries(row).map(([key, value]) => ({ label: key, value }));
    const list = document.createElement("dl");
    list.className = "access-drilldown-list-preview-fields";
    entries
      .filter(Boolean)
      .forEach((entry) => {
        const dt = document.createElement("dt");
        dt.textContent = entry.label;
        const dd = document.createElement("dd");
        dd.textContent =
          entry.value === null || entry.value === undefined ? "" : String(entry.value);
        list.appendChild(dt);
        list.appendChild(dd);
      });
    panel.appendChild(list);
    host.appendChild(panel);
  }

  function cleanupListPreviewDrilldownWatcher(root) {
    if (!(root instanceof HTMLElement)) return;
    const cleanup = root.__meiListPreviewQueryStateCleanup;
    if (typeof cleanup === "function") {
      cleanup();
    }
    root.__meiListPreviewQueryStateCleanup = null;
    const rowSelectCleanup = root.__meiListPreviewRowSelectCleanup;
    if (typeof rowSelectCleanup === "function") {
      rowSelectCleanup();
    }
    root.__meiListPreviewRowSelectCleanup = null;
  }

  async function renderListPreviewDrilldownContent(root, detail, config) {
    applyDrilldownOverlayMeta(root, config);
    setDrilldownOverlayStatus(root, "loading");
    cleanupListPreviewDrilldownWatcher(root);
    const listHost = root.querySelector('[data-drilldown-list-host="true"]');
    const previewHost = root.querySelector('[data-drilldown-preview-host="true"]');
    if (!(listHost instanceof HTMLElement) || !(previewHost instanceof HTMLElement)) {
      setDrilldownOverlayStatus(root, "error");
      return false;
    }
    listHost.replaceChildren();
    renderListPreviewItemPanel(previewHost, null, config);
    const listSlot = config?.listSlot || config?.detailSlot;
    const listConfig = listSlot
      ? {
          ...resolveDrilldownTabConfig(config, listSlot.id),
          listPreviewDrilldown: true,
          queryStateId: config.queryStateId,
          columns: cloneArray(listSlot.fields).length
            ? cloneArray(listSlot.fields)
            : cloneArray(resolveDrilldownTabConfig(config, listSlot.id).columns),
        }
      : { ...config, listPreviewDrilldown: true };
    try {
      const filterHost = root.querySelector(
        '[data-drilldown-body-mode="list-preview"] [data-drilldown-filter-host="true"]',
      );
      await mountAnalyticsFilterBar(root, detail, config, filterHost);
      const tableOk = await mountDrilldownTable(root, detail, listConfig, listHost);
      if (!tableOk) {
        setDrilldownOverlayStatus(root, "error");
        return false;
      }
      const onRowSelect = (event) => {
        if (event?.detail?.query_state_id && event.detail.query_state_id !== config?.queryStateId) {
          return;
        }
        renderListPreviewItemPanel(previewHost, event?.detail?.row || null, config);
      };
      listHost.addEventListener(LIST_PREVIEW_ROW_SELECT_EVENT, onRowSelect);
      root.__meiListPreviewRowSelectCleanup = () => {
        listHost.removeEventListener(LIST_PREVIEW_ROW_SELECT_EVENT, onRowSelect);
      };
      const queryStateId = nonEmptyString(config?.queryStateId, detail?.query_state_id, detail?.queryStateId);
      if (queryStateId) {
        const onQueryStateChange = (event) => {
          if (event?.detail?.id !== queryStateId) return;
          if (!(root instanceof HTMLElement) || root.hasAttribute("hidden")) return;
          mountDrilldownTable(root, detail, listConfig, listHost)
            .then((ok) => {
              if (!ok) return;
              renderListPreviewItemPanel(previewHost, null, config);
              dispatchPreviewUpdated("drilldown");
            })
            .catch((error) => {
              recordPopupDebugIssue({
                level: "error",
                message: String(error?.message || error || "清单预览看板刷新失败"),
                phase: "list_preview_refresh_error",
                detail,
                config,
              });
            });
        };
        window.addEventListener("mei:query-state-change", onQueryStateChange);
        root.__meiListPreviewQueryStateCleanup = () => {
          window.removeEventListener("mei:query-state-change", onQueryStateChange);
        };
      }
      setDrilldownOverlayStatus(root, "ready");
      dispatchPreviewUpdated("drilldown");
      return true;
    } catch (error) {
      recordPopupDebugIssue({
        level: "error",
        message: String(error?.message || error || "清单预览看板渲染失败"),
        phase: "list_preview_render_error",
        detail,
        config,
      });
      setDrilldownOverlayStatus(root, "error");
      return false;
    }
  }

  function cleanupStructuredDrilldownWatcher(root) {
    if (!(root instanceof HTMLElement)) return;
    cleanupAnalyticsDrilldownWatcher(root);
    cleanupListPreviewDrilldownWatcher(root);
    const queryCleanup = root.__meiStructuredQueryStateCleanup;
    if (typeof queryCleanup === "function") {
      queryCleanup();
    }
    root.__meiStructuredQueryStateCleanup = null;
    const rowCleanup = root.__meiStructuredRowSelectionCleanup;
    if (typeof rowCleanup === "function") {
      rowCleanup();
    }
    root.__meiStructuredRowSelectionCleanup = null;
  }

  function boardLayoutAreasCss(areas = []) {
    if (!Array.isArray(areas) || areas.length === 0) return "";
    return areas
      .filter((row) => Array.isArray(row) && row.length > 0)
      .map((row) => `"${row.map((entry) => String(entry || "").trim() || ".").join(" ")}"`)
      .join(" ");
  }

  function applySceneShellLayout(node, layout) {
    if (!(node instanceof HTMLElement)) return;
    node.style.display = "grid";
    node.style.gridTemplateColumns =
      Array.isArray(layout?.columns) && layout.columns.length ? layout.columns.join(" ") : "";
    node.style.gridTemplateRows =
      Array.isArray(layout?.rows) && layout.rows.length ? layout.rows.join(" ") : "";
    node.style.gridTemplateAreas = boardLayoutAreasCss(layout?.areas);
    node.style.gap = nonEmptyString(layout?.gap);
    node.style.padding = nonEmptyString(layout?.padding);
  }

  function ensureStructuredDrilldownZoneHosts(root, sceneShell) {
    const layoutHost = root.querySelector('[data-drilldown-structured-layout="true"]');
    if (!(layoutHost instanceof HTMLElement) || !sceneShell) return null;
    layoutHost.replaceChildren();
    layoutHost.dataset.shellLayoutMode = String(sceneShell.layoutMode || "");
    applySceneShellLayout(layoutHost, sceneShell.layout);
    const zoneHosts = {};
    const zones = Array.isArray(sceneShell.zones) ? sceneShell.zones : [];
    const childrenByParent = new Map();
    zones.forEach((zone) => {
      const parent = nonEmptyString(zone?.parent);
      if (!childrenByParent.has(parent)) childrenByParent.set(parent, []);
      childrenByParent.get(parent).push(zone);
    });

    const createZoneNode = (zone) => {
      const wrapper = document.createElement(zone.role === "filter" || zone.role === "row_preview" ? "aside" : "div");
      wrapper.className = "access-drilldown-shell-zone";
      wrapper.dataset.shellZoneId = zone.id;
      wrapper.dataset.shellZoneRole = zone.role;
      if (zone.area) {
        wrapper.style.gridArea = zone.area;
      }
      if (zone.role === "container") {
        wrapper.classList.add("access-drilldown-shell-zone--container");
        applySceneShellLayout(wrapper, zone.layout);
      } else {
        const host =
          zone.role === "filter" || zone.role === "tab_bar"
            ? wrapper
            : (() => {
                const surface = document.createElement("div");
                surface.className = "access-drilldown-shell-surface";
                wrapper.appendChild(surface);
                return surface;
              })();
        host.classList.add("access-drilldown-shell-host");
        host.dataset.drilldownZoneHost = zone.id;
        zoneHosts[zone.id] = host;
      }
      const children = childrenByParent.get(zone.id) || [];
      children.forEach((child) => wrapper.appendChild(createZoneNode(child)));
      return wrapper;
    };

    const rootZones = childrenByParent.get("") || [];
    rootZones.forEach((zone) => layoutHost.appendChild(createZoneNode(zone)));
    return zoneHosts;
  }

  async function mountStructuredSlotZone(root, detail, config, zone, host) {
    if (!(host instanceof HTMLElement)) return false;
    host.replaceChildren();
    host.style.gridTemplateColumns = "";
    const zoneSlots = Array.isArray(config?.slotsByZone?.[zone.id]) ? config.slotsByZone[zone.id] : [];
    if (!zoneSlots.length) {
      return !zone.required;
    }
    if (zoneSlots.every((slot) => slot.component === "chart")) {
      zoneSlots.forEach((slot, index) => {
        const slotEl = document.createElement("div");
        slotEl.className = "access-drilldown-shell-slot access-drilldown-shell-slot--chart";
        slotEl.dataset.chartSlotIndex = String(index);
        host.appendChild(slotEl);
      });
      host.style.display = "grid";
      host.style.gridTemplateColumns =
        zoneSlots.length > 1 ? `repeat(${zoneSlots.length}, minmax(0, 1fr))` : "1fr";
      return mountAnalyticsChartSlots(root, detail, config, zoneSlots, host);
    }
    const primarySlot = zoneSlots[0];
    const baseConfig = resolveDrilldownTabConfig(config, primarySlot.id);
    const slotConfig = {
      ...baseConfig,
      queryStateId: config.queryStateId,
      hasChartZone: config.hasChartZone,
      hasRowPreviewZone: config.hasRowPreviewZone,
      tableMetricId: nonEmptyString(primarySlot.metricId, baseConfig.tableMetricId, config.tableMetricId),
      datasetId: nonEmptyString(primarySlot.datasetId, baseConfig.datasetId, config.datasetId),
      columns: cloneArray(primarySlot.fields).length
        ? cloneArray(primarySlot.fields)
        : cloneArray(baseConfig.columns),
      rowSelectionMode:
        config?.rowPreviewSourceZoneId && config.rowPreviewSourceZoneId === zone.id ? "single" : "",
    };
    if (primarySlot.component === "data_table") {
      return mountDrilldownTable(root, detail, slotConfig, host);
    }
    if (primarySlot.component === "summary" || primarySlot.component === "metric_card") {
      host.appendChild(createDrilldownSummaryNode(slotConfig, primarySlot.id));
      return true;
    }
    return false;
  }

  function mountStructuredRowPreviewZone(root, zoneHosts, config) {
    const previewZoneId = nonEmptyString(config?.rowPreviewZoneId);
    const sourceZoneId = nonEmptyString(config?.rowPreviewSourceZoneId);
    if (!previewZoneId || !sourceZoneId) return;
    const previewHost = zoneHosts?.[previewZoneId];
    const sourceHost = zoneHosts?.[sourceZoneId];
    if (!(previewHost instanceof HTMLElement) || !(sourceHost instanceof HTMLElement)) return;
    renderListPreviewItemPanel(previewHost, null, config);
    const onRowSelect = (event) => {
      if (event?.detail?.query_state_id && event.detail.query_state_id !== config?.queryStateId) {
        return;
      }
      renderListPreviewItemPanel(previewHost, event?.detail?.row || null, config);
    };
    sourceHost.addEventListener(LIST_PREVIEW_ROW_SELECT_EVENT, onRowSelect);
    root.__meiStructuredRowSelectionCleanup = () => {
      sourceHost.removeEventListener(LIST_PREVIEW_ROW_SELECT_EVENT, onRowSelect);
    };
  }

  function renderStructuredTabZones(root, detail, config, zoneHosts) {
    const tabsHost = zoneHosts?.[config?.tabBarZoneId];
    const contentHost = zoneHosts?.[config?.tabContentZoneId];
    if (!(tabsHost instanceof HTMLElement) || !(contentHost instanceof HTMLElement)) {
      return false;
    }
    const activeTab = renderDrilldownTabs(root, detail, config, tabsHost, contentHost);
    return renderDrilldownContent(root, detail, config, activeTab, contentHost);
  }

  async function renderStructuredDrilldownContent(root, detail, config) {
    applyDrilldownOverlayMeta(root, config);
    setDrilldownOverlayStatus(root, "loading");
    cleanupStructuredDrilldownWatcher(root);
    const zoneHosts = ensureStructuredDrilldownZoneHosts(root, config?.sceneShell);
    if (!zoneHosts) {
      setDrilldownOverlayStatus(root, "error");
      return false;
    }
    try {
      if (config?.sceneShell?.layoutMode === "generic_tabs") {
        const ok = renderStructuredTabZones(root, detail, config, zoneHosts);
        if (!ok) {
          setDrilldownOverlayStatus(root, "error");
          return false;
        }
        return true;
      }
      const filterZone = sceneShellZonesByRole(config?.sceneShell, "filter")[0] || null;
      if (filterZone && zoneHosts[filterZone.id] instanceof HTMLElement) {
        await mountAnalyticsFilterBar(root, detail, config, zoneHosts[filterZone.id]);
      }
      const slotZones = sceneShellZonesByRole(config?.sceneShell, "slots");
      for (const zone of slotZones) {
        const ok = await mountStructuredSlotZone(root, detail, config, zone, zoneHosts[zone.id]);
        if (!ok) {
          setDrilldownOverlayStatus(root, "error");
          return false;
        }
      }
      mountStructuredRowPreviewZone(root, zoneHosts, config);
      const queryStateId = nonEmptyString(config?.queryStateId, detail?.query_state_id, detail?.queryStateId);
      if (queryStateId) {
        const onQueryStateChange = (event) => {
          if (event?.detail?.id !== queryStateId) return;
          if (!(root instanceof HTMLElement) || root.hasAttribute("hidden")) return;
          renderStructuredDrilldownContent(root, detail, config)
            .then((ok) => {
              if (ok) dispatchPreviewUpdated("drilldown");
            })
            .catch((error) => {
              recordPopupDebugIssue({
                level: "error",
                message: String(error?.message || error || "通用下钻壳刷新失败"),
                phase: "structured_shell_refresh_error",
                detail,
                config,
              });
            });
        };
        window.addEventListener("mei:query-state-change", onQueryStateChange);
        root.__meiStructuredQueryStateCleanup = () => {
          window.removeEventListener("mei:query-state-change", onQueryStateChange);
        };
      }
      setDrilldownOverlayStatus(root, "ready");
      dispatchPreviewUpdated("drilldown");
      return true;
    } catch (error) {
      recordPopupDebugIssue({
        level: "error",
        message: String(error?.message || error || "通用下钻壳渲染失败"),
        phase: "structured_shell_render_error",
        detail,
        config,
      });
      setDrilldownOverlayStatus(root, "error");
      return false;
    }
  }

  async function mountDerivedDrilldownContent(root, detail, config, tabId, hostOverride = null) {
    const host =
      hostOverride instanceof HTMLElement
        ? hostOverride
        : root.querySelector('[data-drilldown-table-host="true"]');
    if (!(host instanceof HTMLElement)) {
      return false;
    }
    const datasetId = resolveDrilldownDatasetId(detail, config);
    if (!datasetId) {
      recordPopupDebugIssue({
        level: "error",
        message: "未解析到 explain 派生块需要的数据集 id",
        phase: "derived_dataset_missing",
        detail,
        config,
      });
      return false;
    }
    const dataset = await fetchPopupDrilldownRows(detail, { ...config, datasetId });
    const rows = Array.isArray(dataset?.rows) ? dataset.rows : [];
    if (!rows.length) {
      recordPopupDebugIssue({
        level: "warn",
        message: "popup panel 派生查询返回 0 行，已回退到摘要说明",
        phase: "derived_dataset_empty",
        detail,
        config,
        datasetId,
      });
    }
    if (explainMetricKind(config, tabId) === "composition") {
      const columns = Array.isArray(dataset?.columns) ? dataset.columns : [];
      const dimension = compositionFieldForTab(config, tabId);
      if (!dimension) {
        recordPopupDebugIssue({
          level: "error",
          message: `构成 tab 未解析到分组字段（tab=${normalizeTabId(tabId)}）`,
          phase: "derived_composition_dimension_missing",
          detail,
          config,
          datasetId,
        });
        return false;
      }
      const grouped = limitCompositionRows(
        groupRowsForComposition(rows, dimension, columns, config, detail),
        config,
      );
      if (!grouped.length) return false;
      const chartTag = drilldownChartTag(config?.chartKind, tabId) || "mei-chart-bar";
      const registered = await ensureDrilldownChartRegistered(chartTag);
      if (!registered) return false;
      host.replaceChildren();
      const node = document.createElement(chartTag);
      node.dataset.props = JSON.stringify(
        buildStaticChartModel(
          config?.title || `${dimension}构成`,
          tabId,
          grouped,
          {
            x: "label",
            y: "value",
          },
          config,
        ),
      );
      host.appendChild(node);
      dispatchPreviewUpdated("drilldown");
      return true;
    }
    if (explainMetricKind(config, tabId) === "trend") {
      const columns = Array.isArray(dataset?.columns) ? dataset.columns : [];
      const trendField = nonEmptyString(config?.trendField);
      if (!trendField) {
        recordPopupDebugIssue({
          level: "error",
          message: `趋势 tab 未解析到日期字段（tab=${normalizeTabId(tabId)}）`,
          phase: "derived_trend_field_missing",
          detail,
          config,
          datasetId,
        });
        return false;
      }
      const grouped = groupRowsByMonth(rows, trendField, columns);
      if (!grouped.length) return false;
      const registered = await ensureDrilldownChartRegistered("mei-chart-line");
      if (!registered) return false;
      host.replaceChildren();
      const node = document.createElement("mei-chart-line");
      node.dataset.props = JSON.stringify(
        buildStaticChartModel(
          config?.title || "趋势",
          tabId,
          grouped,
          {
            x: "month",
            y: "value",
          },
          config,
        ),
      );
      host.appendChild(node);
      dispatchPreviewUpdated("drilldown");
      return true;
    }
    return false;
  }

  function renderDrilldownContent(root, detail, config, tabId, hostOverride = null) {
    const activeConfig = resolveDrilldownTabConfig(config, tabId);
    applyDrilldownOverlayMeta(root, activeConfig);
    const host =
      hostOverride instanceof HTMLElement
        ? hostOverride
        : root.querySelector('[data-drilldown-table-host="true"]');
    if (!(host instanceof HTMLElement)) {
      return false;
    }
    const normalizedTab = normalizeTabId(tabId);
    if (config?.genericDrilldown) {
      const slot = config?.slotByTab?.[normalizedTab];
      if (slot?.component === "metric_card") {
        host.replaceChildren(createDrilldownSummaryNode(activeConfig, normalizedTab));
        setDrilldownOverlayStatus(root, "ready");
        return true;
      }
      if (slot?.component === "summary") {
        host.replaceChildren(createDrilldownSummaryNode(activeConfig, normalizedTab));
        setDrilldownOverlayStatus(root, "ready");
        return true;
      }
    }
    const kindTab = explainMetricKind(config, tabId);
    const tabOverride = config?.tabMetrics?.[normalizedTab] || config?.tabMetrics?.[kindTab];
    const hasCustomMetricSource = hasTabMetricDataSource(tabOverride);
    if (
      isDrilldownSummaryTab(tabId, config) ||
      (isDrilldownAnalysisTab(tabId, config) && !hasCustomMetricSource)
    ) {
      if (isDrilldownAnalysisTab(tabId, config) && !hasCustomMetricSource) {
        const summaryConfig = {
          ...activeConfig,
          note: nonEmptyString(activeConfig.note, unconfiguredTabNote(tabId)),
        };
        host.replaceChildren(createDrilldownSummaryNode(summaryConfig, tabId));
        setDrilldownOverlayStatus(root, "ready");
        return true;
      }
      host.replaceChildren(createDrilldownSummaryNode(activeConfig, tabId));
      setDrilldownOverlayStatus(root, "ready");
      return true;
    }
    if (isDrilldownAnalysisTab(tabId, config) || config?.genericDrilldown) {
      host.replaceChildren();
      setDrilldownOverlayStatus(root, "loading");
      const preferTableFirst =
        (config?.genericDrilldown && config?.slotByTab?.[normalizedTab]?.component === "data_table") ||
        (typeof window.__meiDatasetRuntime?.isYearMonthMatrixMetricConfig === "function" &&
          window.__meiDatasetRuntime.isYearMonthMatrixMetricConfig(activeConfig));
      const mountAnalysisContent = async () => {
        if (config?.genericDrilldown && config?.slotByTab?.[normalizedTab]?.supportRole === "composition") {
          if (await mountDerivedDrilldownContent(root, detail, activeConfig, tabId, host)) {
            return true;
          }
        }
        if (preferTableFirst && (await mountDrilldownTable(root, detail, activeConfig, host))) {
          return true;
        }
        if (await mountDrilldownChart(root, detail, activeConfig, tabId, host)) {
          return true;
        }
        if (!preferTableFirst && (await mountDrilldownTable(root, detail, activeConfig, host))) {
          return true;
        }
        if (config?.genericDrilldown && (await mountDerivedDrilldownContent(root, detail, activeConfig, tabId, host))) {
          return true;
        }
        return false;
      };
      mountAnalysisContent()
        .then(async (mounted) => {
          if (mounted) {
            setDrilldownOverlayStatus(root, "ready");
            dispatchPreviewUpdated("drilldown");
            return;
          }
          recordPopupDebugIssue({
            level: "error",
            message: `popup panel 表格挂载失败：${normalizedTab || tabId}`,
            phase: "table_mount_failed",
            detail,
            config: activeConfig,
            datasetId: activeConfig?.datasetId,
            metricId: activeConfig?.tableMetricId,
          });
          setDrilldownOverlayStatus(root, "error");
        })
        .catch(async (error) => {
          recordPopupDebugIssue({
            level: "error",
            message: String(error?.message || error || "图表 explain 块渲染失败"),
            phase: "chart_render_error",
            detail,
            config: activeConfig,
            datasetId: activeConfig?.datasetId,
            metricId: activeConfig?.tableMetricId,
          });
          if (await mountDrilldownTable(root, detail, activeConfig, host)) {
            setDrilldownOverlayStatus(root, "ready");
            dispatchPreviewUpdated("drilldown");
            return;
          }
          setDrilldownOverlayStatus(root, "error");
        });
      return true;
    }
    setDrilldownOverlayStatus(root, "loading");
    mountDrilldownTable(root, detail, activeConfig, host)
      .then((mounted) => {
        if (mounted) {
          setDrilldownOverlayStatus(root, "ready");
          dispatchPreviewUpdated("drilldown");
          return;
        }
        recordPopupDebugIssue({
          level: "error",
          message: `popup panel 表格挂载失败：${normalizedTab || tabId}`,
          phase: "table_mount_failed",
          detail,
          config: activeConfig,
          datasetId: activeConfig?.datasetId,
          metricId: activeConfig?.tableMetricId,
        });
        setDrilldownOverlayStatus(root, "error");
      })
      .catch((error) => {
        recordPopupDebugIssue({
          level: "error",
          message: String(error?.message || error || "明细表渲染失败"),
          phase: "table_render_error",
          detail,
          config: activeConfig,
          datasetId: activeConfig?.datasetId,
          metricId: activeConfig?.tableMetricId,
        });
        setDrilldownOverlayStatus(root, "error");
      });
    return true;
  }

  function renderDrilldownTabs(root, detail, config, hostOverride = null, contentHostOverride = null) {
    const tabsHost =
      hostOverride instanceof HTMLElement
        ? hostOverride
        : root.querySelector('[data-drilldown-tabs="true"]');
    if (!(tabsHost instanceof HTMLElement)) {
      return defaultActiveDrilldownTab(config?.tabs || []);
    }
    const normalizedTabs = Array.from(
      new Set(
        (Array.isArray(config?.tabs) ? config.tabs : [])
          .map((tab) => normalizeTabId(tab))
          .filter(Boolean),
      ),
    );
    const tabs = normalizedTabs.length
      ? normalizedTabs
      : [defaultActiveDrilldownTab(defaultDrilldownTabs(config?.explainKind, { hasDetail: true }))];
    const preferredTab = normalizeTabId(
      nonEmptyString(
        config?.popup?.entry,
        config?.popup?.entry_tab,
        config?.popup?.entryTab,
        config?.popup?.focus,
        config?.link?.entry,
        config?.link?.defaultFocus,
      ),
    );
    const defaultFromSlots = config?.genericDrilldown
      ? normalizeTabId(
          Object.values(config?.slotByTab || {}).find((slot) => slot?.default)?.id,
        )
      : "";
    const activeTab =
      (preferredTab && tabs.includes(preferredTab) ? preferredTab : "") ||
      (defaultFromSlots && tabs.includes(defaultFromSlots) ? defaultFromSlots : "") ||
      defaultActiveDrilldownTab(tabs);
    tabsHost.replaceChildren();
    tabsHost.toggleAttribute("hidden", tabs.length <= 1);
    tabs.forEach((tab) => {
      const explainMetric = explainMetricForTab(config, tab);
      const tabConfig = resolveDrilldownTabConfig(config, tab);
      const button = document.createElement("button");
      button.type = "button";
      button.className = "access-drilldown-tab-button";
      button.dataset.drilldownTab = tab;
      button.setAttribute("role", "tab");
      button.setAttribute("aria-selected", tab === activeTab ? "true" : "false");
      const slotLabel =
        config?.genericDrilldown && config?.slotByTab?.[tab]
          ? nonEmptyString(config.slotByTab[tab].label)
          : "";
      button.textContent = nonEmptyString(
        slotLabel,
        explainMetric?.label,
        tabConfig?.title,
        tabConfig?.label,
        drilldownTabLabel(explainMetric?.kind || tab, tabConfig),
      );
      button.addEventListener("click", () => {
        if (button.getAttribute("aria-selected") === "true") return;
        tabsHost
          .querySelectorAll(".access-drilldown-tab-button")
          .forEach((node) => node.setAttribute("aria-selected", node === button ? "true" : "false"));
        renderDrilldownContent(root, detail, config, tab, contentHostOverride);
      });
      tabsHost.appendChild(button);
    });
    return activeTab;
  }

  function ensureDrilldownOverlayRoot() {
    let root = document.getElementById(DRILLDOWN_OVERLAY_ROOT_ID);
    if (root) {
      return root;
    }
    root = document.createElement("div");
    root.id = DRILLDOWN_OVERLAY_ROOT_ID;
    root.className = "access-drilldown-overlay";
    root.setAttribute("hidden", "hidden");
    root.innerHTML =
      '<div class="access-drilldown-overlay-backdrop" data-drilldown-close="mask"></div>' +
      '<section class="access-drilldown-overlay-panel" role="dialog" aria-modal="true" aria-label="指标下钻明细">' +
      '<header class="access-drilldown-overlay-head">' +
      '<div class="access-drilldown-overlay-head-meta">' +
      '<div class="access-drilldown-overlay-title" data-drilldown-title="true"></div>' +
      '<div class="access-drilldown-overlay-note" data-drilldown-note="true" hidden></div>' +
      "</div>" +
      '<button type="button" class="access-drilldown-overlay-close" data-drilldown-close="button" aria-label="关闭">×</button>' +
      "</header>" +
      '<div class="access-drilldown-panel-hero" data-drilldown-hero="true" hidden>' +
      '<div class="access-drilldown-panel-hero-title" data-drilldown-hero-title="true"></div>' +
      '<div class="access-drilldown-panel-hero-note" data-drilldown-hero-note="true" hidden></div>' +
      "</div>" +
      '<div class="access-drilldown-overlay-tabs" data-drilldown-tabs="true" hidden></div>' +
      '<div class="access-drilldown-overlay-body" data-drilldown-body-mode="generic">' +
      '<div class="access-drilldown-overlay-status" data-drilldown-status="loading">正在加载明细表...</div>' +
      '<div class="access-drilldown-overlay-status" data-drilldown-status="error" hidden>明细表加载失败，请稍后重试。</div>' +
      '<div class="access-drilldown-table-shell" data-drilldown-status="ready" hidden>' +
      '<div class="access-drilldown-table-host" data-drilldown-table-host="true"></div>' +
      "</div>" +
      "</div>" +
      '<div class="access-drilldown-overlay-body access-drilldown-overlay-body--structured" data-drilldown-body-mode="structured" hidden>' +
      '<div class="access-drilldown-overlay-status" data-drilldown-status="loading">正在加载看板...</div>' +
      '<div class="access-drilldown-overlay-status" data-drilldown-status="error" hidden>看板加载失败，请稍后重试。</div>' +
      '<div class="access-drilldown-structured-shell" data-drilldown-status="ready" hidden>' +
      '<div class="access-drilldown-structured-layout" data-drilldown-structured-layout="true"></div>' +
      "</div>" +
      "</div>" +
      "</section>";
    root.addEventListener("click", (event) => {
      const target = event.target;
      if (!(target instanceof HTMLElement)) return;
      if (!target.dataset.drilldownClose) return;
      closeDrilldownOverlay();
    });
    document.body.appendChild(root);
    return root;
  }

  function setDrilldownOverlayStatus(root, status) {
    root
      .querySelectorAll("[data-drilldown-status]")
      .forEach((node) => node.toggleAttribute("hidden", node.dataset.drilldownStatus !== status));
  }

  function closeDrilldownOverlay() {
    const root = document.getElementById(DRILLDOWN_OVERLAY_ROOT_ID);
    if (!root) return;
    cleanupStructuredDrilldownWatcher(root);
    root.setAttribute("hidden", "hidden");
    root.classList.remove("is-open");
    for (const selector of [
      '[data-drilldown-table-host="true"]',
      '[data-drilldown-filter-host="true"]',
      '[data-drilldown-zone-host]',
      '[data-drilldown-structured-layout="true"]',
    ]) {
      root.querySelectorAll(selector).forEach((host) => {
        if (host instanceof HTMLElement) {
          host.replaceChildren();
        }
      });
    }
    document.body.classList.remove("access-drilldown-open");
    // 主屏在 overlay 期间未变，关闭时不广播 page 级 preview-updated，避免实时预警/典型案例等表格整页重查。
  }

  function stashSceneProjectionContext(detail, config) {
    try {
      sessionStorage.setItem(
        SCENE_PROJECTION_CONTEXT_KEY,
        JSON.stringify({
          stored_at: Date.now(),
          detail,
          config: {
            sceneId: config.sceneId,
            boardSceneFile: config.boardSceneFile,
            projection: config.projection,
            entry: nonEmptyString(config.popup?.entry, config.popup?.focus),
          },
        }),
      );
    } catch (_) {
      /* ignore */
    }
  }

  function consumeSceneProjectionContext() {
    let raw = "";
    try {
      raw = sessionStorage.getItem(SCENE_PROJECTION_CONTEXT_KEY) || "";
    } catch (_) {
      return null;
    }
    if (!raw) return null;
    try {
      sessionStorage.removeItem(SCENE_PROJECTION_CONTEXT_KEY);
    } catch (_) {
      /* ignore */
    }
    try {
      return JSON.parse(raw);
    } catch (_) {
      return null;
    }
  }

  function resolveBoardRouteUrl(config) {
    const appId = resolvePreviewAppId();
    if (!appId) return "";
    const boardFile = nonEmptyString(config.boardSceneFile);
    if (!boardFile) return "";
    let url;
    try {
      url = new URL(window.location.href);
    } catch (_) {
      return "";
    }
    url.pathname = `/apps/manage/${appId}`;
    url.searchParams.set("file", boardFile);
    if (config.boardSceneId) {
      url.searchParams.set("scene", config.boardSceneId);
    }
    url.searchParams.set("mei_projection", "route");
    const entry = nonEmptyString(config.popup?.entry, config.popup?.focus);
    if (entry) {
      url.searchParams.set("mei_entry_tab", entry);
    }
    return url.toString();
  }

  function openBoardRouteProjection(detail, config) {
    stashSceneProjectionContext(detail, config);
    const targetUrl = resolveBoardRouteUrl(config);
    if (!targetUrl) {
      openDrilldownOverlay(detail);
      return;
    }
    void navigateInternal(targetUrl, false);
  }

  function openSceneProjection(detail) {
    const config = resolveDrilldownConfig(detail);
    if (!config.enabled || !(config.boardSceneId || config.sceneId)) {
      if (config.errorMessage) {
        recordPopupDebugIssue({
          phase: config.errorCode || "scene_projection",
          message: config.errorMessage,
          detail,
          config,
          datasetId: nonEmptyString(detail?.dataset_id, detail?.__mei_runtime_ref?.dataset_id),
          metricId: nonEmptyString(detail?.metric_id, detail?.__mei_runtime_ref?.metric_id),
        });
      }
      return;
    }
    if (config.projection === "route") {
      openBoardRouteProjection(detail, config);
      return;
    }
    openDrilldownOverlay(detail);
  }

  function applySceneProjectionContextFromStorage() {
    if (!shouldMountDrilldownHost()) return;
    const stored = consumeSceneProjectionContext();
    if (!stored?.detail) return;
    const projection = normalizeProjection(
      nonEmptyString(stored.config?.projection, stored.detail?.projection, "route"),
    );
    if (projection !== "route") return;
    const detail = { ...stored.detail };
    const entry = nonEmptyString(stored.config?.entry, detail.popup?.entry, detail.popup?.focus);
    if (entry) {
      detail.popup = {
        ...(detail.popup || {}),
        entry,
        focus: entry,
        entry_tab: entry,
      };
    }
    openDrilldownOverlay(detail);
  }

  function openDrilldownOverlay(detail) {
    const config = resolveDrilldownConfig(detail);
    if (!config.enabled || !(config.boardSceneId || config.sceneId)) {
      if (config.errorMessage) {
        recordPopupDebugIssue({
          phase: config.errorCode || "scene_projection",
          message: config.errorMessage,
          detail,
          config,
          datasetId: nonEmptyString(detail?.dataset_id, detail?.__mei_runtime_ref?.dataset_id),
          metricId: nonEmptyString(detail?.metric_id, detail?.__mei_runtime_ref?.metric_id),
        });
      }
      return;
    }
    const root = ensureDrilldownOverlayRoot();
    applyDrilldownOverlayMeta(root, config);
    if (config.structuredBoard) {
      renderStructuredDrilldownContent(root, detail, config);
      root.removeAttribute("hidden");
      root.classList.add("is-open");
      document.body.classList.add("access-drilldown-open");
      return;
    }
    const activeTab = renderDrilldownTabs(root, detail, config);
    if (!renderDrilldownContent(root, detail, config, activeTab)) {
      root.removeAttribute("hidden");
      root.classList.add("is-open");
      document.body.classList.add("access-drilldown-open");
      return;
    }
    root.removeAttribute("hidden");
    root.classList.add("is-open");
    document.body.classList.add("access-drilldown-open");
  }

  function installSceneProjectionHost() {
    if (window.self !== window.top) return;
    if (!shouldMountDrilldownHost()) return;
    if (boot.metricDrilldownHostMounted) return;
    boot.metricDrilldownHostMounted = true;
    boot.sceneProjectionHostMounted = true;
    const openByEvent = (event) => {
      if (!shouldMountDrilldownHost()) return;
      const detail = event?.detail || {};
      const config = resolveDrilldownConfig(detail);
      if (!config.enabled || !(config.boardSceneId || config.sceneId)) {
        if (config.errorMessage) {
          recordPopupDebugIssue({
            phase: config.errorCode || "scene_projection",
            message: config.errorMessage,
            detail,
            config,
            datasetId: nonEmptyString(detail?.dataset_id, detail?.__mei_runtime_ref?.dataset_id),
            metricId: nonEmptyString(detail?.metric_id, detail?.__mei_runtime_ref?.metric_id),
          });
        }
        return;
      }
      openSceneProjection(detail);
    };
    document.addEventListener(METRIC_DRILLDOWN_EVENT, openByEvent);
    document.addEventListener(ANALYSIS_OPEN_EVENT, openByEvent);
    document.addEventListener(POPUP_OPEN_EVENT, openByEvent);
    document.addEventListener("keydown", (event) => {
      if (event.key === "Escape") {
        closeDrilldownOverlay();
      }
    });
  }

  function patchDrilldownTableByMetric(tableMetricId) {
    const metricId = String(tableMetricId || "").trim();
    if (!metricId) return true;
    const table = document.querySelector("mei-dataset-table");
    if (!(table instanceof HTMLElement)) return false;
    let props = {};
    try {
      props = JSON.parse(table.dataset.props || "{}");
    } catch (_) {
      props = {};
    }
    if (!props || typeof props !== "object") return true;
    const currentData = props.data && typeof props.data === "object" ? props.data : {};
    const runtimeRef =
      (currentData && currentData.__mei_runtime_ref) ||
      (props.dataset && props.dataset.__mei_runtime_ref) ||
      {};
    const datasetId = String(
      runtimeRef.dataset_id ||
        currentData.from_dataset ||
        currentData.id ||
        props?.dataset?.id ||
        "",
    ).trim();
    if (!datasetId) return true;
    props.data = { __ref: "metric", id: metricId, from_dataset: datasetId };
    table.dataset.props = JSON.stringify(props);
    const remount = document.createElement("mei-dataset-table");
    remount.dataset.props = table.dataset.props;
    table.replaceWith(remount);
    dispatchPreviewUpdated("drilldown");
    return true;
  }

  function renderDrilldownContextBanner(title, note) {
    const header = String(title || "").trim();
    const body = String(note || "").trim();
    if (!header && !body) return;
    let banner = document.getElementById(DRILLDOWN_CONTEXT_BANNER_ID);
    if (!banner) {
      banner = document.createElement("div");
      banner.id = DRILLDOWN_CONTEXT_BANNER_ID;
      banner.className = "access-drilldown-context-banner";
      document.body.appendChild(banner);
    }
    banner.innerHTML =
      `<div class="access-drilldown-context-title">${header || "指标口径"}</div>` +
      (body ? `<div class="access-drilldown-context-note">${body}</div>` : "");
  }

  function clearDrilldownContextBanner() {
    const banner = document.getElementById(DRILLDOWN_CONTEXT_BANNER_ID);
    if (banner) {
      banner.remove();
    }
  }

  function applyDrilldownContextFromQuery() {
    clearTimeout(drilldownContextRetryTimer);
    if (!isAccessRoute()) {
      clearDrilldownContextBanner();
      return;
    }
    let parsed = null;
    try {
      parsed = new URL(window.location.href);
    } catch (_) {
      return;
    }
    const metricId = String(parsed.searchParams.get("drill_metric") || "").trim();
    if (!metricId) {
      clearDrilldownContextBanner();
      return;
    }
    const title = parsed.searchParams.get("drill_title") || "";
    const note = parsed.searchParams.get("drill_note") || "";
    renderDrilldownContextBanner(title, note);
    const tableMetricId = String(parsed.searchParams.get("drill_table_metric") || "").trim();
    if (!tableMetricId) return;
    let attempts = 0;
    const retry = () => {
      attempts += 1;
      if (patchDrilldownTableByMetric(tableMetricId) || attempts >= 24) {
        return;
      }
      drilldownContextRetryTimer = window.setTimeout(retry, 90);
    };
    retry();
  }

  function currentMainPane() {
    return document.querySelector("#workspace-root main.main");
  }

  function clearManageWorkspaceLoadingState() {
    const currentMain = currentMainPane();
    if (!currentMain) return;
    currentMain.removeAttribute("aria-busy");
    const overlay = currentMain.querySelector('[data-mei-manage-nav-loading="true"]');
    if (overlay) overlay.remove();
  }

  function navigationTargetLabel(url) {
    try {
      const parsed = new URL(url, window.location.href);
      const file = String(parsed.searchParams.get("file") || "").trim();
      if (file) return file;
      const scene = String(parsed.searchParams.get("scene") || "").trim();
      if (scene) return `scene:${scene}`;
    } catch (_) {}
    return "目标预览";
  }

  function showManageWorkspaceLoadingState(url) {
    const currentUrl = new URL(window.location.href);
    const nextUrl = new URL(url, window.location.href);
    const isSameManageRoute =
      currentUrl.pathname === nextUrl.pathname &&
      currentUrl.pathname.startsWith("/apps/manage/");
    if (!isSameManageRoute) {
      clearManageWorkspaceLoadingState();
      return;
    }
    const currentMain = currentMainPane();
    if (!currentMain) return;
    currentMain.setAttribute("aria-busy", "true");
    let overlay = currentMain.querySelector('[data-mei-manage-nav-loading="true"]');
    if (!overlay) {
      overlay = document.createElement("div");
      overlay.setAttribute("data-mei-manage-nav-loading", "true");
      overlay.style.cssText = [
        "position:absolute",
        "inset:0",
        "z-index:40",
        "display:grid",
        "place-items:center",
        "padding:24px",
        "background:linear-gradient(180deg, rgba(8,15,30,.42), rgba(8,15,30,.70))",
        "backdrop-filter:blur(2px)",
        "pointer-events:none",
      ].join(";");
      const card = document.createElement("div");
      card.style.cssText = [
        "display:grid",
        "gap:8px",
        "min-width:220px",
        "padding:16px 18px",
        "border-radius:14px",
        "border:1px solid rgba(96,165,250,.35)",
        "background:rgba(15,23,42,.88)",
        "box-shadow:0 12px 40px rgba(2,6,23,.28)",
        "color:#e2e8f0",
        "text-align:center",
      ].join(";");
      const title = document.createElement("strong");
      title.textContent = "正在切换预览";
      title.style.cssText = "font-size:14px;font-weight:700;color:#f8fafc;";
      const detail = document.createElement("span");
      detail.setAttribute("data-mei-manage-nav-target", "true");
      detail.style.cssText =
        "font-size:12px;line-height:1.5;color:#93c5fd;font-family:ui-monospace,SFMono-Regular,monospace;";
      const hint = document.createElement("span");
      hint.textContent = "旧画面将被替换，请稍候...";
      hint.style.cssText = "font-size:11px;line-height:1.5;color:#94a3b8;";
      card.appendChild(title);
      card.appendChild(detail);
      card.appendChild(hint);
      overlay.appendChild(card);
      if (getComputedStyle(currentMain).position === "static") {
        currentMain.style.position = "relative";
      }
      currentMain.appendChild(overlay);
    }
    const detail = overlay.querySelector('[data-mei-manage-nav-target="true"]');
    if (detail) {
      detail.textContent = navigationTargetLabel(url);
    }
  }

  function createLoadingOverlay() {
    if (document.getElementById("mei-spa-loading")) return;
    const overlay = document.createElement("div");
    overlay.id = "mei-spa-loading";
    overlay.className = "spa-loading-overlay";
    overlay.innerHTML =
      '<div class="spa-loading-inner">' +
      '<img class="spa-loading-icon" src="/app-assets/favicon.svg" alt="loading"/>' +
      '<span class="spa-loading-text">加载中...</span>' +
      "</div>";
    document.body.appendChild(overlay);
  }

  function clearLoadingTimer() {
    if (loadingTimer) {
      clearTimeout(loadingTimer);
      loadingTimer = null;
    }
  }

  function showLoading() {
    clearLoadingTimer();
    loadingTimer = setTimeout(() => {
      createLoadingOverlay();
      const overlay = document.getElementById("mei-spa-loading");
      if (!overlay) return;
      overlay.classList.add("is-visible");
      loadingVisibleAt = Date.now();
      loadingTimer = null;
    }, LOADING_DELAY_MS);
  }

  function hideLoading() {
    clearLoadingTimer();
    const overlay = document.getElementById("mei-spa-loading");
    if (!overlay) return;
    if (!overlay.classList.contains("is-visible")) {
      return;
    }
    const elapsed = Date.now() - loadingVisibleAt;
    const finish = () => {
      overlay.classList.remove("is-visible");
    };
    if (elapsed < LOADING_MIN_VISIBLE_MS) {
      setTimeout(finish, LOADING_MIN_VISIBLE_MS - elapsed);
    } else {
      finish();
    }
  }

  function forceHideLoading() {
    clearLoadingTimer();
    const overlay = document.getElementById("mei-spa-loading");
    if (overlay) {
      overlay.classList.remove("is-visible");
    }
  }

  function finishNavigationUi(navigationId) {
    clearManageWorkspaceLoadingState();
    if (navigationId !== currentNavigationId && spaNavigationInFlight > 0) {
      return;
    }
    forceHideLoading();
    clearManageWorkspaceLoadingState();
  }

  function isManageSamePathNavigation(currentUrl, nextUrl) {
    return (
      currentUrl.pathname === nextUrl.pathname &&
      currentUrl.pathname.startsWith("/apps/manage/")
    );
  }

  function shouldReloadHostBundle(path, currentUrl, nextUrl) {
    if (path === "/app-bundles/manage.js") {
      const cur = currentUrl.pathname.startsWith("/apps/manage/");
      const next = nextUrl.pathname.startsWith("/apps/manage/");
      return cur !== next;
    }
    if (path === "/app-bundles/access.js") {
      const cur = isAppRoute(currentUrl.pathname);
      const next = isAppRoute(nextUrl.pathname);
      return cur !== next;
    }
    return false;
  }

  function syncManageTabFromUrl(url) {
    try {
      const tab = new URL(url, window.location.href).searchParams.get("tab");
      if (typeof boot.switchManageTab === "function") {
        boot.switchManageTab(tab || "preview", { updateUrl: false, emit: true });
      }
    } catch (_) {}
  }

  function sameOrigin(url) {
    try {
      const parsed = new URL(url, window.location.href);
      return parsed.origin === window.location.origin;
    } catch (_) {
      return false;
    }
  }

  function shouldHandleUrl(url) {
    if (!sameOrigin(url)) return false;
    const parsed = new URL(url, window.location.href);
    return parsed.pathname.startsWith("/apps/");
  }

  function isSameLocation(url) {
    try {
      const next = new URL(url, window.location.href);
      const current = new URL(window.location.href);
      return (
        next.pathname === current.pathname &&
        next.search === current.search &&
        next.hash === current.hash
      );
    } catch (_) {
      return false;
    }
  }

  function resolveClickTarget(event) {
    const path = event.composedPath ? event.composedPath() : [];
    for (const item of path) {
      if (item instanceof HTMLAnchorElement && item.href) {
        return {
          url: item.href,
          target: item.getAttribute("target") || "",
          download: item.hasAttribute("download"),
        };
      }
      if (
        item instanceof HTMLElement &&
        item.tagName === "SL-BUTTON" &&
        item.hasAttribute("href")
      ) {
        const rawHref = item.getAttribute("href") || "";
        let absolute = rawHref;
        try {
          absolute = new URL(rawHref, window.location.href).href;
        } catch (_) {}
        return {
          url: absolute,
          target: item.getAttribute("target") || "",
          download: item.hasAttribute("download"),
        };
      }
    }
    return null;
  }

  /** 构建页内 Tab 走客户端切换；配置/上传独立壳整页导航；应用↔构建与其它 /apps/ 链路由 SPA 拦截。 */
  function shouldBypassSpaClick(event) {
    const path = event.composedPath ? event.composedPath() : [];
    for (const item of path) {
      if (!(item instanceof HTMLElement) || !item.matches) continue;
      if (
        item.matches(
          "a.manage-view-tab[data-manage-tab], [data-mei-view='config'], [data-mei-view='upload'], a[data-manage-config-link='1']",
        )
      ) {
        return true;
      }
    }
    return false;
  }

  function shouldAbortRuntimeForBypassNavigation(event) {
    const path = event.composedPath ? event.composedPath() : [];
    for (const item of path) {
      if (!(item instanceof HTMLElement) || !item.matches) continue;
      if (
        item.matches(
          "[data-mei-view='config'], [data-mei-view='upload'], a[data-manage-config-link='1']",
        )
      ) {
        return true;
      }
    }
    return false;
  }

  function requestRuntimeAbort(reason) {
    try {
      window.dispatchEvent(
        new CustomEvent("mei:abort-runtime-queries", {
          detail: { reason: String(reason || "").trim() },
        }),
      );
    } catch (_) {}
  }

  function normalizePath(rawUrl) {
    try {
      const parsed = new URL(rawUrl, window.location.href);
      return parsed.pathname;
    } catch (_) {
      return "";
    }
  }

  const SCENE_BUNDLE_PATH_PREFIX = "/workspace-components/bundles/";

  function isSceneBundlePath(path) {
    return Boolean(path && path.startsWith(SCENE_BUNDLE_PATH_PREFIX));
  }

  function isWorkspaceComponentModulePath(path) {
    return path.startsWith("/workspace-components/") && !isSceneBundlePath(path);
  }

  function findSceneBundleSrcInDoc(doc) {
    if (!doc || !doc.body) return "";
    for (const script of doc.body.querySelectorAll("script[src]")) {
      const src = (script.getAttribute("src") || "").trim();
      if (!src) continue;
      const path = normalizePath(src);
      if (isSceneBundlePath(path) || script.getAttribute("data-mei-scene-bundle") === "true") {
        return src;
      }
    }
    return "";
  }

  function currentSceneBundlePath() {
    const node = document.querySelector('script[data-mei-scene-bundle="true"]');
    if (!node) return "";
    const persisted = (node.getAttribute("data-mei-persistent-script") || "").trim();
    if (persisted) return persisted;
    return normalizePath(node.getAttribute("src") || "");
  }

  function shouldForceHardNavForSceneBundleSwitch(doc) {
    const nextBundleSrc = findSceneBundleSrcInDoc(doc);
    if (!nextBundleSrc) return false;
    const nextPath = normalizePath(nextBundleSrc);
    if (!nextPath) return false;
    const currentPath = currentSceneBundlePath();
    if (!currentPath) return false;
    return currentPath !== nextPath;
  }

  function removeExistingSceneBundleScripts() {
    document.querySelectorAll('script[data-mei-scene-bundle="true"]').forEach((node) => {
      node.remove();
    });
  }

  async function syncSceneBundleFromDoc(doc, navigationId) {
    const bundleSrc = findSceneBundleSrcInDoc(doc);
    if (!bundleSrc) {
      removeExistingSceneBundleScripts();
      return true;
    }
    const path = normalizePath(bundleSrc);
    const alreadyLoaded = document.querySelector(
      'script[data-mei-scene-bundle="true"][data-mei-persistent-script="' + path + '"]',
    );
    if (alreadyLoaded) return true;
    removeExistingSceneBundleScripts();
    if (navigationId !== currentNavigationId) return false;
    await loadScript(bundleSrc, {
      module: true,
      persistentKey: path,
      sceneBundle: true,
      softFail: true,
    });
    if (navigationId === currentNavigationId) {
      wakeRuntimeAfterSceneBundleLoaded();
    }
    return navigationId === currentNavigationId;
  }

  function collectBodyScripts(doc) {
    return Array.from(doc.body.querySelectorAll("script[src]"))
      .map((script) => script.getAttribute("src") || "")
      .map((src) => src.trim())
      .filter(Boolean);
  }

  function tagExistingBodyScripts() {
    Array.from(document.body.querySelectorAll("script[src]")).forEach((script) => {
      const src = script.getAttribute("src");
      if (!src) return;
      const path = normalizePath(src);
      if (!path || path === SPA_NAV_SCRIPT) return;
      if (isSceneBundlePath(path) || script.getAttribute("data-mei-scene-bundle") === "true") {
        script.setAttribute("data-mei-scene-bundle", "true");
        script.setAttribute("data-mei-persistent-script", path);
        return;
      }
      if (isWorkspaceComponentModulePath(path)) {
        script.setAttribute("data-mei-persistent-script", path);
        return;
      }
      if (path.startsWith("/app-assets/")) {
        if (RELOAD_APP_SCRIPTS.has(path)) {
          script.setAttribute("data-mei-reload-script", path);
        } else {
          script.setAttribute("data-mei-persistent-script", path);
        }
        return;
      }
      if (path.startsWith("/app-bundles/")) {
        if (RELOAD_BUNDLE_SCRIPTS.has(path)) {
          script.setAttribute("data-mei-reload-script", path);
        } else {
          script.setAttribute("data-mei-persistent-script", path);
        }
      }
    });
  }

  function disposeRuntimeHooks(options) {
    const opts = options || {};
    const names = [
      "disposeAgentPanel",
      "disposeStatusBar",
      "disposeManageTabs",
      "disposeWorkspaceSplitters",
      "disposeFrameStage",
      "disposeSourceTreeControls",
      "disposeSourceHighlight",
    ];
    names.forEach((name) => {
      if (opts.preserveAgentPanel && name === "disposeAgentPanel") return;
      if (opts.preserveStatusBar && name === "disposeStatusBar") return;
      if (opts.preserveManageTabs && name === "disposeManageTabs") return;
      if (opts.preserveWorkspaceSplitters && name === "disposeWorkspaceSplitters") return;
      if (opts.preserveFrameStage && name === "disposeFrameStage") return;
      if (opts.preserveSourceTreeControls && name === "disposeSourceTreeControls") return;
      if (opts.preserveSourceHighlight && name === "disposeSourceHighlight") return;
      const hook = boot[name];
      if (typeof hook === "function") {
        try {
          hook();
        } catch (_) {}
        boot[name] = null;
      }
    });
  }

  function loadScript(rawSrc, options) {
    const opts = options || {};
    const absolute = new URL(rawSrc, window.location.href).toString();
    if (opts.persistentKey) {
      const found = document.querySelector(
        'script[data-mei-persistent-script="' + opts.persistentKey + '"]',
      );
      if (found) return Promise.resolve();
    }
    if (opts.reloadKey) {
      document
        .querySelectorAll('script[data-mei-reload-script="' + opts.reloadKey + '"]')
        .forEach((node) => node.remove());
    }
    return new Promise((resolve, reject) => {
      let settled = false;
      const finish = (fn) => {
        if (settled) return;
        settled = true;
        clearTimeout(timer);
        fn();
      };
      const timer = setTimeout(() => {
        if (opts.softFail) {
          console.warn("[spa-navigation] script load timeout", rawSrc);
          finish(resolve);
          return;
        }
        finish(() => reject(new Error("script load timeout: " + rawSrc)));
      }, SCRIPT_LOAD_TIMEOUT_MS);
      const script = document.createElement("script");
      if (opts.module) script.type = "module";
      script.src = absolute;
      script.async = false;
      if (opts.persistentKey) {
        script.setAttribute("data-mei-persistent-script", opts.persistentKey);
      }
      if (opts.sceneBundle) {
        script.setAttribute("data-mei-scene-bundle", "true");
      }
      if (opts.reloadKey) {
        script.setAttribute("data-mei-reload-script", opts.reloadKey);
      }
      script.onload = () => finish(resolve);
      script.onerror = () => {
        if (opts.softFail) {
          console.warn("[spa-navigation] script load skipped", rawSrc);
          finish(resolve);
          return;
        }
        finish(() => reject(new Error("failed to load script: " + rawSrc)));
      };
      document.body.appendChild(script);
    });
  }

  function pulseManagePreview(detail, options) {
    const opts = options || {};
    dispatchManageContextChange(detail);
    dispatchPreviewUpdated("page", {
      resetRuntimeQueryCache: opts.resetRuntimeQueryCache !== false,
    });
    requestAnimationFrame(() => {
      dispatchPreviewUpdated("page", {
        resetRuntimeQueryCache: opts.resetRuntimeQueryCache !== false,
      });
      if (typeof boot.scheduleFrameViewportRelayout === "function") {
        try {
          boot.scheduleFrameViewportRelayout();
        } catch (_) {}
      }
    });
  }

  function publishManagePreviewFromDoc(doc, options) {
    const panelRoot =
      document.querySelector("#meilang-author-panel") ||
      (doc && doc.querySelector("#meilang-author-panel"));
    pulseManagePreview(extractManagePanelContext(panelRoot), options);
  }

  function replaceShellFromDoc(doc, url, replaceHistory) {
    const currentShell = document.querySelector(".shell");
    const nextShell = doc.querySelector(".shell");
    if (!currentShell || !nextShell) return false;
    currentShell.className = nextShell.className;
    currentShell.replaceChildren(
      ...Array.from(nextShell.childNodes).map((node) => node.cloneNode(true)),
    );
    if (replaceHistory) {
      window.history.replaceState({}, "", url);
    } else {
      window.history.pushState({}, "", url);
    }
    return true;
  }

  async function syncMissingWorkspaceModulesOnly(doc, navigationId) {
    const bundleReady = await syncSceneBundleFromDoc(doc, navigationId);
    if (!bundleReady || navigationId !== currentNavigationId) return false;
    if (findSceneBundleSrcInDoc(doc)) return true;
    const scripts = collectBodyScripts(doc).filter((src) => {
      const path = normalizePath(src);
      return isWorkspaceComponentModulePath(path);
    });
    for (const src of scripts) {
      if (navigationId !== currentNavigationId) return false;
      const path = normalizePath(src);
      if (
        document.querySelector(
          'script[data-mei-persistent-script="' + path + '"]',
        )
      ) {
        continue;
      }
      await loadScript(src, { module: true, persistentKey: path, softFail: true });
    }
    return true;
  }

  async function ensureHostBundlesFromDoc(doc, navigationId, currentUrl, nextUrl) {
    for (const src of collectBodyScripts(doc)) {
      if (navigationId !== currentNavigationId) return false;
      const path = normalizePath(src);
      if (path !== "/app-bundles/manage.js" && path !== "/app-bundles/access.js") {
        continue;
      }
      const alreadyLoaded =
        document.querySelector('script[data-mei-persistent-script="' + path + '"]') ||
        document.querySelector('script[data-mei-reload-script="' + path + '"]');
      if (alreadyLoaded) {
        if (currentUrl && nextUrl && shouldReloadHostBundle(path, currentUrl, nextUrl)) {
          await loadScript(path + "?spa=" + Date.now(), {
            reloadKey: path,
            softFail: true,
          });
        }
        continue;
      }
      await loadScript(src, { persistentKey: path, softFail: true });
    }
    return true;
  }

  async function syncScriptsFromDocument(doc, navigationId, options) {
    const opts = options || {};
    const currentUrl = opts.currentUrl;
    const nextUrl = opts.nextUrl;
    const bundleReady = await syncSceneBundleFromDoc(doc, navigationId);
    if (!bundleReady || navigationId !== currentNavigationId) return false;
    const docUsesSceneBundle = Boolean(findSceneBundleSrcInDoc(doc));
    const scripts = collectBodyScripts(doc);
    for (const src of scripts) {
      if (navigationId !== currentNavigationId) return false;
      const path = normalizePath(src);
      if (!path) continue;
      if (path === SPA_NAV_SCRIPT) continue;
      if (
        path === "/app-bundles/manage.js" ||
        path === "/app-bundles/access.js"
      ) {
        if (
          currentUrl &&
          nextUrl &&
          !shouldReloadHostBundle(path, currentUrl, nextUrl)
        ) {
          await loadScript(src, { persistentKey: path });
          continue;
        }
      }
      if (
        opts.preserveManageWorkspace &&
        path === "/app-bundles/manage.js"
      ) {
        continue;
      }
      if (
        opts.preserveAgentPanel &&
        path.startsWith("/app-assets/") &&
        path.includes("agent-panel")
      ) {
        continue;
      }
      if (opts.preserveStatusBar && path === "/app-assets/statusbar.js") {
        continue;
      }
      if (
        opts.preserveWorkspaceSplitters &&
        path === "/app-assets/workspace-splitters.js"
      ) {
        continue;
      }
      if (
        opts.preserveSourceTreeControls &&
        path === "/app-assets/source-tree-controls.js"
      ) {
        continue;
      }
      if (isSceneBundlePath(path)) {
        continue;
      }
      if (docUsesSceneBundle && isWorkspaceComponentModulePath(path)) {
        continue;
      }
      if (isWorkspaceComponentModulePath(path)) {
        await loadScript(src, { module: true, persistentKey: path, softFail: true });
        continue;
      }
      if (path.startsWith("/app-assets/")) {
        if (RELOAD_APP_SCRIPTS.has(path)) {
          const withBuster = path + "?spa=" + Date.now();
          await loadScript(withBuster, { reloadKey: path, softFail: true });
          continue;
        }
        await loadScript(src, { persistentKey: path, softFail: true });
        continue;
      }
      if (path.startsWith("/app-bundles/")) {
        if (RELOAD_BUNDLE_SCRIPTS.has(path)) {
          const withBuster = path + "?spa=" + Date.now();
          await loadScript(withBuster, { reloadKey: path, softFail: true });
          continue;
        }
        await loadScript(src, { persistentKey: path, softFail: true });
        continue;
      }
    }
    return true;
  }

  function cloneNodeOrNull(node) {
    return node ? node.cloneNode(true) : null;
  }

  function extractManagePanelContext(root) {
    if (!root) return null;
    return {
      app: String(root.dataset.app || ""),
      scene: String(root.dataset.scene || ""),
      file: String(root.dataset.file || root.dataset.target || ""),
      sceneTarget: String(root.dataset.sceneTarget || ""),
      mode: String(root.dataset.mode || ""),
      sourceViews: String(root.dataset.sourceViews || ""),
      viewTab: String(root.dataset.viewTab || ""),
    };
  }

  function dispatchManageContextChange(detail) {
    if (!detail) return;
    document.dispatchEvent(
      new CustomEvent("mei:manage-context-change", {
        detail,
      }),
    );
  }

  function normalizeNavHref(rawHref) {
    try {
      const url = new URL(rawHref, window.location.href);
      url.searchParams.delete("tab");
      return url.pathname + "?" + url.searchParams.toString();
    } catch (_) {
      return String(rawHref || "");
    }
  }

  function syncSidebarLinkState(currentSidebar, nextSidebar) {
    if (!currentSidebar || !nextSidebar) return;
    currentSidebar.className = nextSidebar.className;
    const currentLinks = Array.from(currentSidebar.querySelectorAll("a.tree-link"));
    const nextLinks = Array.from(nextSidebar.querySelectorAll("a.tree-link"));
    const nextByKey = new Map();
    nextLinks.forEach((link) => {
      nextByKey.set(normalizeNavHref(link.getAttribute("href") || ""), link);
    });
    currentLinks.forEach((link) => {
      const key = normalizeNavHref(link.getAttribute("href") || "");
      const next = nextByKey.get(key);
      if (!next) return;
      link.className = next.className;
      link.setAttribute("href", next.getAttribute("href") || "");
      if (next.hasAttribute("title")) {
        link.setAttribute("title", next.getAttribute("title") || "");
      } else {
        link.removeAttribute("title");
      }
      Array.from(link.attributes)
        .filter((attr) => attr.name.startsWith("data-"))
        .forEach((attr) => link.removeAttribute(attr.name));
      Array.from(next.attributes)
        .filter((attr) => attr.name.startsWith("data-"))
        .forEach((attr) => link.setAttribute(attr.name, attr.value));
      link.innerHTML = next.innerHTML;
    });
    const currentDetails = Array.from(
      currentSidebar.querySelectorAll(".tree-li-branch > details"),
    );
    const nextDetails = Array.from(
      nextSidebar.querySelectorAll(".tree-li-branch > details"),
    );
    currentDetails.forEach((detail, index) => {
      if (index >= nextDetails.length) return;
      const wasOpen = detail.open;
      const serverWantsOpen = nextDetails[index].open;
      // 服务端仅按「选中路径」展开祖先；合并保留用户已展开的其它分支，避免换文件整树收起。
      detail.open = serverWantsOpen || wasOpen;
    });
  }

  function syncStatusbarContent(currentStatusbar, nextStatusbar) {
    if (!currentStatusbar || !nextStatusbar) return;
    currentStatusbar.className = nextStatusbar.className;
    const currentLayout = currentStatusbar.querySelector(".statusbar-layout");
    const nextLayout = nextStatusbar.querySelector(".statusbar-layout");
    if (!currentLayout || !nextLayout) return;
    currentLayout.className = nextLayout.className;
    const currentTracks = Array.from(currentLayout.children);
    const nextTracks = Array.from(nextLayout.children);
    currentTracks.forEach((track, index) => {
      if (index >= nextTracks.length) return;
      track.className = nextTracks[index].className;
      track.replaceChildren(
        ...Array.from(nextTracks[index].childNodes).map((node) => node.cloneNode(true)),
      );
    });
  }

  function syncElementAttributes(currentEl, nextEl, options) {
    if (!currentEl || !nextEl) return;
    const opts = options || {};
    const preserve = new Set(opts.preserve || []);
    Array.from(currentEl.attributes).forEach((attr) => {
      if (preserve.has(attr.name)) return;
      currentEl.removeAttribute(attr.name);
    });
    Array.from(nextEl.attributes).forEach((attr) => {
      if (preserve.has(attr.name) && currentEl.hasAttribute(attr.name)) return;
      currentEl.setAttribute(attr.name, attr.value);
    });
  }

  /** 同一 manage 路径下换 file/scene/tab 只换工作区，避免整页重载 manage bundle。 */
  function shouldPreserveManageWorkspace(currentUrl, nextUrl) {
    return (
      currentUrl.pathname === nextUrl.pathname &&
      currentUrl.pathname.startsWith("/apps/manage/")
    );
  }

  /** 同路径 SPA 只替换 #workspace-root 时，顶栏仍在壳外，需从下一页文档同步 href（访问 / 演示 / 应用切换）。 */
  function syncManageTopbarFromDoc(doc) {
    try {
      const currentHeader = document.querySelector("header.topbar-shell");
      const nextHeader = doc.querySelector("header.topbar-shell");
      if (!currentHeader || !nextHeader) return;

      const currentGroup = currentHeader.querySelector("sl-button-group.mode-tab-group");
      const nextGroup = nextHeader.querySelector("sl-button-group.mode-tab-group");
      if (currentGroup && nextGroup) {
        const curBtns = currentGroup.querySelectorAll("sl-button[href]");
        const nextBtns = nextGroup.querySelectorAll("sl-button[href]");
        const n = Math.min(curBtns.length, nextBtns.length);
        for (let i = 0; i < n; i++) {
          const nh = nextBtns[i].getAttribute("href");
          if (nh) curBtns[i].setAttribute("href", nh);
        }
      }

      const curLaunch = currentHeader.querySelector("sl-button.topbar-launch-btn");
      const nextLaunch = nextHeader.querySelector("sl-button.topbar-launch-btn");
      if (curLaunch && nextLaunch) {
        const nh = nextLaunch.getAttribute("href");
        if (nh) curLaunch.setAttribute("href", nh);
      }

      const curTabs = currentHeader.querySelectorAll("nav.app-tabs a[href]");
      const nextTabs = nextHeader.querySelectorAll("nav.app-tabs a[href]");
      const m = Math.min(curTabs.length, nextTabs.length);
      for (let j = 0; j < m; j++) {
        const h = nextTabs[j].getAttribute("href");
        if (h) curTabs[j].setAttribute("href", h);
      }

      const curBread = currentHeader.querySelector(".app-current-path");
      const nextBread = nextHeader.querySelector(".app-current-path");
      if (curBread && nextBread) {
        curBread.replaceWith(nextBread.cloneNode(true));
      }
    } catch (err) {
      console.warn("[spa-navigation] sync topbar skipped", err);
    }
  }

  function swapManageWorkspace(doc, url, replaceHistory) {
    const currentShell = document.querySelector(".shell");
    const nextShell = doc.querySelector(".shell");
    const currentWorkspace = document.getElementById("workspace-root");
    const nextWorkspace = doc.getElementById("workspace-root");
    const currentLeftSidebar =
      currentWorkspace && currentWorkspace.querySelector("aside.sidebar.left");
    const nextLeftSidebar =
      nextWorkspace && nextWorkspace.querySelector("aside.sidebar.left");
    const currentMain = currentWorkspace && currentWorkspace.querySelector("main.main");
    const nextMain = nextWorkspace && nextWorkspace.querySelector("main.main");
    const currentRightSidebar =
      currentWorkspace && currentWorkspace.querySelector("aside.sidebar.right");
    const nextRightSidebar =
      nextWorkspace && nextWorkspace.querySelector("aside.sidebar.right");
    const currentStatusbar = document.querySelector(".statusbar");
    const nextStatusbar = doc.querySelector(".statusbar");
    const nextPanelRoot =
      nextRightSidebar && nextRightSidebar.querySelector("#meilang-author-panel");
    const nextPanelContext = extractManagePanelContext(nextPanelRoot);

    if (
      !currentShell ||
      !nextShell ||
      !currentWorkspace ||
      !currentLeftSidebar ||
      !nextLeftSidebar ||
      !currentMain ||
      !nextMain ||
      !currentRightSidebar ||
      !nextRightSidebar
    ) {
      return false;
    }

    currentShell.className = nextShell.className;
    syncElementAttributes(currentWorkspace, nextWorkspace, { preserve: ["id"] });
    syncSidebarLinkState(currentLeftSidebar, nextLeftSidebar);
    currentRightSidebar.className = nextRightSidebar.className;
    const preparedMain = cloneNodeOrNull(nextMain);
    if (!preparedMain) return false;
    preparedMain.classList.add("spa-fragment-enter");
    currentMain.replaceWith(preparedMain);
    if (currentStatusbar && nextStatusbar) {
      syncStatusbarContent(currentStatusbar, nextStatusbar);
    }

    syncManageTopbarFromDoc(doc);

    if (replaceHistory) {
      window.history.replaceState({}, "", url);
    } else {
      window.history.pushState({}, "", url);
    }
    return true;
  }

  function runPostSpaWork(doc, url, navigationId, currentUrl, nextUrl) {
    void (async () => {
      try {
        if (navigationId !== currentNavigationId) return;
        if (!preserveManageWorkspaceFromUrls(currentUrl, nextUrl)) {
          const bundlesReady = await ensureHostBundlesFromDoc(
            doc,
            navigationId,
            currentUrl,
            nextUrl,
          );
          if (!bundlesReady || navigationId !== currentNavigationId) return;
        }
        if (navigationId !== currentNavigationId) return;
        await syncMissingWorkspaceModulesOnly(doc, navigationId);
        if (navigationId !== currentNavigationId) return;
        publishManagePreviewFromDoc(doc, { resetRuntimeQueryCache: false });
        if (nextUrl.pathname.startsWith("/apps/manage/")) {
          if (typeof boot.installManageTabs === "function") {
            boot.installManageTabs();
          }
          if (typeof boot.mountSourceTreeControls === "function") {
            boot.mountSourceTreeControls();
          }
          syncManageTabFromUrl(url);
          pulseManagePreview(extractManagePanelContext(document.querySelector("#meilang-author-panel")), {
            resetRuntimeQueryCache: false,
          });
        }
        installSceneProjectionHost();
        applyDrilldownContextFromQuery();
        applySceneProjectionContextFromStorage();
      } catch (err) {
        console.warn("[spa-navigation] post-spa work failed", err);
      }
    })();
  }

  function preserveManageWorkspaceFromUrls(currentUrl, nextUrl) {
    return shouldPreserveManageWorkspace(currentUrl, nextUrl);
  }

  async function loadAndSwap(url, replaceHistory, navigationId) {
    const fetchController = new AbortController();
    const fetchTimer = setTimeout(() => fetchController.abort(), SPA_FETCH_TIMEOUT_MS);
    let response;
    try {
      response = await fetch(url, {
        credentials: "same-origin",
        headers: { "x-mei-spa-nav": "1" },
        signal: fetchController.signal,
      });
    } finally {
      clearTimeout(fetchTimer);
    }
    if (!response.ok) throw new Error("navigation failed: " + response.status);
    const html = await response.text();
    if (navigationId !== currentNavigationId) return false;
    const doc = new DOMParser().parseFromString(html, "text/html");
    const nextShell = doc.querySelector(".shell");
    const currentShell = document.querySelector(".shell");
    if (!nextShell || !currentShell) {
      const err = new Error("spa shell missing in response");
      err.meiSpaHardNav = true;
      throw err;
    }
    if (shouldForceHardNavForSceneBundleSwitch(doc)) {
      const err = new Error("scene bundle changed; fallback to hard navigation");
      err.meiSpaHardNav = true;
      throw err;
    }
    const currentUrl = new URL(window.location.href);
    const nextUrl = new URL(url, window.location.href);
    const preserveManageWorkspace = shouldPreserveManageWorkspace(currentUrl, nextUrl);
    disposeRuntimeHooks({
      preserveAgentPanel: preserveManageWorkspace,
      preserveStatusBar: preserveManageWorkspace,
      preserveManageTabs: preserveManageWorkspace,
      preserveWorkspaceSplitters: preserveManageWorkspace,
      preserveFrameStage: preserveManageWorkspace,
      preserveSourceTreeControls: preserveManageWorkspace,
      preserveSourceHighlight: preserveManageWorkspace,
    });
    if (preserveManageWorkspace && typeof window.__meiClearRuntimePerfDiagnostics === "function") {
      try {
        window.__meiClearRuntimePerfDiagnostics("SPA 换文件");
      } catch (_) {}
    }
    document.title = doc.title || document.title;
    if (document.body.className !== doc.body.className) {
      document.body.className = doc.body.className;
    }
    if (preserveManageWorkspace) {
      const swapped = swapManageWorkspace(doc, url, replaceHistory);
      if (!swapped) {
        replaceShellFromDoc(doc, url, replaceHistory);
      }
    } else {
      replaceShellFromDoc(doc, url, replaceHistory);
    }
    if (navigationId !== currentNavigationId) return false;
    runPostSpaWork(doc, url, navigationId, currentUrl, nextUrl);
    return true;
  }

  async function navigateInternal(url, replaceHistory) {
    currentNavigationId += 1;
    const navigationId = currentNavigationId;
    spaNavigationInFlight += 1;
    boot._spaInFlight = spaNavigationInFlight;
    requestRuntimeAbort("spa_navigation");
    closeDrilldownOverlay();
    let currentUrl = null;
    let nextUrl = null;
    try {
      currentUrl = new URL(window.location.href);
      nextUrl = new URL(url, window.location.href);
    } catch (_) {}
    const manageSamePath =
      currentUrl && nextUrl && isManageSamePathNavigation(currentUrl, nextUrl);
    if (manageSamePath) {
      showManageWorkspaceLoadingState(url);
    } else {
      showManageWorkspaceLoadingState(url);
      showLoading();
    }
    try {
      const completed = await loadAndSwap(url, replaceHistory, navigationId);
      if (!completed && navigationId === currentNavigationId) {
        console.warn("[spa-navigation] navigation superseded", url);
      }
    } catch (error) {
      console.error("[spa-navigation] navigation failed", error);
      if (error && error.name === "AbortError") {
        console.warn("[spa-navigation] fetch timeout", url);
        if (window.MeiHostHttpFeedback && typeof window.MeiHostHttpFeedback.notify === "function") {
          window.MeiHostHttpFeedback.notify({
            status: 504,
            url: url,
            title: "页面加载超时",
            message: "SPA 导航等待宿主响应超时，请稍后重试或刷新页面。",
          });
        }
        return;
      }
      if (error && error.meiSpaHardNav) {
        window.location.assign(url);
        return;
      }
      const statusMatch =
        error && error.message && String(error.message).match(/navigation failed:\s*(\d{3})/);
      const status = statusMatch ? Number(statusMatch[1]) : 500;
      if (window.MeiHostHttpFeedback && typeof window.MeiHostHttpFeedback.notify === "function") {
        window.MeiHostHttpFeedback.notify({
          status: status,
          url: url,
          title: status === 404 ? "页面不存在" : "页面加载失败",
          message:
            (error && error.message ? String(error.message) : "导航失败") +
            "。可尝试刷新或联系管理员。",
        });
      } else {
        window.location.assign(url);
      }
    } finally {
      spaNavigationInFlight = Math.max(0, spaNavigationInFlight - 1);
      boot._spaInFlight = spaNavigationInFlight;
      finishNavigationUi(navigationId);
    }
  }

  boot.navigateSpa = function (url, replaceHistory) {
    return navigateInternal(url, !!replaceHistory);
  };

  tagExistingBodyScripts();
  installSceneProjectionHost();
  applyDrilldownContextFromQuery();
  applySceneProjectionContextFromStorage();

  document.addEventListener(
    "click",
    (event) => {
      if (event.defaultPrevented) return;
      if (event.button !== 0) return;
      if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
      if (shouldBypassSpaClick(event)) {
        if (shouldAbortRuntimeForBypassNavigation(event)) {
          requestRuntimeAbort("full_navigation_bypass");
        }
        return;
      }
      const target = resolveClickTarget(event);
      if (!target) return;
      if (target.download) return;
      if (target.target && target.target !== "_self") return;
      if (!shouldHandleUrl(target.url)) return;
      if (isSameLocation(target.url)) {
        event.preventDefault();
        return;
      }
      event.preventDefault();
      void navigateInternal(target.url, false);
    },
    true,
  );

  window.addEventListener("popstate", () => {
    closeDrilldownOverlay();
    if (shouldHandleUrl(window.location.href)) {
      void navigateInternal(window.location.href, true);
    }
  });
})();
