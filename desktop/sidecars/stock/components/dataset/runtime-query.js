import {
  clearSessionRuntimeQueryCaches,
  clientQueryCacheConfig,
  enumerateSessionRuntimeQueryCaches,
  persistMemoryRuntimeQueryCaches,
  readSessionRuntimeQueryCache,
  writeSessionRuntimeQueryCache,
} from "./runtime-query-session-cache.js";
import {
  hydrateQueryStateStore,
  installQueryStatePersistence,
} from "./query-state-store.js";

const STORE_KEY = "__meiQueryStateStore";
const EVENT_NAME = "mei:query-state-change";
const FALLBACK_EVAL_CACHES = {
  metricInflight: new Map(),
  metricResults: new Map(),
  metricScopeInflight: new Map(),
  metricScopeResults: new Map(),
  datasetInflight: new Map(),
  datasetResults: new Map(),
};

function cacheStore() {
  if (typeof window !== "undefined" && window.__meiEvalStoreCaches) {
    return window.__meiEvalStoreCaches;
  }
  return FALLBACK_EVAL_CACHES;
}

const METRIC_QUERY_CACHE_TTL_MS = 300_000;
const DATASET_QUERY_CACHE_TTL_MS = 300_000;
const SCENE_METRIC_BATCH_INFLIGHT = new Map();
const SCENE_METRIC_BATCH_SCHEDULES = new Map();
const SCENE_METRIC_BATCH_FLUSH_DELAY_MS = 32;
let sceneMetricBatchFlushTimer = null;
const ACTIVE_RUNTIME_FETCH_CONTROLLERS = new Set();
const PARSED_DATA_PROPS_CACHE = new WeakMap();
export const MEI_DRILLDOWN_OVERLAY_ID = "mei-access-drilldown-overlay";
export const MEI_SCENE_BOARD_OVERLAY_ID = "mei-access-scene-board-overlay";

/** 元素是否位于任一运行时二级看板 overlay 内（含 scene-board 与 generic drilldown）。 */
export function isRuntimeDrilldownOverlayElement(element) {
  if (!(element instanceof Element)) {
    return false;
  }
  return Boolean(
    element.closest(`#${MEI_DRILLDOWN_OVERLAY_ID}`) ||
      element.closest(`#${MEI_SCENE_BOARD_OVERLAY_ID}`),
  );
}
export const MEI_PREFETCH_PANEL_METRICS = "meilang:prefetch-panel-metrics";
export const MEI_RUNTIME_QUERY_READY = "meilang:runtime-query-ready";
export const MEI_ABORT_RUNTIME_QUERIES = "mei:abort-runtime-queries";
const PANEL_METRIC_BATCHES = new Map();

function runtimePerfDisableSet() {
  if (typeof window === "undefined") {
    return new Set();
  }
  const raw = [];
  try {
    const query = new URLSearchParams(window.location.search || "");
    raw.push(query.get("mei_perf_disable") || "");
  } catch (_) {
    /* ignore */
  }
  const globalValue = window.__MEI_PERF_DISABLE__;
  if (Array.isArray(globalValue)) {
    raw.push(globalValue.join(","));
  } else if (typeof globalValue === "string") {
    raw.push(globalValue);
  }
  return new Set(
    raw
      .join(",")
      .split(",")
      .map((item) => String(item || "").trim().toLowerCase())
      .filter(Boolean)
  );
}

function runtimePerfDisabled(flag) {
  return runtimePerfDisableSet().has(String(flag || "").trim().toLowerCase());
}

export function parseProps(element) {
  if (!(element instanceof Element)) {
    return {};
  }
  const raw = String(element.getAttribute("data-props") || "");
  const cached = PARSED_DATA_PROPS_CACHE.get(element);
  if (cached && cached.raw === raw) {
    return cached.value;
  }
  let value = {};
  try {
    value = raw ? JSON.parse(raw) : {};
  } catch {
    value = {};
  }
  PARSED_DATA_PROPS_CACHE.set(element, { raw, value });
  return value;
}

function safeTrim(value) {
  return String(value ?? "").trim();
}

function runtimePreviewScope(props, element = null) {
  const fromProps = safeTrim(
    props?._mei?.preview_scope ||
      props?._mei?.previewScope ||
      props?.preview_scope ||
      props?.previewScope,
  );
  if (fromProps) return fromProps;
  if (typeof window === "undefined") return "";
  const resolver = window.__meiLangBoot?.devEvalScopeFromElement;
  return typeof resolver === "function" ? safeTrim(resolver(element)) : "";
}

function devEvalAllowsRuntimeQuery(props, element = null) {
  if (typeof window === "undefined") return true;
  const allows = window.__meiLangBoot?.devEvalAllowsRuntimeQuery;
  return typeof allows !== "function" || allows(props, element);
}

function staticDatasetRowsForBlockedQuery(props) {
  if (typeof window === "undefined") return null;
  const metricRef = resolveRuntimeMetricRef(props);
  const rows = window.__meiLangBoot?.devEvalStaticDatasetRows?.(metricRef?.metric_id);
  if (!Array.isArray(rows) || !rows.length) {
    return null;
  }
  return {
    rows,
    total_rows: rows.length,
    perf: { static_placeholder: 1 },
  };
}

function staticMetricResultForBlockedQuery(props, metricIds = []) {
  if (typeof window === "undefined") return null;
  const scalar = window.__meiLangBoot?.devEvalScalarFromFixture;
  if (typeof scalar !== "function") return null;
  const requested = [...new Set(
    (Array.isArray(metricIds) ? metricIds : [])
      .map((value) => safeTrim(value))
      .filter(Boolean),
  )];
  const refMetricId = safeTrim(resolveRuntimeMetricRef(props)?.metric_id);
  const ids = requested.length ? requested : refMetricId ? [refMetricId] : [];
  if (!ids.length) return null;
  const metrics = ids
    .map((metricId) => {
      const entry = scalar(metricId);
      if (!entry) return null;
      return {
        id: metricId,
        value: entry.value,
        label: entry.label,
        unit: entry.unit,
        rows: [],
      };
    })
    .filter(Boolean);
  if (!metrics.length) return null;
  return {
    metrics,
    total_rows: metrics.length,
    perf: { static_placeholder: 1 },
  };
}

function activePageSceneId() {
  if (typeof document === "undefined") return "";
  return safeTrim(
    document.body?.getAttribute?.("data-scene-id") ||
      document.body?.dataset?.sceneId ||
      "",
  );
}

function activePageTarget() {
  if (typeof window === "undefined") return "";
  return safeTrim(
    window.__mei?.bootstrap_target_file ||
      document.body?.getAttribute?.("data-target") ||
      "",
  );
}

export function runtimeCallerMeta(element, fallbackComponent = "") {
  const props = parseProps(element);
  const queryStateId = queryStateIdOf(props);
  const panelId =
    element?.closest?.("[data-mei-panel-id]")?.getAttribute?.("data-mei-panel-id") ||
    "";
  const component =
    safeTrim(fallbackComponent) ||
    safeTrim(element?.tagName || "").toLowerCase() ||
    undefined;
  return {
    component,
    panel_id: safeTrim(panelId) || undefined,
    scene_id: safeTrim(props?._mei?.active_scene_id) || activePageSceneId() || undefined,
    target:
      safeTrim(props?._mei?.active_target_file || props?._mei?.entry_target) ||
      activePageTarget() ||
      undefined,
    query_state_id: safeTrim(queryStateId) || undefined,
  };
}

export function escapeHtml(value) {
  return String(value ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;");
}

export function escapeHtmlAttr(value) {
  return escapeHtml(value).replaceAll('"', "&quot;");
}

/** 兼容层：历史 `target` 字段（source locator） */
function datasetCompileTarget(props) {
  const raw = props?._mei?.entry_target ?? props?._mei?.active_target_file;
  const s = String(raw ?? "").trim();
  return s || undefined;
}

/** 从 imported world metrics owner id 提取 capsule compile target（如 scenes/10-地图.mei）。 */
function importedWorldMetricsCompileTarget(datasetId) {
  const id = String(datasetId || "").trim();
  const match = /^__world_metrics__::(.+)::metrics$/.exec(id);
  return match ? String(match[1] || "").trim() : "";
}

/** scene-first 寻址：优先 runtime ref，其次 SSR 注入的 active_scene_id */
function sceneQueryCoords(props, runtimeRef) {
  const sceneId = String(
    runtimeRef?.scene_id ?? props?._mei?.active_scene_id ?? activePageSceneId()
  ).trim();
  const importedTarget = importedWorldMetricsCompileTarget(runtimeRef?.dataset_id);
  const pageTarget = datasetCompileTarget(props) || activePageTarget();
  const metricScenePath = String(runtimeRef?.scene_path ?? "").trim();
  const coords = {};
  if (sceneId) coords.scene_id = sceneId;
  // 嵌入 capsule 的 world metrics 在独立 scene 文件下 compile；须对齐 prebuild scope。
  if (importedTarget) {
    coords.target = importedTarget;
  } else if (pageTarget) {
    coords.target = pageTarget;
  } else if (metricScenePath) {
    coords.target = metricScenePath;
  }
  return coords;
}

function readGlobalHostRuntimeCapabilities() {
  if (typeof window === "undefined") return null;
  const cached = window.__meiHostRuntimeCapabilities;
  if (cached && typeof cached === "object" && !Array.isArray(cached)) {
    return cached;
  }
  const script = document.getElementById("mei-host-runtime-capabilities");
  const raw = String(script?.textContent || "").trim();
  if (!raw) return null;
  try {
    const parsed = JSON.parse(raw);
    if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
      window.__meiHostRuntimeCapabilities = parsed;
      return parsed;
    }
  } catch (_) {
    /* ignore */
  }
  return null;
}

function runtimeCapabilityMap(props) {
  const raw =
    props?._mei?.runtime_capabilities ??
    props?._mei?.runtimeCapabilities ??
    readGlobalHostRuntimeCapabilities();
  if (!raw || typeof raw !== "object" || Array.isArray(raw)) {
    return {};
  }
  return raw;
}

export function isStaticSkeletonDisplay(props) {
  if (typeof document !== "undefined") {
    const body = document.body;
    if (body instanceof HTMLElement) {
      if (body.getAttribute("data-mei-prototype") === "true") {
        return true;
      }
      const surface = String(body.getAttribute("data-surface") || body.dataset.surface || "")
        .trim()
        .toLowerCase();
      if (surface === "prototype") {
        return true;
      }
    }
  }
  const caps = runtimeCapabilityMap(props);
  if (caps?.static_display?.enabled) {
    return true;
  }
  const candidates = [
    props?.content,
    props?.metric,
    props?.data,
    props?.value,
    props?.dataset,
    props?.dataset?.dataset,
  ];
  for (const candidate of candidates) {
    if (!candidate || typeof candidate !== "object" || Array.isArray(candidate)) {
      continue;
    }
    if (candidate.__mei_data_origin === "static_skeleton") {
      return true;
    }
    if (candidate.dataset?.__mei_data_origin === "static_skeleton") {
      return true;
    }
  }
  return false;
}

function normalizeRuntimeQueryCapability(
  rawCapability,
  {
    defaultRequiresSceneId = true,
    missingEndpointReason = "missing runtime query endpoint",
    disabledReason = "runtime query capability disabled by host/runtime contract",
  } = {}
) {
  if (!rawCapability || typeof rawCapability !== "object" || Array.isArray(rawCapability)) {
    return {
      enabled: false,
      api: "",
      requiresSceneId: defaultRequiresSceneId,
      source: "none",
      reason: missingEndpointReason,
    };
  }
  const api = safeTrim(rawCapability.api || rawCapability.endpoint || rawCapability.url);
  const enabledFlag = rawCapability.enabled;
  const sceneQualifiedFlag =
    rawCapability.scene_qualified ??
    rawCapability.sceneQualified ??
    rawCapability.requires_scene_id ??
    rawCapability.requiresSceneId;
  const requiresSceneId =
    typeof sceneQualifiedFlag === "boolean"
      ? sceneQualifiedFlag
      : defaultRequiresSceneId;
  if (enabledFlag === false) {
    return {
      enabled: false,
      api,
      requiresSceneId,
      source: "runtime_capabilities",
      reason: disabledReason,
    };
  }
  if (!api) {
    return {
      enabled: false,
      api: "",
      requiresSceneId,
      source: "runtime_capabilities",
      reason: missingEndpointReason,
    };
  }
  return {
    enabled: true,
    api,
    requiresSceneId,
    source: "runtime_capabilities",
    reason: "",
  };
}

function rowsQueryCapabilityConfig(props) {
  const capabilities = runtimeCapabilityMap(props);
  const raw =
    capabilities.rows_query ??
    capabilities.rowsQuery ??
    capabilities.dataset_rows_query ??
    capabilities.datasetRowsQuery;
  return normalizeRuntimeQueryCapability(
    raw,
    {
      defaultRequiresSceneId: true,
      missingEndpointReason: "missing runtime rows query endpoint",
      disabledReason: "runtime rows query capability disabled by host/runtime contract",
    }
  );
}

function metricQueryCapabilityConfig(props) {
  const capabilities = runtimeCapabilityMap(props);
  const raw =
    capabilities.metric_query ??
    capabilities.metricQuery ??
    capabilities.metrics_query ??
    capabilities.metricsQuery;
  return normalizeRuntimeQueryCapability(
    raw,
    {
      defaultRequiresSceneId: true,
      missingEndpointReason: "missing runtime metric query endpoint",
      disabledReason: "runtime metric query capability disabled by host/runtime contract",
    }
  );
}

function metricBatchQueryCapabilityConfig(props) {
  const capabilities = runtimeCapabilityMap(props);
  const raw =
    capabilities.metric_batch_query ??
    capabilities.metricBatchQuery;
  return normalizeRuntimeQueryCapability(
    raw,
    {
      defaultRequiresSceneId: true,
      missingEndpointReason: "missing runtime metric batch query endpoint",
      disabledReason: "runtime metric batch query capability disabled by host/runtime contract",
    }
  );
}

export function resolveDatasetQueryCapability(props) {
  const config = rowsQueryCapabilityConfig(props);
  const api = config.api;
  const metricRef = resolveRuntimeMetricRef(props);
  const dataRef = resolveRuntimeDataRef(props);
  const dataset = resolveDatasetLike(props);
  const runtimeRef = metricRef || dataRef;
  const datasetId = String(
    metricRef?.dataset_id || dataRef?.dataset_id || dataset?.id || ""
  ).trim();
  const coords = sceneQueryCoords(props, runtimeRef);
  if (!config.enabled) {
    return {
      enabled: false,
      api,
      datasetId,
      sceneId: safeTrim(coords.scene_id),
      requiresSceneId: config.requiresSceneId,
      source: config.source,
      reason:
        `shared runtime dataset query capability is unavailable (${config.reason})`,
    };
  }
  if (!datasetId) {
    return {
      enabled: false,
      api,
      datasetId,
      sceneId: safeTrim(coords.scene_id),
      requiresSceneId: config.requiresSceneId,
      source: config.source,
      reason:
        "shared runtime dataset query capability is unavailable (missing dataset binding)",
    };
  }
  if (config.requiresSceneId && !safeTrim(coords.scene_id)) {
    return {
      enabled: false,
      api,
      datasetId,
      sceneId: "",
      requiresSceneId: true,
      source: config.source,
      reason:
        "shared runtime dataset query capability is unavailable (missing scene_id for scene-qualified query)",
    };
  }
  return {
    enabled: true,
    api,
    datasetId,
    sceneId: safeTrim(coords.scene_id),
    requiresSceneId: config.requiresSceneId,
    source: config.source,
    reason: "",
  };
}

function requireSceneQualifiedRequest(coords, requestKind, meta = {}) {
  const sceneId = safeTrim(coords?.scene_id);
  if (sceneId) {
    return { ...coords, scene_id: sceneId };
  }
  const component = safeTrim(meta?.component || meta?.fallbackComponent);
  const target = safeTrim(coords?.target);
  const detail = target ? ` (legacy target-only request: ${target})` : "";
  const source = component ? ` for ${component}` : "";
  throw new Error(
    `${requestKind}${source} requires scene_id; target-only runtime requests are no longer supported${detail}`
  );
}

/** 与 `mei-lang/host-shell/app/assets/manage-tabs.js` 发出的标签切换事件一致 */
export const MEI_MANAGE_TAB_CHANGE = "mei:manage-tab-change";

export function previewUpdatedScope(event) {
  const detail = event?.detail;
  if (detail && typeof detail === "object" && detail.scope != null) {
    return String(detail.scope).trim() || "page";
  }
  return "page";
}

export function isDrilldownOverlayOpen() {
  if (typeof document === "undefined") {
    return false;
  }
  return (
    document.body?.classList?.contains("access-drilldown-open") === true ||
    document.body?.classList?.contains("access-scene-board-open") === true ||
    document.body?.classList?.contains("access-layer2-open") === true
  );
}

function drilldownOverlayViewportRoots() {
  if (typeof document === "undefined") {
    return [];
  }
  const roots = [];
  if (document.body?.classList?.contains("access-layer2-open")) {
    document
      .querySelectorAll("[data-layer2-tab-panel]:not([hidden])")
      .forEach((panel) => roots.push(panel));
  }
  if (document.body?.classList?.contains("access-scene-board-open")) {
    const sceneBoard = document.getElementById("mei-access-scene-board-overlay");
    if (sceneBoard instanceof Element && !sceneBoard.hasAttribute("hidden")) {
      roots.push(sceneBoard);
    }
  }
  if (document.body?.classList?.contains("access-drilldown-open")) {
    const drilldown = document.getElementById("mei-access-drilldown-overlay");
    if (drilldown instanceof Element && !drilldown.hasAttribute("hidden")) {
      roots.push(drilldown);
    }
  }
  return roots;
}

function recentScopeActivationMatches(props, windowMs = 15000) {
  if (typeof window === "undefined") {
    return false;
  }
  const last = window.__meiLastScopeActivation;
  if (!last || typeof last !== "object") {
    return false;
  }
  const at = Number(last.at || 0);
  if (!Number.isFinite(at) || Date.now() - at > windowMs) {
    return false;
  }
  const scene = String(
    props?._mei?.scene_id ?? props?._mei?.scene ?? props?.scene_id ?? "",
  ).trim();
  const scope = String(last.scope || last.sceneId || "").trim();
  return !scene || !scope || scene === scope;
}

function sleepMs(ms) {
  return new Promise((resolve) => {
    setTimeout(resolve, ms);
  });
}

function isHomeViewportRuntimeProps(props) {
  const target = String(
    props?._mei?.entry_target ?? props?._mei?.active_target_file ?? ""
  ).trim();
  if (!target) {
    return true;
  }
  const normalized = target.replace(/\\/g, "/");
  return normalized === "main.mei" || normalized.endsWith("/main.mei");
}

/** overlay 打开时暂停主屏（main.mei）metric 拉取，避免误触发 batch:9。 */
export function shouldPauseHomeRuntimeMetricFetch(props) {
  return isDrilldownOverlayOpen() && isHomeViewportRuntimeProps(props);
}

/**
 * 二级看板 overlay 打开时，主屏组件不应响应 preview-updated；overlay 内组件仍应刷新。
 */
export function shouldReactToPreviewUpdated(event, element) {
  const scope = previewUpdatedScope(event);
  const inOverlay = isRuntimeDrilldownOverlayElement(element);
  if (scope === "drilldown") {
    return inOverlay;
  }
  if (isDrilldownOverlayOpen() && !inOverlay) {
    return false;
  }
  return true;
}

/**
 * Build 预览 scoped dim：非聚焦 panel 仅降透明度，仍会参与 layout；
 * runtime prefetch 必须跳过，避免用错误 scene compile 上下文批量打 API。
 */
function isBuildPreviewScopedDim(el) {
  if (!(el instanceof Element)) {
    return false;
  }
  if (typeof window === "undefined") {
    return false;
  }
  if (!/^\/apps\/(?:build|manage)\//.test(String(window.location.pathname || ""))) {
    return false;
  }
  return Boolean(el.closest(".build-preview-scoped-dim"));
}

/**
 * 判断元素是否参与布局（未被 HTML `hidden` / display:none 等裁掉）。
 * 管理页「应用预览」在 diagnostics/source 标签下会带 `hidden`，子节点仍会被插入 DOM，
 * 但不应触发 dataset / 图表等重活。
 */
export function elementIsDisplayed(el) {
  if (!el || !(el instanceof Element) || !el.isConnected) {
    return false;
  }
  if (isBuildPreviewScopedDim(el)) {
    return false;
  }
  let node = el;
  while (node) {
    if (node instanceof HTMLElement && node.hasAttribute("hidden")) {
      return false;
    }
    try {
      const st = window.getComputedStyle(node);
      if (st.display === "none" || st.visibility === "hidden") {
        return false;
      }
    } catch (_) {
      /* ignore */
    }
    node = node.parentElement;
  }
  return true;
}

/**
 * 在元素可见后再执行 `fn`（管理页切换回「应用预览」时也会触发）。
 * @returns 取消函数：在 disconnected 时调用，避免卸载后仍执行 `fn`
 */
export function deferUntilDisplayed(el, fn) {
  if (!el || typeof fn !== "function") {
    return () => {};
  }
  if (typeof window === "undefined") {
    try {
      fn();
    } catch (_) {
      /* ignore */
    }
    return () => {};
  }

  let done = false;
  let canceled = false;
  let io = null;
  let hiddenObserver = null;
  let fallbackTimer = null;

  const cleanupWatchers = () => {
    document.removeEventListener(MEI_MANAGE_TAB_CHANGE, onManageTab);
    window.removeEventListener("pageshow", onPageShow);
    window.removeEventListener("meilang:preview-updated", onPreviewUpdated);
    window.removeEventListener("meilang:prefetch-panel-metrics", onPreviewUpdated);
    if (io) {
      try {
        io.disconnect();
      } catch (_) {
        /* ignore */
      }
      io = null;
    }
    if (hiddenObserver) {
      try {
        hiddenObserver.disconnect();
      } catch (_) {
        /* ignore */
      }
      hiddenObserver = null;
    }
    if (fallbackTimer != null) {
      window.clearTimeout(fallbackTimer);
      fallbackTimer = null;
    }
  };

  const finish = () => {
    if (done || canceled) return;
    if (!el.isConnected) {
      canceled = true;
      cleanupWatchers();
      return;
    }
    if (!elementIsDisplayed(el)) return;
    done = true;
    cleanupWatchers();
    try {
      fn();
    } catch (_) {
      /* ignore */
    }
  };

  const tryRun = () => {
    if (done || canceled) return;
    finish();
  };

  function onManageTab() {
    requestAnimationFrame(() => tryRun());
  }

  function onPageShow() {
    requestAnimationFrame(() => tryRun());
  }

  /** 预览 DOM / frame viewport 缩放完成后，零尺寸 panel 的 IO 才会变为可见 */
  function onPreviewUpdated(event) {
    if (!shouldReactToPreviewUpdated(event, el)) {
      return;
    }
    if (previewUpdatedScope(event) === "page") {
      requestAnimationFrame(() => {
        requestAnimationFrame(() => {
          tryRun();
        });
      });
      return;
    }
    requestAnimationFrame(() => tryRun());
  }

  tryRun();
  if (done || canceled) {
    return () => {
      canceled = true;
      cleanupWatchers();
    };
  }

  document.addEventListener(MEI_MANAGE_TAB_CHANGE, onManageTab);
  window.addEventListener("pageshow", onPageShow);
  window.addEventListener("meilang:preview-updated", onPreviewUpdated);
  window.addEventListener("meilang:prefetch-panel-metrics", onPreviewUpdated);

  if (window.IntersectionObserver) {
    try {
      io = new IntersectionObserver(
        () => {
          tryRun();
        },
        { threshold: 0, root: null, rootMargin: "480px 480px 480px 480px" },
      );
      io.observe(el);
    } catch (_) {
      /* ignore */
    }
  }

  // Safari 在祖先 `hidden` 切换后不一定重算 IntersectionObserver；监听 hidden 解除。
  if (typeof MutationObserver !== "undefined") {
    try {
      hiddenObserver = new MutationObserver(() => {
        requestAnimationFrame(() => tryRun());
      });
      let node = el;
      while (node) {
        hiddenObserver.observe(node, {
          attributes: true,
          attributeFilter: ["hidden"],
        });
        node = node.parentElement;
      }
    } catch (_) {
      /* ignore */
    }
  }

  // Keep fallback short so cold path does not wait 10s+ for IO edge cases.
  fallbackTimer = window.setTimeout(() => tryRun(), 2500);

  return () => {
    canceled = true;
    cleanupWatchers();
  };
}

export function queryStateIdOf(props) {
  return String(props?.query_state || props?.queryState || "").trim();
}

export function getQueryState(id) {
  if (!id) return { filters: {}, filter_intents: [] };
  const store = ensureStore();
  return normalizeQueryState(store[id] || { filters: {}, filter_intents: [] });
}

export function setQueryState(id, nextState, options = {}) {
  if (!id) return { filters: {}, filter_intents: [] };
  const store = ensureStore();
  const current = normalizeQueryState(store[id] || { filters: {}, filter_intents: [] });
  const candidate = {
    ...current,
    ...(nextState && typeof nextState === "object" ? nextState : {}),
  };
  const transitionSource = normalizeFilterIntentSource(
    options?.transitionSource ?? options?.filterIntentSource ?? nextState?.transition_source ?? nextState?.filter_intent_source,
    ""
  );
  const explicitFilterIntents = Array.isArray(nextState?.filter_intents)
    ? nextState.filter_intents
    : Array.isArray(nextState?.filterIntents)
      ? nextState.filterIntents
      : [];
  if (Object.prototype.hasOwnProperty.call(candidate, "filters")) {
    const normalizedFilters = mergeFilters(candidate.filters);
    const nextFilterSource = normalizeFilterIntentSource(
      options?.filterIntentSource ?? nextState?.filter_intent_source ?? nextState?.filterIntentSource,
      "query_state"
    );
    if (explicitFilterIntents.length > 0) {
      candidate.filter_intents = explicitFilterIntents;
    } else {
      const sourceByDimension = filterIntentSourceMap(current.filter_intents);
      const patchDimensions = Array.isArray(options?.filterIntentDimensions)
        ? options.filterIntentDimensions
        : Object.keys(
            nextState && typeof nextState === "object" && nextState.filters && typeof nextState.filters === "object"
              ? nextState.filters
              : {}
          );
      patchDimensions.forEach((dimension) => {
        const normalizedDimension = String(dimension || "").trim();
        if (!normalizedDimension) return;
        if (Object.prototype.hasOwnProperty.call(normalizedFilters, normalizedDimension)) {
          sourceByDimension.set(normalizedDimension, nextFilterSource);
        } else {
          sourceByDimension.delete(normalizedDimension);
        }
      });
      candidate.filter_intents = filterIntentsFromFilters(normalizedFilters, "query_state", sourceByDimension);
    }
  }
  if (transitionSource) {
    candidate.last_transition = {
      source: transitionSource,
      at: Date.now(),
    };
  }
  const normalized = normalizeQueryState(candidate);
  store[id] = normalized;
  window.dispatchEvent(
    new CustomEvent(EVENT_NAME, {
      detail: {
        id,
        state: normalized,
      },
    })
  );
  return normalized;
}

export function subscribeQueryState(id, callback) {
  if (!id || typeof callback !== "function") {
    return () => {};
  }
  const handler = (event) => {
    if (event?.detail?.id !== id) return;
    callback(normalizeQueryState(event.detail.state));
  };
  window.addEventListener(EVENT_NAME, handler);
  callback(getQueryState(id));
  return () => window.removeEventListener(EVENT_NAME, handler);
}

export function mergeFilters(...maps) {
  const out = {};
  for (const item of maps) {
    if (!item || typeof item !== "object") continue;
    for (const [key, value] of Object.entries(item)) {
      const normalizedKey = String(key || "").trim();
      const normalizedValue = String(value ?? "").trim();
      if (!normalizedKey || !normalizedValue) continue;
      out[normalizedKey] = normalizedValue;
    }
  }
  return out;
}

export function sharedFiltersForQueryStateId(queryStateId) {
  const id = String(queryStateId || "").trim();
  if (!id) return {};
  return mergeFilters(getQueryState(id).filters || {});
}

export function sharedFilterIntentsForQueryStateId(queryStateId) {
  const id = String(queryStateId || "").trim();
  if (!id) return [];
  const state = getQueryState(id);
  return Array.isArray(state.filter_intents)
    ? state.filter_intents.map((entry) => ({ ...entry }))
    : [];
}

export function sharedSearchForQueryStateId(queryStateId) {
  const id = String(queryStateId || "").trim();
  if (!id) return "";
  return String(getQueryState(id).search || "").trim();
}

export function setQueryStateFilter(id, dimension, value, options = {}) {
  const queryStateId = String(id || "").trim();
  const normalizedDimension = String(dimension || "").trim();
  if (!queryStateId || !normalizedDimension) {
    return getQueryState(queryStateId);
  }
  const encode = String(options?.encode || options?.filterEncode || "").trim().toLowerCase();
  if (encode === "contains_any" || encode === "contains-any") {
    return toggleQueryStateContainsAnyFilter(queryStateId, normalizedDimension, value, options);
  }
  const current = getQueryState(queryStateId);
  const nextFilters = mergeFilters(current.filters);
  const nextValue = String(value ?? "").trim();
  const shouldToggle = options?.toggle !== false;
  const currentValue = String(nextFilters[normalizedDimension] ?? "").trim();
  if (!nextValue || (shouldToggle && currentValue && currentValue === nextValue)) {
    delete nextFilters[normalizedDimension];
  } else {
    nextFilters[normalizedDimension] = nextValue;
  }
  return setQueryState(
    queryStateId,
    { filters: nextFilters },
    {
      filterIntentSource: options?.filterIntentSource ?? options?.source ?? "unknown",
      transitionSource: options?.transitionSource ?? options?.source ?? "unknown",
      filterIntentDimensions: [normalizedDimension],
    }
  );
}

/** Membership multi-select: toggle one needle inside `contains_any:a,b`. */
export function toggleQueryStateContainsAnyFilter(id, dimension, value, options = {}) {
  const queryStateId = String(id || "").trim();
  const normalizedDimension = String(dimension || "").trim();
  const token = String(value ?? "").trim();
  if (!queryStateId || !normalizedDimension || !token) {
    return getQueryState(queryStateId);
  }
  const current = getQueryState(queryStateId);
  const nextFilters = mergeFilters(current.filters);
  const raw = String(nextFilters[normalizedDimension] ?? "").trim();
  const selected = new Set(parseContainsAnyFilterValues(raw));
  if (selected.has(token)) selected.delete(token);
  else selected.add(token);
  if (selected.size === 0) {
    delete nextFilters[normalizedDimension];
  } else {
    nextFilters[normalizedDimension] = `contains_any:${Array.from(selected).join(",")}`;
  }
  return setQueryState(
    queryStateId,
    { filters: nextFilters },
    {
      filterIntentSource: options?.filterIntentSource ?? options?.source ?? "unknown",
      transitionSource: options?.transitionSource ?? options?.source ?? "unknown",
      filterIntentDimensions: [normalizedDimension],
    }
  );
}

function parseContainsAnyFilterValues(raw) {
  const text = String(raw || "").trim();
  if (!text) return [];
  const body = text.startsWith("contains_any:")
    ? text.slice("contains_any:".length)
    : text.startsWith("contains:")
      ? text.slice("contains:".length)
      : text.startsWith("in:")
        ? text.slice(3)
        : text;
  return body
    .split(",")
    .map((part) => part.trim())
    .filter(Boolean);
}

export function resolveRuntimeDataRef(props) {
  for (const candidate of runtimeCandidates(props)) {
    const ref = candidate?.__mei_runtime_ref;
    if (ref && ref.kind === "data" && ref.dataset_id) {
      return ref;
    }
    const kind = String(candidate?.__ref || "").trim().toLowerCase();
    const datasetId = String(
      candidate?.dataset_id || candidate?.from_dataset || candidate?.id || "",
    ).trim();
    if ((kind === "data" || kind === "dataset") && datasetId) {
      return {
        kind: "data",
        dataset_id: datasetId,
        scene_id: candidate?.scene_id,
        target: candidate?.target,
      };
    }
  }
  return null;
}

export function resolveRuntimeMetricRef(props) {
  for (const candidate of runtimeCandidates(props)) {
    const ref = candidate?.__mei_runtime_ref;
    if (ref && ref.kind === "metric" && ref.dataset_id && ref.metric_id) {
      return ref;
    }
    const kind = String(candidate?.__ref || "").trim().toLowerCase();
    const datasetId = String(
      candidate?.dataset_id || candidate?.from_dataset || "",
    ).trim();
    const metricId = String(candidate?.metric_id || candidate?.id || "").trim();
    if (kind === "metric" && datasetId && metricId) {
      return {
        kind: "metric",
        dataset_id: datasetId,
        metric_id: metricId,
        scene_id: candidate?.scene_id,
        target: candidate?.target,
      };
    }
  }
  return null;
}

/** 年度×月度对比矩阵（month / year / value）优先表格展示，避免误用折线图。 */
export function isYearMonthMatrixMetricConfig(config) {
  if (!config || typeof config !== "object") {
    return false;
  }
  const metricId = String(
    config.tableMetricId ||
      config.runtimeRef?.metric_id ||
      config.runtimeRef?.metricId ||
      "",
  ).trim();
  if (metricId.includes("year_month_matrix")) {
    return true;
  }
  const columns = Array.isArray(config.columns) ? config.columns.map((col) => String(col || "").trim()) : [];
  if (!columns.length) {
    return false;
  }
  const normalized = new Set(columns);
  return normalized.has("month") && normalized.has("year") && normalized.has("value");
}

/** 按 runtime ref 在 metric 查询结果中匹配（兼容短 id 与 `capsule::id` 命名空间 id）。 */
export function findRuntimeMetricInResults(metrics, runtimeRef) {
  if (!Array.isArray(metrics) || !runtimeRef?.metric_id) {
    return null;
  }
  const wanted = String(runtimeRef.metric_id).trim();
  if (!wanted) {
    return null;
  }
  const direct = metrics.find((item) => item && String(item.id || "").trim() === wanted);
  if (direct) {
    return direct;
  }
  const suffix = `::${wanted}`;
  return (
    metrics.find((item) => {
      const id = String(item?.id || "").trim();
      return id === wanted || id.endsWith(suffix);
    }) || null
  );
}

function sameSceneQueryCoords(left, right) {
  return (
    safeTrim(left?.scene_id) === safeTrim(right?.scene_id) &&
    safeTrim(left?.target) === safeTrim(right?.target)
  );
}

function canonicalMetricFilters(queryStateId = "", filters = {}) {
  return mergeFilters(sharedFiltersForQueryStateId(queryStateId), filters);
}

function canonicalMetricSearch(queryStateId = "", search = "") {
  const explicit = String(search || "").trim();
  if (explicit) return explicit;
  return sharedSearchForQueryStateId(queryStateId);
}

function panelMetricBatchKey(panel, props, queryStateId = "", filters = {}, search = "") {
  const runtimeRef = resolveRuntimeMetricRef(props);
  if (!runtimeRef?.dataset_id || !(panel instanceof Element)) {
    return "";
  }
  const effectiveQueryStateId = String(queryStateId || queryStateIdOf(props) || "").trim();
  const coords = sceneQueryCoords(props, runtimeRef);
  const mergedFilters = canonicalMetricFilters(effectiveQueryStateId, filters);
  const mergedSearch = canonicalMetricSearch(effectiveQueryStateId, search);
  return [
    panel.getAttribute("data-mei-panel-id") || "",
    coords.scene_id || "",
    coords.target || "",
    runtimeRef.dataset_id,
    effectiveQueryStateId,
    mergedSearch,
    JSON.stringify(mergedFilters),
  ].join("|");
}

function sceneMetricBatchScopeKey(props, queryStateId = "", filters = {}, search = "") {
  const runtimeRef = resolveRuntimeMetricRef(props);
  if (!runtimeRef?.dataset_id) {
    return "";
  }
  const effectiveQueryStateId = String(queryStateId || queryStateIdOf(props) || "").trim();
  const coords = sceneQueryCoords(props, runtimeRef);
  const mergedFilters = canonicalMetricFilters(effectiveQueryStateId, filters);
  const mergedSearch = canonicalMetricSearch(effectiveQueryStateId, search);
  return [
    coords.scene_id || "",
    coords.target || "",
    effectiveQueryStateId,
    mergedSearch,
    JSON.stringify(mergedFilters),
  ].join("|");
}

function normalizeSceneMetricBatchGroups(groups = []) {
  const dedup = new Map();
  for (const group of Array.isArray(groups) ? groups : []) {
    const datasetId = safeTrim(group?.datasetId || group?.dataset_id);
    if (!datasetId) {
      continue;
    }
    const metricIds = (Array.isArray(group?.metricIds)
      ? group.metricIds
      : Array.isArray(group?.metric_ids)
      ? group.metric_ids
      : [])
      .map((value) => safeTrim(value))
      .filter(Boolean);
    let entry = dedup.get(datasetId);
    if (!entry) {
      entry = {
        dataset_id: datasetId,
        metric_ids: new Set(),
      };
      dedup.set(datasetId, entry);
    }
    metricIds.forEach((metricId) => entry.metric_ids.add(metricId));
  }
  return [...dedup.values()]
    .map((entry) => ({
      dataset_id: entry.dataset_id,
      metric_ids: [...entry.metric_ids].sort(),
    }))
    .filter((entry) => entry.metric_ids.length > 0);
}

function metricIdMatchesRequested(metricId, requestedIds = []) {
  const normalized = safeTrim(metricId);
  if (!normalized) {
    return false;
  }
  for (const requestedId of Array.isArray(requestedIds) ? requestedIds : []) {
    const wanted = safeTrim(requestedId);
    if (!wanted) continue;
    if (normalized === wanted || normalized.endsWith(`::${wanted}`)) {
      return true;
    }
  }
  return false;
}

function filterMetricsForRequestedIds(metrics = [], requestedIds = []) {
  const normalizedRequested = [...new Set(
    (Array.isArray(requestedIds) ? requestedIds : [])
      .map((value) => safeTrim(value))
      .filter(Boolean)
  )];
  if (normalizedRequested.length === 0) {
    return Array.isArray(metrics) ? metrics : [];
  }
  return (Array.isArray(metrics) ? metrics : []).filter((metric) =>
    metricIdMatchesRequested(metric?.id, normalizedRequested)
  );
}

function projectScheduledSingleDatasetMetricResult(batchData, datasetId, requestedIds = []) {
  const normalizedDatasetId = safeTrim(datasetId);
  if (!batchData || !normalizedDatasetId) {
    return null;
  }
  const metrics = filterMetricsForRequestedIds(batchData.metrics, requestedIds);
  return {
    scene_id: safeTrim(batchData.scene_id),
    scene_path: safeTrim(batchData.scene_path) || undefined,
    dataset_id: normalizedDatasetId,
    total_rows: Number(batchData.total_rows) || 0,
    metrics,
    perf: batchData.perf,
  };
}

function flushScheduledSceneMetricBatches() {
  if (sceneMetricBatchFlushTimer != null) {
    clearTimeout(sceneMetricBatchFlushTimer);
    sceneMetricBatchFlushTimer = null;
  }
  for (const scheduleKey of [...SCENE_METRIC_BATCH_SCHEDULES.keys()]) {
    const schedule = SCENE_METRIC_BATCH_SCHEDULES.get(scheduleKey);
    if (!schedule) continue;
    if (typeof schedule.cancelFlush === "function") {
      try {
        schedule.cancelFlush();
      } catch (_) {
        /* ignore */
      }
      schedule.cancelFlush = null;
    }
    if (typeof schedule.flush === "function") {
      attachAbortRejectionGuard(schedule.flush());
    }
  }
}

function scheduleSceneMetricBatchFlush(delayMs = SCENE_METRIC_BATCH_FLUSH_DELAY_MS) {
  if (typeof window === "undefined") {
    flushScheduledSceneMetricBatches();
    return;
  }
  const delay = Math.max(0, Number(delayMs) || 0);
  if (sceneMetricBatchFlushTimer != null) {
    clearTimeout(sceneMetricBatchFlushTimer);
  }
  sceneMetricBatchFlushTimer = window.setTimeout(() => {
    sceneMetricBatchFlushTimer = null;
    flushScheduledSceneMetricBatches();
  }, delay);
}

function projectScheduledSceneMetricBatchResult(batchData, datasetId, requestedIds = []) {
  const normalizedDatasetId = safeTrim(datasetId);
  if (!normalizedDatasetId) {
    return null;
  }
  const groups = Array.isArray(batchData?.groups) ? batchData.groups : [];
  const group = groups.find(
    (candidate) => safeTrim(candidate?.dataset_id) === normalizedDatasetId
  );
  if (!group) {
    return null;
  }
  const metrics = filterMetricsForRequestedIds(group.metrics, requestedIds);
  const perf = mergeServerAndClientPerf(group?.perf, {});
  perf.client_scene_batch_schedule_hit = 1;
  perf.client_scene_batch_schedule_group_count = groups.length;
  return {
    scene_id: safeTrim(batchData?.scene_id),
    scene_path: safeTrim(batchData?.scene_path) || undefined,
    dataset_id: normalizedDatasetId,
    total_rows: Number(group?.total_rows) || 0,
    metrics,
    perf,
  };
}

function scheduleAfterStablePaint(fn, options = {}) {
  if (typeof window === "undefined") {
    const timer = setTimeout(() => fn(), 0);
    return () => clearTimeout(timer);
  }
  if (typeof window.requestAnimationFrame !== "function") {
    const timer = window.setTimeout(() => fn(), 0);
    return () => window.clearTimeout(timer);
  }
  const aggressive = options?.aggressive === true;
  let first = 0;
  let second = 0;
  first = window.requestAnimationFrame(() => {
    if (aggressive) {
      fn();
      return;
    }
    second = window.requestAnimationFrame(() => {
      fn();
    });
  });
  return () => {
    if (first) window.cancelAnimationFrame(first);
    if (second) window.cancelAnimationFrame(second);
  };
}

function createManagedAbortController(signals = []) {
  const controller = new AbortController();
  ACTIVE_RUNTIME_FETCH_CONTROLLERS.add(controller);
  const cleanups = [];
  const cleanup = () => {
    ACTIVE_RUNTIME_FETCH_CONTROLLERS.delete(controller);
    while (cleanups.length > 0) {
      try {
        cleanups.pop()();
      } catch (_) {
        /* ignore */
      }
    }
  };
  controller.signal.addEventListener("abort", cleanup, { once: true });
  controller.__meiRelease = cleanup;
  for (const signal of Array.isArray(signals) ? signals : []) {
    if (!signal || typeof signal.addEventListener !== "function") continue;
    if (signal.aborted) {
      controller.abort(sharedAbortError());
      break;
    }
    const onAbort = () => controller.abort(sharedAbortError());
    signal.addEventListener("abort", onAbort, { once: true });
    cleanups.push(() => signal.removeEventListener("abort", onAbort));
  }
  return controller;
}

function abortPendingPanelMetricBatches() {
  for (const batch of PANEL_METRIC_BATCHES.values()) {
    if (typeof batch?.cancelFlush === "function") {
      try {
        batch.cancelFlush();
      } catch (_) {
        /* ignore */
      }
    }
    try {
      batch?.reject?.(sharedAbortError());
    } catch (_) {
      /* ignore */
    }
    if (batch?.requesters instanceof Set) {
      batch.requesters.clear();
    }
  }
  PANEL_METRIC_BATCHES.clear();
}

function abortPendingSceneMetricBatchSchedules() {
  if (sceneMetricBatchFlushTimer != null) {
    clearTimeout(sceneMetricBatchFlushTimer);
    sceneMetricBatchFlushTimer = null;
  }
  for (const schedule of SCENE_METRIC_BATCH_SCHEDULES.values()) {
    if (typeof schedule?.cancelFlush === "function") {
      try {
        schedule.cancelFlush();
      } catch (_) {
        /* ignore */
      }
    }
    const requests = Array.isArray(schedule?.requests) ? schedule.requests : [];
    requests.forEach((request) => {
      try {
        request?.reject?.(sharedAbortError());
      } catch (_) {
        /* ignore */
      }
    });
    schedule.requests = [];
  }
  SCENE_METRIC_BATCH_SCHEDULES.clear();
}

function resolveAbortClearCaches(reason, options = {}) {
  const explicit = options?.clearCaches;
  if (explicit === true || explicit === false) {
    return explicit;
  }
  const normalized = String(reason || "").trim();
  if (normalized === "spa_navigation") {
    return false;
  }
  if (normalized.startsWith("manage_tab:")) {
    return false;
  }
  if (normalized === "pagehide" || normalized === "full_navigation_bypass") {
    return true;
  }
  return true;
}

export function abortRuntimeQueries(reason = "", options = {}) {
  abortPendingPanelMetricBatches();
  abortPendingSceneMetricBatchSchedules();
  for (const controller of [...ACTIVE_RUNTIME_FETCH_CONTROLLERS]) {
    try {
      controller.abort(sharedAbortError());
    } catch (_) {
      /* ignore */
    }
  }
  if (resolveAbortClearCaches(reason, options)) {
    if (String(reason || "").trim() === "pagehide") {
      persistRuntimeQueryMemoryCachesToSession();
    }
    clearRuntimeQueryCaches();
  }
  if (typeof window !== "undefined") {
    window.__meiLastRuntimeAbortReason = String(reason || "").trim();
  }
}

const EXPLICIT_METRIC_PROP_KEYS = [
  "content",
  "value",
  "data",
  "metric",
  "totalMetric",
  "total_metric",
  "numerMetric",
  "numer_metric",
  "noViolMetric",
  "no_viol_metric",
];

function collectRuntimeMetricRefsFromProps(props) {
  const refs = [];
  const seen = new Set();
  for (const key of EXPLICIT_METRIC_PROP_KEYS) {
    const candidate = props?.[key];
    const ref = candidate?.__mei_runtime_ref;
    if (!ref || ref.kind !== "metric" || !ref.dataset_id || !ref.metric_id) {
      continue;
    }
    const metricId = String(ref.metric_id).trim();
    if (!metricId || seen.has(metricId)) {
      continue;
    }
    seen.add(metricId);
    refs.push(ref);
  }
  return refs;
}

function normalizeExplicitMetricIds(metricIds) {
  if (!Array.isArray(metricIds)) {
    return [];
  }
  return [
    ...new Set(
      metricIds.map((value) => String(value || "").trim()).filter(Boolean),
    ),
  ].sort();
}

export function collectPanelRuntimeMetricIdsFromPanel(panel, anchorProps, queryStateId = "") {
  const anchorRefs = collectRuntimeMetricRefsFromProps(anchorProps);
  const runtimeRef = anchorRefs[0] || resolveRuntimeMetricRef(anchorProps);
  const metricId = safeTrim(runtimeRef?.metric_id);
  if (!runtimeRef?.dataset_id) {
    return anchorRefs.length
      ? [...new Set(anchorRefs.map((ref) => String(ref.metric_id).trim()).filter(Boolean))].sort()
      : metricId
        ? [metricId]
        : [];
  }
  if (!(panel instanceof Element)) {
    return anchorRefs.length
      ? [...new Set(anchorRefs.map((ref) => String(ref.metric_id).trim()).filter(Boolean))].sort()
      : metricId
        ? [metricId]
        : [];
  }
  const currentQueryStateId = String(queryStateId || queryStateIdOf(anchorProps) || "").trim();
  const currentCoords = sceneQueryCoords(anchorProps, runtimeRef);
  const ids = new Set();
  for (const ref of anchorRefs) {
    const id = String(ref?.metric_id || "").trim();
    if (id) {
      ids.add(id);
    }
  }
  if (!ids.size && metricId) {
    ids.add(metricId);
  }
  panel.querySelectorAll("[data-props]").forEach((node) => {
    const candidateProps = parseProps(node);
    if (!devEvalAllowsRuntimeQuery(candidateProps, node)) {
      return;
    }
    for (const candidateRef of collectRuntimeMetricRefsFromProps(candidateProps)) {
      if (!candidateRef?.dataset_id || !candidateRef?.metric_id) continue;
      if (safeTrim(candidateRef.dataset_id) !== safeTrim(runtimeRef.dataset_id)) continue;
      const candidateQueryStateId = String(queryStateIdOf(candidateProps) || "").trim();
      if (currentQueryStateId && candidateQueryStateId && candidateQueryStateId !== currentQueryStateId) {
        continue;
      }
      const candidateCoords = sceneQueryCoords(candidateProps, candidateRef);
      if (!sameSceneQueryCoords(currentCoords, candidateCoords)) continue;
      ids.add(String(candidateRef.metric_id).trim());
    }
  });
  return [...ids].sort();
}

export function collectPanelRuntimeMetricIds(element, props, queryStateId = "") {
  const runtimeRef = resolveRuntimeMetricRef(props);
  const metricId = safeTrim(runtimeRef?.metric_id);
  if (!runtimeRef?.dataset_id) {
    return metricId ? [metricId] : [];
  }
  const panel = element?.closest?.("[data-mei-panel-id]");
  if (!(panel instanceof Element)) {
    return metricId ? [metricId] : [];
  }
  return collectPanelRuntimeMetricIdsFromPanel(panel, props, queryStateId);
}

function resolveMetricBatchPanel(element, props, queryStateId = "") {
  if (!(element instanceof Element)) {
    return null;
  }
  const panels = [];
  let node = element.closest?.("[data-mei-panel-id]") || null;
  while (node instanceof Element) {
    panels.push(node);
    node = node.parentElement?.closest?.("[data-mei-panel-id]") || null;
  }
  if (!panels.length) {
    return null;
  }
  for (const panel of panels) {
    const metricIds = collectPanelRuntimeMetricIdsFromPanel(panel, props, queryStateId);
    if (metricIds.length > 1) {
      return panel;
    }
  }
  return panels[0];
}

function schedulePanelMetricBatch(panel, element, props, options = {}) {
  if (!devEvalAllowsRuntimeQuery(props, element)) {
    return Promise.resolve(null);
  }
  const effectiveQueryStateId = String(options.queryStateId || queryStateIdOf(props) || "").trim();
  const batchKey = panelMetricBatchKey(
    panel,
    props,
    effectiveQueryStateId,
    options.filters,
    options.search
  );
  if (!batchKey) {
    const metricIds = collectPanelRuntimeMetricIdsFromPanel(panel, props, effectiveQueryStateId);
    return fetchRuntimeMetrics(props, { ...options, queryStateId: effectiveQueryStateId, metricIds });
  }

  let batch = PANEL_METRIC_BATCHES.get(batchKey);
  if (!batch) {
    batch = {
      panel,
      props,
      options: { ...options, queryStateId: effectiveQueryStateId },
      requesters: new Set(),
      promise: null,
      cancelFlush: null,
      flush: null,
    };
    PANEL_METRIC_BATCHES.set(batchKey, batch);
    batch.promise = attachAbortRejectionGuard(
      new Promise((resolve, reject) => {
        batch.resolve = resolve;
        batch.reject = reject;
      }),
    );
  } else {
    batch.panel = panel;
    batch.props = props;
    batch.options = { ...options, queryStateId: effectiveQueryStateId };
  }
  if (element instanceof Element) {
    batch.requesters.add(element);
  }

  if (typeof batch.flush !== "function") {
    batch.flush = async () => {
      batch.cancelFlush = null;
      const active = PANEL_METRIC_BATCHES.get(batchKey);
      if (!active || active !== batch) {
        return;
      }
      try {
        const metricIds = collectPanelRuntimeMetricIdsFromPanel(
          batch.panel,
          batch.props,
          batch.options.queryStateId,
        );
        const requesterLabels = [...batch.requesters]
          .map((requester) => {
            const tag = String(requester?.tagName || "").toLowerCase();
            const panelId = String(requester?.closest?.("[data-mei-panel-id]")?.getAttribute?.("data-mei-panel-id") || "");
            const role = String(requester?.getAttribute?.("data-component") || "");
            return [tag, panelId, role].filter(Boolean).join(":");
          })
          .filter(Boolean)
          .slice(0, 8);
        const data = await fetchRuntimeMetrics(batch.props, {
          ...batch.options,
          metricIds,
          meta: {
            ...(batch.options.meta || {}),
            panel_batch_consumer_count: batch.requesters.size,
            panel_batch_consumers: requesterLabels.join(","),
          },
        });
        batch.resolve?.(data);
      } catch (error) {
        batch.reject?.(error);
      } finally {
        batch.requesters.clear();
        PANEL_METRIC_BATCHES.delete(batchKey);
      }
    };
  }

  if (typeof batch.cancelFlush === "function") {
    batch.cancelFlush();
  }
  const aggressivePrefetch =
    options.prefetchEager === true || isPrefetchMetricRequest(options.meta);
  batch.cancelFlush = scheduleAfterStablePaint(() => {
    attachAbortRejectionGuard(batch.flush());
  }, { aggressive: true });

  return batch.promise;
}

export function prefetchPanelRuntimeMetrics(panel, anchor, props, options = {}) {
  if (!(panel instanceof Element) || !(anchor instanceof Element)) {
    return Promise.resolve(null);
  }
  const overlayOpen = isDrilldownOverlayOpen();
  const inOverlay = isRuntimeDrilldownOverlayElement(panel);
  if (overlayOpen && !inOverlay) {
    return Promise.resolve(null);
  }
  if (!elementIsDisplayed(panel)) {
    return Promise.resolve(null);
  }
  if (!devEvalAllowsRuntimeQuery(props, anchor)) {
    return Promise.resolve(null);
  }
  return schedulePanelMetricBatch(panel, anchor, props, {
    ...options,
    prefetchEager: true,
  });
}

/**
 * 按 viewport 内同一 scene + dataset + query_state 合并指标 id，减少首页多块指标的串行请求。
 */
export function prefetchViewportRuntimeMetrics(root = document) {
  if (typeof document === "undefined") {
    return;
  }
  const scopeRoot = root && root.querySelectorAll ? root : document;
  const targetingOverlay =
    scopeRoot !== document && isRuntimeDrilldownOverlayElement(scopeRoot);
  if (isDrilldownOverlayOpen() && !targetingOverlay && scopeRoot === document) {
    return;
  }
  const viewportRoots =
    scopeRoot === document
      ? [...document.querySelectorAll('[data-mei-frame-viewport="true"]')]
      : [scopeRoot];
  const groups = new Map();
  for (const viewport of viewportRoots) {
    if (!(viewport instanceof Element) || !elementIsDisplayed(viewport)) {
      continue;
    }
    viewport.querySelectorAll("[data-props]").forEach((node) => {
      if (!(node instanceof Element) || !node.isConnected || isBuildPreviewScopedDim(node)) {
        return;
      }
      const props = parseProps(node);
      if (!devEvalAllowsRuntimeQuery(props, node)) {
        return;
      }
      const runtimeRefs = collectRuntimeMetricRefsFromProps(props);
      if (!runtimeRefs.length) {
        const runtimeRef = resolveRuntimeMetricRef(props);
        if (runtimeRef?.dataset_id && runtimeRef?.metric_id) {
          runtimeRefs.push(runtimeRef);
        }
      }
      if (!runtimeRefs.length) {
        return;
      }
      const capability = metricQueryCapabilityConfig(props);
      if (!capability.enabled) {
        return;
      }
      const effectiveQueryStateId = String(queryStateIdOf(props) || "").trim();
      const groupKey = sceneMetricBatchScopeKey(props, effectiveQueryStateId, {}, "");
      let group = groups.get(groupKey);
      if (!group) {
        group = { props, queryStateId: effectiveQueryStateId, entries: new Map() };
        groups.set(groupKey, group);
      }
      for (const runtimeRef of runtimeRefs) {
        const datasetId = safeTrim(runtimeRef.dataset_id);
        if (!datasetId) {
          continue;
        }
        let entry = group.entries.get(datasetId);
        if (!entry) {
          entry = { datasetId, metricIds: new Set(), props };
          group.entries.set(datasetId, entry);
        }
        entry.metricIds.add(String(runtimeRef.metric_id).trim());
      }
    });
  }
  for (const group of groups.values()) {
    const capability = metricBatchQueryCapabilityConfig(group.props);
    if (!capability.enabled) {
      continue;
    }
    const api = capability.api;
    const batchGroups = [...group.entries.values()]
      .map((entry) => ({
        datasetId: entry.datasetId,
        metricIds: [...entry.metricIds].sort(),
      }))
      .filter((entry) => entry.metricIds.length > 0);
    if (!batchGroups.length) {
      continue;
    }
    for (const entry of batchGroups) {
      const entryProps =
        group.entries.get(entry.datasetId)?.props || group.props;
      void scheduleSceneRuntimeMetricRequest(api, entryProps, entry.metricIds, {
        queryStateId: group.queryStateId,
        datasetId: entry.datasetId,
        meta: { component: "prefetch_viewport" },
      });
    }
  }
  scheduleSceneMetricBatchFlush();
}

export function prefetchVisiblePanelMetrics(root = document) {
  if (typeof document === "undefined") {
    return;
  }
  const overlayRoots = drilldownOverlayViewportRoots();
  if (overlayRoots.length > 0 && root === document) {
    overlayRoots.forEach((overlayRoot) => prefetchVisiblePanelMetrics(overlayRoot));
    return;
  }
  const scopeRoot = root && root.querySelectorAll ? root : document;
  const viewportSelector = '[data-mei-frame-viewport="true"]';
  const hasFrameViewport =
    scopeRoot === document
      ? Boolean(document.querySelector(viewportSelector))
      : scopeRoot.matches?.(viewportSelector) ||
        Boolean(scopeRoot.querySelector?.(viewportSelector));
  if (hasFrameViewport) {
    if (scopeRoot === document) {
      document.querySelectorAll(viewportSelector).forEach((viewport) => {
        prefetchViewportRuntimeMetrics(viewport);
      });
    } else {
      prefetchViewportRuntimeMetrics(scopeRoot);
    }
    return;
  }
  const seen = new Set();
  scopeRoot.querySelectorAll("[data-mei-panel-id]").forEach((panel) => {
    if (
      !(panel instanceof Element) ||
      !elementIsDisplayed(panel) ||
      isBuildPreviewScopedDim(panel)
    ) {
      return;
    }
    const panelId = String(panel.getAttribute("data-mei-panel-id") || "").trim();
    if (!panelId || seen.has(panelId)) {
      return;
    }
    const anchor = panel.querySelector("[data-props]");
    if (!(anchor instanceof Element)) {
      return;
    }
    const props = parseProps(anchor);
    if (!resolveRuntimeMetricRef(props)?.dataset_id) {
      return;
    }
    seen.add(panelId);
    void prefetchPanelRuntimeMetrics(panel, anchor, props, {
      meta: { component: "prefetch", panel_id: panelId },
    });
  });
}

function readCompileEpochFromPage() {
  if (typeof document === "undefined") {
    return "";
  }
  const shell = document.querySelector(".shell[data-compile-epoch]");
  if (shell) {
    return String(shell.getAttribute("data-compile-epoch") || "").trim();
  }
  return "";
}

function handlePreviewUpdatedRuntimeQueryCache(event) {
  const scope = previewUpdatedScope(event);
  if (scope === "page") {
    scheduleSceneMetricBatchFlush(0);
  }
  if (scope === "drilldown") {
    return;
  }
  const detail = event?.detail && typeof event.detail === "object" ? event.detail : {};
  if (detail.resetRuntimeQueryCache === true) {
    clearRuntimeQueryCaches({ clearSession: true });
    return;
  }
  if (detail.resetRuntimeQueryCache === false) {
    return;
  }
  const epoch = String(detail.compileEpoch || readCompileEpochFromPage() || "").trim();
  const dataGen = String(detail.dataGeneration || detail.data_generation || "").trim();
  if (epoch || dataGen) {
    maybeInvalidateRuntimeQueryCachesForCompileEpoch(epoch, dataGen);
  }
}

function readHostMetricQueryApi() {
  try {
    const el = document.getElementById("mei-host-runtime-capabilities");
    if (el) {
      const parsed = JSON.parse(el.textContent || "{}");
      return safeTrim(parsed?.metric_query?.api || parsed?.metric_batch_query?.api);
    }
  } catch (_) {
    /* ignore */
  }
  const caps = window.__meiHostRuntimeCapabilities;
  if (caps && typeof caps === "object") {
    return safeTrim(caps.metric_query?.api || caps.metric_batch_query?.api);
  }
  return "";
}

const BOOTSTRAP_DATASET_PAGE_SIZES = [0, 3, 5, 10, 16, 20, 25, 30, 50, 64, 100];

function bootstrapDatasetPageSizesForMetric(contract) {
  const sizes = new Set(BOOTSTRAP_DATASET_PAGE_SIZES);
  const fromContract = Number(contract?.page_size ?? contract?.pageSize ?? 0);
  if (Number.isFinite(fromContract) && fromContract > 0) {
    sizes.add(Math.round(fromContract));
  }
  return [...sizes].sort((left, right) => left - right);
}

function normalizeDatasetQueryCachePayload(payload = {}) {
  const normalized = { ...(payload || {}) };
  if (!normalized.full) {
    delete normalized.full;
  }
  if (!normalized.summary) {
    delete normalized.summary;
  }
  if (normalizePositiveInt(normalized.page, 1, { min: 1 }) === 1 && normalized.full === true) {
    delete normalized.full;
  }
  if (!Array.isArray(normalized.sort) || normalized.sort.length === 0) {
    delete normalized.sort;
  }
  delete normalized.column_state;
  delete normalized.preview_scope;
  if (!safeTrim(normalized.target)) {
    delete normalized.target;
  }
  if (!safeTrim(normalized.metric_id)) {
    delete normalized.metric_id;
  }
  if (!safeTrim(normalized.search)) {
    delete normalized.search;
    if (normalized.query_state && typeof normalized.query_state === "object") {
      const nextQueryState = { ...normalized.query_state };
      if (!safeTrim(nextQueryState.search)) {
        delete nextQueryState.search;
      }
      normalized.query_state =
        Object.keys(nextQueryState).length > 0 ? nextQueryState : { filters: normalized.filters || {} };
    }
  }
  if (!normalized.filter_intents || normalized.filter_intents.length === 0) {
    delete normalized.filter_intents;
  }
  if (normalized.query_state && typeof normalized.query_state === "object") {
    const queryState = { ...normalized.query_state };
    if (!Array.isArray(queryState.group) || queryState.group.length === 0) {
      delete queryState.group;
    }
    if (!queryState.time_range) {
      delete queryState.time_range;
    }
    if (!safeTrim(queryState.search)) {
      delete queryState.search;
    }
    if (!queryState.filters || typeof queryState.filters !== "object") {
      queryState.filters = {};
    }
    normalized.query_state = queryState;
  }
  return normalized;
}

function datasetQueryPayloadVariants(payload = {}) {
  const base = normalizeDatasetQueryCachePayload(payload);
  const variants = [base];
  const withMetric = { ...base };
  const metricId = safeTrim(payload?.metric_id);
  if (metricId) {
    withMetric.metric_id = metricId;
    variants.push(withMetric);
  }
  const withoutMetric = { ...base };
  delete withoutMetric.metric_id;
  variants.push(withoutMetric);
  const target = safeTrim(payload?.target);
  if (target) {
    const withoutTarget = { ...base };
    delete withoutTarget.target;
    variants.push(withoutTarget);
    if (metricId) {
      variants.push({ ...withoutTarget, metric_id: metricId });
    }
  }
  const seen = new Set();
  return variants.filter((candidate) => {
    const key = stableSerialize(candidate);
    if (seen.has(key)) {
      return false;
    }
    seen.add(key);
    return true;
  });
}

function bootstrapScopeSceneIds() {
  if (typeof window === "undefined") {
    return [];
  }
  const fallbackPageCtx = readBootstrapSeedPageContext(window.__mei);
  return bootstrapScopeEntries(window.__mei)
    .map((scope) => bootstrapScopeName(scope, fallbackPageCtx))
    .filter(Boolean);
}

function datasetQueryPayloadVariantsForBootstrapLookup(payload = {}) {
  const requestScene = safeTrim(payload?.scene_id);
  const sceneIds = new Set(bootstrapScopeSceneIds());
  if (requestScene) {
    sceneIds.add(requestScene);
  }
  const variants = [];
  const seen = new Set();
  const pushVariants = (candidate) => {
    for (const variant of datasetQueryPayloadVariants(candidate)) {
      const key = stableSerialize(normalizeDatasetQueryCachePayload(variant));
      if (seen.has(key)) {
        continue;
      }
      seen.add(key);
      variants.push(variant);
    }
  };
  for (const sceneId of sceneIds) {
    pushVariants({ ...payload, scene_id: sceneId });
  }
  if (!requestScene) {
    pushVariants(payload);
  }
  return variants;
}

function bootstrapDatasetLookupPageSizes(payload = {}) {
  return [
    ...new Set([
      ...bootstrapDatasetPageSizesForMetric({
        page_size: payload?.page_size,
        pageSize: payload?.page_size,
      }),
      ...BOOTSTRAP_DATASET_PAGE_SIZES,
    ]),
  ].sort((left, right) => left - right);
}

function resolveBootstrapDatasetCacheEntry(api, payload, fingerprint, now = Date.now(), props = null) {
  const datasetId = safeTrim(payload?.dataset_id);
  const page = normalizePositiveInt(payload?.page, 1, { min: 1 });
  if (!datasetId || page !== 1) {
    return null;
  }
  const fingerprints = props
    ? datasetQueryFingerprintCandidates(props, fingerprint)
    : [String(fingerprint || "").trim()].filter(Boolean);
  const pageSizes = bootstrapDatasetLookupPageSizes(payload);
  for (const fp of fingerprints) {
    for (const pageSize of pageSizes) {
      for (const variant of datasetQueryPayloadVariantsForBootstrapLookup({
        ...payload,
        page: 1,
        page_size: pageSize,
      })) {
        const cacheKey = datasetQueryCacheKey(api, variant, fp);
        const cached = cacheStore().datasetResults.get(cacheKey);
        if (cached && cached.expiresAt > now) {
          return { cacheKey, cached, variant, fingerprint: fp };
        }
      }
    }
  }
  return null;
}

function rememberDatasetQueryCacheEntry(api, payload, fingerprint, data, expiresAt) {
  const cacheKey = datasetQueryCacheKey(api, payload, fingerprint);
  cacheStore().datasetResults.set(cacheKey, { data, expiresAt });
  return cacheKey;
}

if (typeof window !== "undefined") {
  window.__meiEvalStoreReaders = {
    readMetric(api, payload, fingerprint) {
      const cacheKey = metricQueryCacheKey(api, payload, fingerprint);
      return cacheStore().metricResults.get(cacheKey)?.data || null;
    },
    readDataset(api, payload, fingerprint) {
      return resolveBootstrapDatasetCacheEntry(api, payload, fingerprint)?.cached?.data || null;
    },
    metricCacheKey(api, payload, fingerprint) {
      return metricQueryCacheKey(api, payload, fingerprint);
    },
    datasetCacheKey(api, payload, fingerprint) {
      return datasetQueryCacheKey(api, payload, fingerprint);
    },
    datasetCacheSize() {
      return cacheStore().datasetResults.size;
    },
    sampleDatasetCacheKeys(limit = 5) {
      return [...cacheStore().datasetResults.keys()].slice(0, limit);
    },
  };
}

function readBootstrapSeedPageContext(bootstrap = window.__mei) {
  const shell = document.querySelector(".shell[data-compile-target]");
  let hostMeta = {};
  const anchor = document.querySelector("[data-props]");
  if (anchor) {
    try {
      const props = JSON.parse(anchor.getAttribute("data-props") || "{}");
      if (props?._mei && typeof props._mei === "object") {
        hostMeta = props._mei;
      }
    } catch (_) {
      /* ignore */
    }
  }
  const boot = bootstrap && typeof bootstrap === "object" ? bootstrap : {};
  return {
    scene_id: safeTrim(
      boot.bootstrap_scope ||
        boot.bootstrapScope ||
        shell?.getAttribute("data-compile-scene") ||
        hostMeta.active_scene_id ||
        "home",
    ),
    target: safeTrim(
      boot.bootstrap_target_file ||
        boot.targetFile ||
        shell?.getAttribute("data-compile-target") ||
        hostMeta.active_target_file ||
        hostMeta.entry_target ||
        "",
    ),
    compile_epoch: safeTrim(
      boot.bootstrap_compile_epoch ||
        boot.compileEpoch ||
        hostMeta.compile_epoch ||
        readCompileEpochFromPage(),
    ),
    data_generation: safeTrim(
      boot.bootstrap_data_generation ||
        boot.dataGeneration ||
        hostMeta.data_generation ||
        readShellRuntimeDataGeneration() ||
        "",
    ),
    app_id: safeTrim(
      readShellRuntimeAppId() ||
        readRouteRuntimeAppId() ||
        boot.bootstrap_app_id ||
        boot.appId ||
        hostMeta.app_id ||
        "",
    ),
  };
}

function bootstrapScopeEntries(bootstrap = window.__mei) {
  const boot = bootstrap && typeof bootstrap === "object" ? bootstrap : {};
  const scopes = Array.isArray(boot.bootstrap_scopes)
    ? boot.bootstrap_scopes
    : Array.isArray(boot.bootstrapScopes)
      ? boot.bootstrapScopes
      : [];
  const normalized = scopes.filter((entry) => entry && typeof entry === "object");
  if (normalized.length > 0) {
    return normalized;
  }
  return [boot];
}

function bootstrapScopeName(scopeBootstrap, fallbackPageCtx) {
  return safeTrim(
    scopeBootstrap?.bootstrap_scope || scopeBootstrap?.bootstrapScope || fallbackPageCtx?.scene_id || "home",
  );
}

function bootstrapScopeRevision(scopeBootstrap, bootstrap = window.__mei) {
  return safeTrim(
    scopeBootstrap?.client_revision ||
      scopeBootstrap?.clientRevision ||
      bootstrap?.client_revision ||
      bootstrap?.clientRevision ||
      "",
  );
}

function bootstrapQueryFingerprint(pageCtx) {
  return `${pageCtx.compile_epoch}|${pageCtx.data_generation}`;
}

function bootstrapSceneCoords(pageCtx, datasetId) {
  const coords = {};
  if (pageCtx.scene_id) {
    coords.scene_id = pageCtx.scene_id;
  }
  const importedTarget = importedWorldMetricsCompileTarget(datasetId);
  if (importedTarget) {
    coords.target = importedTarget;
  } else if (pageCtx.target) {
    coords.target = pageCtx.target;
  }
  return coords;
}

function buildBootstrapQueryStatePayload() {
  return mergedQueryStatePayload("", {});
}

function buildBootstrapMetricQueryPayload(pageCtx, datasetId, metricIds, queryStatePayload) {
  const coords = bootstrapSceneCoords(pageCtx, datasetId);
  return {
    ...coords,
    dataset_id: datasetId,
    metric_ids: [...metricIds].sort(),
    search: queryStatePayload.search || undefined,
    filters: queryStatePayload.filters,
    query_state: {
      filters: queryStatePayload.filters,
      ...(safeTrim(queryStatePayload.search) ? { search: queryStatePayload.search } : {}),
      ...(queryStatePayload.group.length > 0 ? { group: queryStatePayload.group } : {}),
      ...(queryStatePayload.timeRange ? { time_range: queryStatePayload.timeRange } : {}),
    },
    filter_intents:
      queryStatePayload.filterIntents.length > 0 ? queryStatePayload.filterIntents : undefined,
  };
}

function inferColumnsFromRows(rows) {
  if (!Array.isArray(rows) || rows.length === 0) {
    return [];
  }
  return Object.keys(rows[0] || {}).filter(Boolean);
}

function extractBootstrapDatasetRows(contract) {
  if (!contract || typeof contract !== "object") {
    return null;
  }
  const value = contract.value;
  if (Array.isArray(value)) {
    return value;
  }
  if (value && typeof value === "object" && Array.isArray(value.rows)) {
    return value.rows;
  }
  if (Array.isArray(contract.rows)) {
    return contract.rows;
  }
  return null;
}

function buildBootstrapDatasetRowsData(contract, pageCtx, entryMeta = null) {
  const rows = extractBootstrapDatasetRows(contract);
  if (!Array.isArray(rows)) {
    return null;
  }
  const schemaCols = Array.isArray(contract?.schema)
    ? contract.schema
        .map((col) => safeTrim(col?.id || col?.name || col?.field))
        .filter(Boolean)
    : [];
  const columns = schemaCols.length > 0 ? schemaCols : inferColumnsFromRows(rows);
  const total =
    Number(entryMeta?.total_rows ?? contract?.total_rows ?? rows.length) || rows.length;
  return {
    scene_id: pageCtx.scene_id,
    rows,
    columns,
    total,
    page: 1,
    has_more: total > rows.length,
    perf: { bootstrap: 1 },
  };
}

function buildBootstrapDatasetQueryPayload(
  pageCtx,
  datasetId,
  metricId,
  queryStatePayload,
  pageSize,
) {
  const coords = bootstrapSceneCoords(pageCtx, datasetId);
  return {
    ...coords,
    dataset_id: datasetId,
    metric_id: metricId,
    page: 1,
    page_size: pageSize,
    search: queryStatePayload.search || undefined,
    filters: queryStatePayload.filters,
    query_state: {
      filters: queryStatePayload.filters,
      ...(safeTrim(queryStatePayload.search) ? { search: queryStatePayload.search } : {}),
      ...(queryStatePayload.group.length > 0 ? { group: queryStatePayload.group } : {}),
      ...(queryStatePayload.timeRange ? { time_range: queryStatePayload.timeRange } : {}),
    },
    filter_intents:
      queryStatePayload.filterIntents.length > 0 ? queryStatePayload.filterIntents : undefined,
    full: false,
    summary: false,
  };
}

function readBootstrapMetricQueryApi() {
  const caps = readGlobalHostRuntimeCapabilities();
  if (caps && typeof caps === "object") {
    const api = safeTrim(caps.metric_query?.api || caps.metric_batch_query?.api);
    if (api) {
      return api;
    }
  }
  return readHostMetricQueryApi();
}

function readBootstrapRowsQueryApi() {
  const caps = readGlobalHostRuntimeCapabilities();
  if (caps && typeof caps === "object") {
    const api = safeTrim(
      caps.rows_query?.api || caps.dataset_rows_query?.api || caps.rowsQuery?.api,
    );
    if (api) {
      return api;
    }
  }
  try {
    const el = document.getElementById("mei-host-runtime-capabilities");
    if (el) {
      const parsed = JSON.parse(el.textContent || "{}");
      return safeTrim(parsed?.rows_query?.api || parsed?.dataset_rows_query?.api);
    }
  } catch (_) {
    /* ignore */
  }
  return "";
}

export function seedFromBootstrap(bootstrap = window.__mei) {
  if (typeof window === "undefined" || !bootstrap || typeof bootstrap !== "object") {
    return 0;
  }
  delete window.__meiBootstrapSeedError;
  const scopeBootstraps = bootstrapScopeEntries(bootstrap);
  if (!scopeBootstraps.length) {
    window.__meiBootstrapSeedError = "bootstrap_scopes_missing";
    return 0;
  }
  const fallbackPageCtx = readBootstrapSeedPageContext(bootstrap);
  const revisionByScope =
    window.__meiBootstrapRevisionByScope && typeof window.__meiBootstrapRevisionByScope === "object"
      ? window.__meiBootstrapRevisionByScope
      : {};
  let shouldReset = false;
  scopeBootstraps.forEach((scopeBootstrap) => {
    const scope = bootstrapScopeName(scopeBootstrap, fallbackPageCtx);
    const revision = bootstrapScopeRevision(scopeBootstrap, bootstrap);
    if (!scope || !revision) {
      return;
    }
    const prevRevision = String(revisionByScope[scope] || "").trim();
    if (prevRevision && prevRevision !== revision) {
      shouldReset = true;
    }
  });
  if (shouldReset) {
    cacheStore().metricResults.clear();
    cacheStore().metricScopeResults.clear();
    cacheStore().datasetResults.clear();
  }
  const metricApi = readBootstrapMetricQueryApi();
  const rowsApi = readBootstrapRowsQueryApi();
  if (!metricApi) {
    window.__meiBootstrapSeedError = "metric_api_missing";
    return 0;
  }
  let seededCount = 0;
  const cacheConfig = window.__meiClientQueryCacheConfig || clientQueryCacheConfig({});
  const expiresAt = Date.now() + METRIC_QUERY_CACHE_TTL_MS;
  scopeBootstraps.forEach((scopeBootstrap) => {
    const metrics = Array.isArray(scopeBootstrap?.metrics)
      ? scopeBootstrap.metrics
      : Array.isArray(scopeBootstrap?.bootstrap_metrics)
        ? scopeBootstrap.bootstrap_metrics
        : Array.isArray(scopeBootstrap?.bootstrapMetrics)
          ? scopeBootstrap.bootstrapMetrics
          : [];
    if (!metrics.length) {
      return;
    }
    const pageCtx = readBootstrapSeedPageContext({
      ...bootstrap,
      ...scopeBootstrap,
      bootstrap_scope: scopeBootstrap.bootstrap_scope || scopeBootstrap.bootstrapScope,
      bootstrap_target_file: scopeBootstrap.target_file || scopeBootstrap.targetFile,
      bootstrap_compile_epoch: scopeBootstrap.compile_epoch || scopeBootstrap.compileEpoch,
    });
    if (!pageCtx.compile_epoch || !pageCtx.target) {
      window.__meiBootstrapSeedError = "bootstrap_page_context_incomplete";
      return;
    }
    if (pageCtx.app_id) {
      window.__meiRuntimeAppId = pageCtx.app_id;
    }
    if (pageCtx.data_generation) {
      window.__meiRuntimeDataGeneration = pageCtx.data_generation;
    }
    const scope = bootstrapScopeName(scopeBootstrap, pageCtx);
    const revision = bootstrapScopeRevision(scopeBootstrap, bootstrap);
    if (scope && revision) {
      revisionByScope[scope] = revision;
      if (!window.__meiBootstrapRevision) {
        window.__meiBootstrapRevision = revision;
      }
    }
    const queryFingerprint = bootstrapQueryFingerprint(pageCtx);
    const queryStatePayload = buildBootstrapQueryStatePayload();
    const byDataset = new Map();
    for (const entry of metrics) {
      const contract = entry?.contract && typeof entry.contract === "object" ? entry.contract : entry;
      const metricId = safeTrim(contract?.id || entry?.id);
      const datasetId = safeTrim(
        contract?.dataset_id ||
          contract?.owner_dataset_id ||
          contract?.dataset ||
          entry?.dataset_id,
      );
      if (!metricId || !datasetId) {
        continue;
      }
      if (!byDataset.has(datasetId)) {
        byDataset.set(datasetId, []);
      }
      byDataset.get(datasetId).push({ contract, entry });
    }
    for (const [datasetId, datasetMetricEntries] of byDataset.entries()) {
      const datasetMetrics = datasetMetricEntries.map((item) => item.contract);
      const metricIds = datasetMetrics
        .map((metric) => safeTrim(metric?.id))
        .filter(Boolean)
        .sort();
      const payload = buildBootstrapMetricQueryPayload(
        pageCtx,
        datasetId,
        metricIds,
        queryStatePayload,
      );
      const cacheKey = metricQueryCacheKey(metricApi, payload, queryFingerprint);
      const scopeKey = metricQueryScopeCacheKey(metricApi, payload, queryFingerprint);
      const totalRows = datasetMetricEntries.reduce(
        (max, item) => Math.max(max, Number(item.entry?.total_rows) || 0),
        0,
      );
      const data = {
        scene_id: scope,
        dataset_id: datasetId,
        total_rows: totalRows,
        metrics: datasetMetrics,
        perf: { bootstrap: 1 },
      };
      cacheStore().metricResults.set(cacheKey, { data, expiresAt });
      rememberMetricScopeResult(scopeKey, metricIds, data, expiresAt);
      seededCount += 1;
      if (
        pageCtx.app_id &&
        pageCtx.data_generation &&
        String(cacheConfig.persist || "").toLowerCase() === "sessionstorage"
      ) {
        writeSessionRuntimeQueryCache(
          pageCtx.app_id,
          cacheKey,
          pageCtx.data_generation,
          "metric",
          data,
          METRIC_QUERY_CACHE_TTL_MS,
          cacheConfig.maxEntries,
        );
      }
    }
    if (!rowsApi) {
      return;
    }
    for (const entry of metrics) {
      const contract = entry?.contract && typeof entry.contract === "object" ? entry.contract : entry;
      const metricId = safeTrim(contract?.id || entry?.id);
      const datasetId = safeTrim(
        contract?.dataset_id ||
          contract?.owner_dataset_id ||
          contract?.dataset ||
          entry?.dataset_id,
      );
      const shape = safeTrim(contract?.shape).toLowerCase();
      if (!metricId || !datasetId || shape !== "dataframe") {
        continue;
      }
      const rowsData = buildBootstrapDatasetRowsData(contract, pageCtx, entry);
      if (!rowsData) {
        continue;
      }
      for (const pageSize of bootstrapDatasetPageSizesForMetric(contract)) {
        const payload = buildBootstrapDatasetQueryPayload(
          pageCtx,
          datasetId,
          metricId,
          queryStatePayload,
          pageSize,
        );
        const rowsDataWithPage = { ...rowsData, page_size: pageSize };
        const payloadVariants = [
          ...datasetQueryPayloadVariants(payload),
          ...datasetQueryPayloadVariants({ ...payload, full: true }),
        ];
        const seenVariants = new Set();
        for (const variant of payloadVariants) {
          const variantKey = stableSerialize(normalizeDatasetQueryCachePayload(variant));
          if (seenVariants.has(variantKey)) {
            continue;
          }
          seenVariants.add(variantKey);
          const cacheKey = datasetQueryCacheKey(rowsApi, variant, queryFingerprint);
          cacheStore().datasetResults.set(cacheKey, {
            data: rowsDataWithPage,
            expiresAt,
          });
          seededCount += 1;
          if (
            pageCtx.app_id &&
            pageCtx.data_generation &&
            String(cacheConfig.persist || "").toLowerCase() === "sessionstorage"
          ) {
            writeSessionRuntimeQueryCache(
              pageCtx.app_id,
              cacheKey,
              pageCtx.data_generation,
              "dataset",
              rowsDataWithPage,
              METRIC_QUERY_CACHE_TTL_MS,
              cacheConfig.maxEntries,
            );
          }
        }
      }
    }
  });
  window.__meiBootstrapRevisionByScope = revisionByScope;
  rebuildEvalDeliveryClassIndex(bootstrap);
  if (seededCount > 0) {
    notifyClientRuntimeQueryCacheHit("bootstrap");
  }
  return seededCount;
}

function deliveryClassForMetricShape(shape) {
  const normalized = safeTrim(shape).toLowerCase();
  if (normalized === "dataframe") {
    return "dataframe_page1";
  }
  return "metric_scalar";
}

function rebuildEvalDeliveryClassIndex(bootstrap = window.__mei) {
  if (typeof window === "undefined") {
    return;
  }
  const index = {};
  const metrics = Array.isArray(bootstrap?.bootstrap_metrics) ? bootstrap.bootstrap_metrics : [];
  metrics.forEach((entry) => {
    const contract = entry?.contract && typeof entry.contract === "object" ? entry.contract : entry;
    const id = safeTrim(contract?.id || entry?.id);
    if (!id) {
      return;
    }
    const fromMount = safeTrim(entry?.delivery_class || contract?.delivery_class);
    index[id] = fromMount || deliveryClassForMetricShape(contract?.shape || entry?.shape);
  });
  const seed = bootstrap?.bootstrap_seed;
  const mounts = Array.isArray(seed?.mounts) ? seed.mounts : [];
  mounts.forEach((mount) => {
    const id = safeTrim(mount?.metric_id);
    const deliveryClass = safeTrim(mount?.delivery_class);
    if (id && deliveryClass) {
      index[id] = deliveryClass;
    }
  });
  window.__meiEvalDeliveryClassByMetric = index;
}

function metricDeliveryClass(metricId) {
  const map = typeof window !== "undefined" ? window.__meiEvalDeliveryClassByMetric : null;
  if (!map || typeof map !== "object") {
    return "";
  }
  return safeTrim(map[safeTrim(metricId)]);
}

function deliveryClassAllowsIndependentFetch(deliveryClass, { page = 1 } = {}) {
  const cls = safeTrim(deliveryClass);
  const normalizedPage = normalizePositiveInt(page, 1, { min: 1 });
  if (!cls) {
    return normalizedPage > 1;
  }
  if (cls === "dataframe_page_n" || cls === "dataframe_page1") {
    return normalizedPage > 1;
  }
  return ["media_blob", "map_tile", "mesh_asset"].includes(cls);
}

function packFirstAppliesToDatasetFetch(props, { metricId = "", page = 1, ...options } = {}) {
  const deliveryClass = metricDeliveryClass(metricId);
  if (deliveryClassAllowsIndependentFetch(deliveryClass, { page })) {
    return false;
  }
  return shouldEnforcePackFirst(props, { ...options, page });
}

function readEvalStoreMetricData(api, payload, fingerprint) {
  if (typeof window === "undefined") {
    return null;
  }
  const boot = window.__meiLangBoot || {};
  if (typeof boot.evalStore?.getMetric !== "function") {
    return null;
  }
  return boot.evalStore.getMetric(api, payload, fingerprint);
}

function readEvalStoreDatasetData(api, payload, fingerprint) {
  if (typeof window === "undefined") {
    return null;
  }
  const boot = window.__meiLangBoot || {};
  if (typeof boot.evalStore?.getDatasetPage1 !== "function") {
    return null;
  }
  return boot.evalStore.getDatasetPage1(api, payload, fingerprint);
}

export function clearEvalRuntimeCaches() {
  if (typeof window !== "undefined" && window.__meiLangBoot?.evalStoreCache?.clearAll) {
    window.__meiLangBoot.evalStoreCache.clearAll();
  } else {
    cacheStore().metricInflight.clear();
    cacheStore().metricResults.clear();
    cacheStore().metricScopeInflight.clear();
    cacheStore().metricScopeResults.clear();
    cacheStore().datasetInflight.clear();
    cacheStore().datasetResults.clear();
  }
  if (typeof window !== "undefined") {
    window.__meiBootstrapSeeded = false;
    window.__meiBootstrapSeedCount = 0;
    delete window.__meiEvalDeliveryClassByMetric;
  }
}

if (typeof window !== "undefined") {
  window.clearEvalRuntimeCaches = clearEvalRuntimeCaches;
  const boot = (window.__meiLangBoot = window.__meiLangBoot || {});
  boot.setQueryState = setQueryState;
  boot.getQueryState = getQueryState;
  boot.metricDeliveryClass = metricDeliveryClass;
  boot.deliveryClassAllowsIndependentFetch = deliveryClassAllowsIndependentFetch;
}

function readRouteRuntimeAppId() {
  if (typeof window === "undefined") {
    return "";
  }
  try {
    const match = String(window.location.pathname || "").match(/^\/apps\/([^/]+)\//);
    return match ? safeTrim(match[1]) : "";
  } catch (_) {
    return "";
  }
}

function readShellRuntimeAppId() {
  if (typeof document === "undefined") {
    return "";
  }
  const shell =
    document.querySelector(".shell[data-app-path]") ||
    document.querySelector("[data-runtime-node][data-app-path]") ||
    document.querySelector("[data-app]");
  if (!shell) {
    return "";
  }
  return safeTrim(
    shell.getAttribute("data-app-path") ||
      shell.getAttribute("data-app") ||
      shell.dataset?.appPath ||
      shell.dataset?.app ||
      "",
  );
}

function readShellRuntimeDataGeneration() {
  if (typeof document === "undefined") {
    return "";
  }
  const shell = document.querySelector(
    ".shell[data-data-generation], .shell[data-compile-epoch], [data-runtime-node][data-data-generation]",
  );
  if (!shell) {
    return "";
  }
  return safeTrim(
    shell.getAttribute("data-data-generation") ||
      shell.getAttribute("data-compile-epoch") ||
      "",
  );
}

export function syncRuntimeQueryAppContextFromPage(options = {}) {
  if (typeof window === "undefined") {
    return "";
  }
  const opts = options && typeof options === "object" ? options : {};
  const prevAppId = safeTrim(window.__meiRuntimeAppId);
  const nextAppId = readRuntimeQueryAppId();
  const pageCtx = readBootstrapSeedPageContext();
  const nextDataGen =
    readShellRuntimeDataGeneration() ||
    safeTrim(pageCtx.data_generation) ||
    safeTrim(window.__meiRuntimeDataGeneration);
  if (nextDataGen) {
    window.__meiRuntimeDataGeneration = nextDataGen;
  }
  const appChanged = Boolean(prevAppId && nextAppId && prevAppId !== nextAppId);
  if (opts.clearCaches === true || appChanged) {
    abortPendingPanelMetricBatches();
    abortPendingSceneMetricBatchSchedules();
    clearRuntimeQueryCaches();
  }
  return nextAppId;
}

function readRuntimeQueryAppId() {
  if (typeof window === "undefined") {
    return "";
  }
  const fromShell = readShellRuntimeAppId();
  if (fromShell) {
    window.__meiRuntimeAppId = fromShell;
    return fromShell;
  }
  const fromRoute = readRouteRuntimeAppId();
  if (fromRoute) {
    window.__meiRuntimeAppId = fromRoute;
    return fromRoute;
  }
  const pageCtx = readBootstrapSeedPageContext();
  const fromBootstrap = safeTrim(pageCtx.app_id);
  if (fromBootstrap) {
    window.__meiRuntimeAppId = fromBootstrap;
    return fromBootstrap;
  }
  return safeTrim(window.__meiRuntimeAppId);
}

function readRuntimeQueryDataGeneration(bootstrap = window.__mei) {
  if (typeof window === "undefined") {
    return "";
  }
  const fromShell = readShellRuntimeDataGeneration();
  if (fromShell) {
    window.__meiRuntimeDataGeneration = fromShell;
    return fromShell;
  }
  const pageCtx = readBootstrapSeedPageContext(bootstrap);
  if (pageCtx.data_generation) {
    window.__meiRuntimeDataGeneration = pageCtx.data_generation;
  }
  return pageCtx.data_generation || "";
}

function primeBootstrapRuntimeContext(bootstrap = window.__mei) {
  if (typeof window === "undefined" || !bootstrap || typeof bootstrap !== "object") {
    return;
  }
  const pageCtx = readBootstrapSeedPageContext(bootstrap);
  const appId = readRuntimeQueryAppId() || pageCtx.app_id;
  if (appId) {
    window.__meiRuntimeAppId = appId;
  }
  const dataGen = readRuntimeQueryDataGeneration(bootstrap) || pageCtx.data_generation;
  if (dataGen) {
    window.__meiRuntimeDataGeneration = dataGen;
  }
  if (pageCtx.compile_epoch) {
    window.__meiLastCompileEpoch = pageCtx.compile_epoch;
  }
  if (pageCtx.data_generation) {
    window.__meiLastDataGeneration = pageCtx.data_generation;
  }
  window.__meiClientQueryCacheConfig = clientQueryCacheConfig({});
}

function hydrateSessionRuntimeQueryCaches() {
  if (typeof window === "undefined") {
    return 0;
  }
  const appId = readRuntimeQueryAppId();
  if (!appId) {
    return 0;
  }
  const config = window.__meiClientQueryCacheConfig || clientQueryCacheConfig({});
  if (String(config.persist || "").trim().toLowerCase() !== "sessionstorage") {
    return 0;
  }
  const dataGen = readRuntimeQueryDataGeneration() || String(window.__meiRuntimeDataGeneration || "").trim();
  const now = Date.now();
  let hydrated = 0;
  for (const entry of enumerateSessionRuntimeQueryCaches(appId, dataGen, now)) {
    if (entry.kind === "dataset") {
      const existing = cacheStore().datasetResults.get(entry.cacheKey);
      if (existing && existing.expiresAt > now) {
        continue;
      }
      cacheStore().datasetResults.set(entry.cacheKey, {
        data: entry.data,
        expiresAt: entry.expiresAt,
      });
      hydrated += 1;
      continue;
    }
    const existing = cacheStore().metricResults.get(entry.cacheKey);
    if (existing && existing.expiresAt > now) {
      continue;
    }
    cacheStore().metricResults.set(entry.cacheKey, {
      data: entry.data,
      expiresAt: entry.expiresAt,
    });
    hydrated += 1;
  }
  if (hydrated > 0) {
    notifyClientRuntimeQueryCacheHit("session-hydrate");
  }
  return hydrated;
}

function rehydrateClientRuntimeQueryCaches(bootstrap = window.__mei) {
  primeBootstrapRuntimeContext(bootstrap);
  const appId = readRuntimeQueryAppId();
  const sceneId =
    String(bootstrap?.bootstrap_scope || "").trim() ||
    String(window.__meiRuntimeSceneId || "").trim() ||
    "home";
  if (appId) {
    hydrateQueryStateStore(appId, sceneId);
    installQueryStatePersistence(appId, sceneId);
  }
  return hydrateSessionRuntimeQueryCaches();
}

let runtimeQueryCacheLifecycleInstalled = false;
function installRuntimeQueryCacheLifecycle() {
  if (typeof window === "undefined" || runtimeQueryCacheLifecycleInstalled) {
    return;
  }
  runtimeQueryCacheLifecycleInstalled = true;
  const run = () => {
    rehydrateClientRuntimeQueryCaches(window.__mei);
  };
  if (document.readyState !== "loading") {
    run();
  } else {
    document.addEventListener("DOMContentLoaded", run, { once: true });
  }
  window.addEventListener("pageshow", run);
  document.addEventListener("mei-bootstrap-ready", run);
}

let bootstrapSeedScheduled = false;
function scheduleBootstrapSeed() {
  installRuntimeQueryCacheLifecycle();
  rehydrateClientRuntimeQueryCaches(window.__mei);
  if (window.__meiBootstrapSeeded) {
    return window.__meiBootstrapSeedCount || 0;
  }
  const run = () => {
    const count = seedFromBootstrap(window.__mei);
    if (count > 0) {
      window.__meiBootstrapSeeded = true;
      window.__meiBootstrapSeedCount = count;
      delete window.__meiBootstrapSeedError;
      rehydrateClientRuntimeQueryCaches(window.__mei);
    }
    return count;
  };
  if (typeof document !== "undefined") {
    document.addEventListener(
      "mei-bootstrap-ready",
      () => {
        run();
      },
      { once: true },
    );
  }
  if (document.readyState === "loading") {
    if (!bootstrapSeedScheduled) {
      bootstrapSeedScheduled = true;
      document.addEventListener(
        "DOMContentLoaded",
        () => {
          bootstrapSeedScheduled = false;
          run();
        },
        { once: true },
      );
    }
    return 0;
  }
  return run();
}

export function bootstrapDatasetRowsDataForTest(contract, pageCtx, entryMeta = null) {
  return buildBootstrapDatasetRowsData(contract, pageCtx, entryMeta);
}

export function bootstrapDatasetPageSizesForTest() {
  return [...BOOTSTRAP_DATASET_PAGE_SIZES];
}

export function bootstrapDatasetCacheKeyForTest(api, pageCtx, datasetId, metricId, pageSize = 20) {
  const queryStatePayload = buildBootstrapQueryStatePayload();
  const payload = buildBootstrapDatasetQueryPayload(
    pageCtx,
    datasetId,
    metricId,
    queryStatePayload,
    pageSize,
  );
  return datasetQueryCacheKey(api, payload, bootstrapQueryFingerprint(pageCtx));
}

export function resolveBootstrapDatasetCacheEntryForTest(api, payload, fingerprint) {
  return resolveBootstrapDatasetCacheEntry(api, payload, fingerprint);
}

export function bootstrapMetricCacheKeyForTest(api, pageCtx, datasetId, metricIds) {
  const queryStatePayload = buildBootstrapQueryStatePayload();
  const payload = buildBootstrapMetricQueryPayload(pageCtx, datasetId, metricIds, queryStatePayload);
  return metricQueryCacheKey(api, payload, bootstrapQueryFingerprint(pageCtx));
}

export function runtimeMetricCacheKeyForTest(api, props, datasetId, metricIds) {
  const queryStatePayload = mergedQueryStatePayload("", {});
  const runtimeRef = {
    dataset_id: datasetId,
    scene_id: props?._mei?.active_scene_id,
  };
  const coords = sceneQueryCoords(props, runtimeRef);
  const payload = {
    ...coords,
    dataset_id: datasetId,
    metric_ids: [...metricIds].sort(),
    search: queryStatePayload.search || undefined,
    filters: queryStatePayload.filters,
    query_state: {
      filters: queryStatePayload.filters,
      ...(safeTrim(queryStatePayload.search) ? { search: queryStatePayload.search } : {}),
      ...(queryStatePayload.group.length > 0 ? { group: queryStatePayload.group } : {}),
      ...(queryStatePayload.timeRange ? { time_range: queryStatePayload.timeRange } : {}),
    },
    filter_intents:
      queryStatePayload.filterIntents.length > 0 ? queryStatePayload.filterIntents : undefined,
  };
  return metricQueryCacheKey(api, payload, runtimeQueryFingerprint(props));
}

function handleScopeActivationRuntimeQueryCache() {
  scheduleBootstrapSeed();
  if (typeof requestAnimationFrame === "function") {
    requestAnimationFrame(() => {
      prefetchVisiblePanelMetrics();
    });
    return;
  }
  setTimeout(() => {
    prefetchVisiblePanelMetrics();
  }, 0);
}

if (typeof window !== "undefined") {
  window.addEventListener(MEI_ABORT_RUNTIME_QUERIES, (event) => {
    const detail = event?.detail && typeof event.detail === "object" ? event.detail : {};
    abortRuntimeQueries(detail.reason || "", { clearCaches: detail.clearCaches });
  });
  window.addEventListener("meilang:scope-activation", handleScopeActivationRuntimeQueryCache);
  window.addEventListener("meilang:preview-updated", handlePreviewUpdatedRuntimeQueryCache);
  window.addEventListener("pagehide", () => {
    abortRuntimeQueries("pagehide");
  });
  window.addEventListener(MEI_PREFETCH_PANEL_METRICS, () => {
    scheduleBootstrapSeed();
    prefetchVisiblePanelMetrics();
  });
  scheduleBootstrapSeed();
  window.dispatchEvent(
    new CustomEvent(MEI_RUNTIME_QUERY_READY, {
      detail: { source: "runtime-query" },
    }),
  );
  window.__meiAbortRuntimeQueries = abortRuntimeQueries;
  window.__meiSyncRuntimeQueryAppContext = syncRuntimeQueryAppContextFromPage;
}

export function resolveDatasetLike(props) {
  const direct = props?.data || props?.value || null;
  if (direct && typeof direct === "object" && (Array.isArray(direct.rows) || Array.isArray(direct.value))) {
    return direct;
  }
  return props?.dataset?.dataset || props?.dataset || {};
}

function resolveRuntimeQueryErrorHost() {
  try {
    if (window.parent && window.parent !== window) {
      const el = window.parent.document.getElementById("mei-runtime-query-errors");
      if (el) return el;
    }
  } catch (_) {
    /* 父文档跨域或不可访问时忽略 */
  }
  return document.getElementById("mei-runtime-query-errors");
}

/**
 * 将 /api/datasets/query 或 metrics 失败写入管理页「错误与诊断」中的 #mei-runtime-query-errors。
 */
export function recordRuntimeDatasetQueryError({
  kind = "dataset_query",
  datasetId = "",
  api = "",
  status = 0,
  message = "",
  sceneId = "",
  target = "",
  component = "",
  panelId = "",
  metricId = "",
  requestId = "",
  phase = "",
} = {}) {
  const host = resolveRuntimeQueryErrorHost();
  const now = new Date();
  const time = now.toLocaleTimeString("zh-CN", { hour12: false });
  const line = {
    time,
    kind: String(kind || "dataset_query"),
    datasetId: String(datasetId || ""),
    api: String(api || ""),
    status: Number(status) || 0,
    message: String(message || "").trim().slice(0, 4000),
    sceneId: String(sceneId || "").trim(),
    target: String(target || "").trim(),
    component: String(component || "").trim(),
    panelId: String(panelId || "").trim(),
    metricId: String(metricId || "").trim(),
    requestId: String(requestId || "").trim(),
    phase: String(phase || "").trim(),
  };
  const history = Array.isArray(window.__meiRuntimeQueryErrorHistory)
    ? window.__meiRuntimeQueryErrorHistory
    : [];
  history.unshift(line);
  window.__meiRuntimeQueryErrorHistory = history.slice(0, 25);
  const reportClientError = window.__meiLangBoot?.reportClientError;
  if (typeof reportClientError === "function") {
    reportClientError({
      ...line,
      appId: currentRuntimeAppId(),
      pageUrl: window.location?.href || "",
    });
  }
  if (!host) return;
  host.innerHTML = window.__meiRuntimeQueryErrorHistory
    .map((e) => {
      const st = e.status ? `HTTP ${e.status} · ` : "";
      const apiShort = escapeHtml(e.api).slice(0, 200);
      const msg = escapeHtml(e.message);
      const ds = escapeHtml(e.datasetId);
      const k = escapeHtml(e.kind);
      const context = [
        e.phase ? `phase=${escapeHtml(e.phase)}` : "",
        e.sceneId ? `scene=${escapeHtml(e.sceneId)}` : "",
        e.target ? `file=${escapeHtml(e.target)}` : "",
        e.component ? `component=${escapeHtml(e.component)}` : "",
        e.panelId ? `panel=${escapeHtml(e.panelId)}` : "",
        e.metricId ? `metric=${escapeHtml(e.metricId)}` : "",
      ]
        .filter(Boolean)
        .join(" · ");
      const req = e.requestId
        ? `<span style="display:inline-block;margin-left:6px;padding:0 6px;border-radius:999px;border:1px solid rgba(148,163,184,.4);font-size:10px;line-height:16px;color:#cbd5e1;">req=${escapeHtml(
            e.requestId
          )}</span>`
        : "";
      return (
        `<div style="display:block;margin:6px 0;padding:8px;border-radius:8px;border:1px solid rgba(248,113,113,.4);background:rgba(127,29,29,.18);color:#fecaca;font-size:11px;line-height:1.45;">` +
        `[${escapeHtml(e.time)}] <strong>${k}</strong> · dataset=<span style="font-family:ui-monospace,monospace">${ds}</span>${req}<br/>` +
        `<span style="color:#cbd5e1">${st}${apiShort}</span><br/>` +
        (context
          ? `<span style="display:block;margin-top:4px;color:#fca5a5;">${context}</span>`
          : "") +
        `<code style="display:block;margin-top:4px;white-space:pre-wrap;word-break:break-word;color:#fecaca;">${msg}</code></div>`
      );
    })
    .join("");
}

export function isAbortError(error) {
  if (!error) return false;
  if (error.name === "AbortError") return true;
  const msg = String(error.message || error || "");
  return msg.includes("aborted") || msg.includes("AbortError");
}

function attachAbortRejectionGuard(promise) {
  if (!promise || typeof promise.catch !== "function") {
    return promise;
  }
  void promise.catch((error) => {
    if (isAbortError(error)) {
      return;
    }
  });
  return promise;
}

/**
 * 拆分 fetch 墙钟：headers 就绪（近似 TTFB）与 JSON 解析。
 * @returns {{ response, data, clientPerf }}
 */
async function fetchJsonWithClientPerf(url, init = {}) {
  const totalStart = performance.now();
  const fetchStart = performance.now();
  const response = await fetch(url, init);
  const clientTtfbMs = Math.round(performance.now() - fetchStart);
  const clientTotalMs = Math.round(performance.now() - totalStart);
  const clientPerf = {
    client_ttfb_ms: clientTtfbMs,
    client_json_ms: 0,
    client_total_ms: clientTotalMs,
    client_fetch_parse_ms: clientTotalMs,
  };
  const requestId = safeTrim(response.headers?.get?.("x-mei-request-id"));
  if (requestId) {
    clientPerf.request_id = requestId;
  }
  if (!response.ok) {
    const errorText = await response.text();
    return { response, data: null, clientPerf, errorText };
  }
  const jsonStart = performance.now();
  const data = await response.json();
  clientPerf.client_json_ms = Math.round(performance.now() - jsonStart);
  clientPerf.client_total_ms = Math.round(performance.now() - totalStart);
  clientPerf.client_fetch_parse_ms = clientPerf.client_total_ms;
  const serverHandlerMs = Number(data?.perf?.total_ms);
  if (Number.isFinite(serverHandlerMs) && serverHandlerMs >= 0) {
    clientPerf.server_handler_total_ms = serverHandlerMs;
    const gap = clientPerf.client_total_ms - serverHandlerMs;
    if (gap > 0) {
      clientPerf.client_outside_server_ms = gap;
    }
  }
  return { response, data, clientPerf, errorText: "" };
}

function mergeServerAndClientPerf(serverPerf, clientPerf) {
  const merged =
    serverPerf && typeof serverPerf === "object"
      ? Object.assign({}, serverPerf)
      : {};
  if (clientPerf && typeof clientPerf === "object") {
    Object.assign(merged, clientPerf);
  }
  if (
    !Number.isFinite(Number(merged.server_handler_total_ms)) &&
    Number.isFinite(Number(merged.total_ms))
  ) {
    merged.server_handler_total_ms = Number(merged.total_ms);
  }
  const serverHandler = Number(merged.server_handler_total_ms ?? merged.total_ms);
  const clientTotal = Number(merged.client_total_ms ?? merged.client_fetch_parse_ms);
  if (
    Number.isFinite(serverHandler) &&
    Number.isFinite(clientTotal) &&
    clientTotal > serverHandler &&
    !Number.isFinite(merged.client_outside_server_ms)
  ) {
    merged.client_outside_server_ms = Math.round(clientTotal - serverHandler);
  }
  return merged;
}

function pruneMetricQueryCaches(now = Date.now()) {
  for (const [key, entry] of cacheStore().metricResults.entries()) {
    if (!entry || !Number.isFinite(entry.expiresAt) || entry.expiresAt <= now) {
      cacheStore().metricResults.delete(key);
    }
  }
  for (const [key, entries] of cacheStore().metricScopeResults.entries()) {
    const next = (Array.isArray(entries) ? entries : []).filter(
      (entry) => entry && Number.isFinite(entry.expiresAt) && entry.expiresAt > now
    );
    if (next.length > 0) {
      cacheStore().metricScopeResults.set(key, next);
    } else {
      cacheStore().metricScopeResults.delete(key);
    }
  }
}

function pruneDatasetQueryCaches(now = Date.now()) {
  for (const [key, entry] of cacheStore().datasetResults.entries()) {
    if (!entry || !Number.isFinite(entry.expiresAt) || entry.expiresAt <= now) {
      cacheStore().datasetResults.delete(key);
    }
  }
}

function runtimeCompileEpoch(props) {
  const fromProps = String(props?._mei?.compile_epoch || "").trim();
  if (fromProps) {
    return fromProps;
  }
  if (typeof window === "undefined") {
    return "";
  }
  return String(
    window.__mei?.compile_epoch || window.__mei?.bootstrap_compile_epoch || "",
  ).trim();
}

function runtimeDataGeneration(props) {
  const fromProps = String(props?._mei?.data_generation || "").trim();
  if (fromProps) {
    return fromProps;
  }
  if (typeof window === "undefined") {
    return "";
  }
  return String(
    window.__mei?.data_generation ||
      window.__mei?.bootstrap_data_generation ||
      window.__meiRuntimeDataGeneration ||
      "",
  ).trim();
}

function runtimeQueryFingerprint(props) {
  if (typeof window !== "undefined" && bootstrapPackExpected()) {
    const boot = window.__mei || {};
    const compileEpoch = String(
      boot.bootstrap_compile_epoch ||
        boot.compileEpoch ||
        boot.compile_epoch ||
        props?._mei?.compile_epoch ||
        "",
    ).trim();
    const dataGen = String(
      boot.bootstrap_data_generation ||
        boot.dataGeneration ||
        boot.data_generation ||
        props?._mei?.data_generation ||
        window.__meiRuntimeDataGeneration ||
        "",
    ).trim();
    if (compileEpoch || dataGen) {
      return `${compileEpoch}|${dataGen}`;
    }
  }
  const compileEpoch = runtimeCompileEpoch(props);
  const dataGen = runtimeDataGeneration(props);
  return `${compileEpoch}|${dataGen}`;
}

function datasetQueryFingerprintCandidates(props, primaryFingerprint = "") {
  const candidates = [];
  const push = (value) => {
    const normalized = String(value || "").trim();
    if (normalized && !candidates.includes(normalized)) {
      candidates.push(normalized);
    }
  };
  push(primaryFingerprint);
  if (typeof window !== "undefined" && window.__mei) {
    const boot = window.__mei;
    push(
      bootstrapQueryFingerprint({
        compile_epoch:
          boot.bootstrap_compile_epoch || boot.compileEpoch || boot.compile_epoch || "",
        data_generation:
          boot.bootstrap_data_generation ||
            boot.dataGeneration ||
            boot.data_generation ||
            "",
      }),
    );
  }
  const propsEpoch = String(props?._mei?.compile_epoch || "").trim();
  const propsGen = String(props?._mei?.data_generation || "").trim();
  if (propsEpoch || propsGen) {
    push(`${propsEpoch}|${propsGen}`);
  }
  return candidates;
}

const BOOTSTRAP_SEED_WAIT_MS = 8000;
const NO_CLIENT_BOOTSTRAP_REVISION = "__no_client_bootstrap__";

function bootstrapSeedReady() {
  return !!(typeof window !== "undefined" && window.__meiBootstrapSeeded && (window.__meiBootstrapSeedCount || 0) > 0);
}

/**
 * Pack-First 仅在「可 seed 的 Eval Pack」上等待。
 * 空 pack / `__no_client_bootstrap__` 不得仅因 payloadReady 触发 8s 干等。
 * 仅有 meta revision、却无 artifact-url / 内联 metrics / payloadReady 时也不干等：
 * thin shell 在 revision_mismatch 后可能残留旧 meta，API 已返回空包。
 */
export function isSeedableBootstrapPack({
  metrics = null,
  payloadReady = false,
  clientRevision = "",
  bootstrapInlined = false,
  metaClientRevision = "",
  noClientPack = false,
  artifactUrl = "",
} = {}) {
  const rev = String(clientRevision || "").trim();
  const metaRev = String(metaClientRevision || "").trim();
  const artifact = String(artifactUrl || "").trim();
  if (
    noClientPack === true ||
    noClientPack === 1 ||
    rev === NO_CLIENT_BOOTSTRAP_REVISION ||
    metaRev === NO_CLIENT_BOOTSTRAP_REVISION
  ) {
    return false;
  }
  if (Array.isArray(metrics) && metrics.length > 0) {
    return true;
  }
  if (bootstrapInlined) {
    return true;
  }
  // revision_only：document 声明了真实 revision，且有 artifact-url 或 payload 已到
  // → 允许短暂 Pack-First 等待。裸 meta 不够（防 stale thin-shell cache）。
  if (metaRev && (artifact || payloadReady)) {
    return true;
  }
  return false;
}

function bootstrapManifestMetricIds() {
  const ids = new Set();
  const list = window.__mei?.bootstrap_metrics;
  if (!Array.isArray(list)) {
    return ids;
  }
  list.forEach((entry) => {
    const id = safeTrim(entry?.id || entry?.contract?.id);
    if (id) {
      ids.add(id);
    }
  });
  return ids;
}

function bootstrapCoversRequestedMetrics(metricIds = []) {
  const manifest = bootstrapManifestMetricIds();
  if (!manifest.size) {
    return false;
  }
  return (Array.isArray(metricIds) ? metricIds : []).every((id) =>
    manifest.has(safeTrim(id)),
  );
}

function shouldDeferUncoveredBootstrapMetricFetch(props, metricIds = []) {
  if (!bootstrapPackExpected() || !shouldEnforcePackFirst(props)) {
    return false;
  }
  if (bootstrapCoversRequestedMetrics(metricIds)) {
    return false;
  }
  // 未覆盖的 metric（含 map.bundle 大表）一律放行：由 awaitBootstrapSeedIfNeeded
  // 做短暂等待后再走网络。切勿在 seed 未就绪时对 map 直接 return null——
  // map 组件不会因 bootstrap-ready 自动重试，会导致底图图层永久空白。
  return false;
}

function readBootstrapInlinedMeta() {
  if (typeof document === "undefined") {
    return false;
  }
  const inlined = document.querySelector('meta[name="mei-bootstrap-inlined"]');
  return !!(inlined && String(inlined.getAttribute("content") || "").trim() === "1");
}

function readBootstrapClientRevisionMeta() {
  if (typeof document === "undefined") {
    return "";
  }
  const el = document.querySelector('meta[name="mei-bootstrap-client-revision"]');
  return el ? String(el.getAttribute("content") || "").trim() : "";
}

function readBootstrapArtifactUrlMeta() {
  if (typeof document === "undefined") {
    return "";
  }
  const el = document.querySelector('meta[name="mei-bootstrap-artifact-url"]');
  const fromMeta = el ? String(el.getAttribute("content") || "").trim() : "";
  if (fromMeta) {
    return fromMeta;
  }
  return String(window.__mei?.bootstrap_artifact_url || "").trim();
}

function bootstrapPackExpected() {
  if (typeof window === "undefined") {
    return false;
  }
  return isSeedableBootstrapPack({
    metrics: window.__mei?.bootstrap_metrics,
    payloadReady: !!window.__meiBootstrapPayloadReady,
    clientRevision: window.__mei?.client_revision || "",
    bootstrapInlined: readBootstrapInlinedMeta(),
    metaClientRevision: readBootstrapClientRevisionMeta(),
    noClientPack: window.__meiBootstrapNoClientPack,
    artifactUrl: readBootstrapArtifactUrlMeta(),
  });
}

function isDefaultExplanatoryQuery(
  props,
  { queryStateId = "", search = "", filters = {}, page = 1 } = {},
) {
  const normalizedPage = normalizePositiveInt(page, 1, { min: 1 });
  if (normalizedPage > 1) {
    return false;
  }
  const qsPayload = mergedQueryStatePayload(queryStateId, filters, { search });
  if (safeTrim(qsPayload.search)) {
    return false;
  }
  if (qsPayload.filters && Object.keys(qsPayload.filters).length > 0) {
    return false;
  }
  if (Array.isArray(qsPayload.group) && qsPayload.group.length > 0) {
    return false;
  }
  if (qsPayload.timeRange) {
    return false;
  }
  const dataMode = String(props?._mei?.data_mode || props?.dataMode || "eval")
    .trim()
    .toLowerCase();
  if (dataMode && dataMode !== "eval") {
    return false;
  }
  return true;
}

function shouldEnforcePackFirst(props, options = {}) {
  if (typeof window === "undefined") {
    return false;
  }
  if (!isDefaultExplanatoryQuery(props, options)) {
    return false;
  }
  return bootstrapPackExpected();
}

function awaitBootstrapSeedIfNeeded(props) {
  if (bootstrapSeedReady()) {
    return Promise.resolve(true);
  }
  if (!shouldEnforcePackFirst(props)) {
    return Promise.resolve(false);
  }
  scheduleBootstrapSeed();
  return new Promise((resolve) => {
    const finish = (ready) => {
      cleanup();
      resolve(ready);
    };
    const onReady = () => {
      finish(bootstrapSeedReady());
    };
    const timer = setTimeout(() => {
      if (!bootstrapSeedReady()) {
        window.__meiEvalPackMissReason = window.__meiEvalPackMissReason || "bootstrap_seed_timeout";
      }
      finish(bootstrapSeedReady());
    }, BOOTSTRAP_SEED_WAIT_MS);
    const cleanup = () => {
      clearTimeout(timer);
      document.removeEventListener("mei-bootstrap-ready", onReady);
    };
    document.addEventListener("mei-bootstrap-ready", onReady, { once: true });
    if (bootstrapSeedReady()) {
      finish(true);
    }
  });
}

export function isDefaultExplanatoryQueryForTest(props, options = {}) {
  return isDefaultExplanatoryQuery(props, options);
}

export function bootstrapPackExpectedForTest() {
  return bootstrapPackExpected();
}

export function isSeedableBootstrapPackForTest(input) {
  return isSeedableBootstrapPack(input);
}

function rememberHostRuntimeQueryMeta(props) {
  if (typeof window === "undefined") {
    return;
  }
  const appId = String(props?._mei?.app_id || "").trim();
  if (appId) {
    window.__meiRuntimeAppId = appId;
  }
  const dataGen = runtimeDataGeneration(props);
  if (dataGen) {
    window.__meiRuntimeDataGeneration = dataGen;
  }
  window.__meiClientQueryCacheConfig = clientQueryCacheConfig(props);
}

function persistRuntimeQueryMemoryCachesToSession() {
  if (typeof window === "undefined") {
    return;
  }
  const appId = String(window.__meiRuntimeAppId || "").trim();
  if (!appId) {
    return;
  }
  const config = window.__meiClientQueryCacheConfig || clientQueryCacheConfig({});
  if (String(config.persist || "").trim().toLowerCase() !== "sessionstorage") {
    return;
  }
  const dataGen = String(window.__meiRuntimeDataGeneration || "").trim();
  const now = Date.now();
  const metricEntries = [];
  for (const [cacheKey, entry] of cacheStore().metricResults.entries()) {
    if (entry && Number.isFinite(entry.expiresAt) && entry.expiresAt > now && entry.data) {
      metricEntries.push({ cacheKey, kind: "metric", data: entry.data });
    }
  }
  for (const [cacheKey, entry] of cacheStore().datasetResults.entries()) {
    if (entry && Number.isFinite(entry.expiresAt) && entry.expiresAt > now && entry.data) {
      metricEntries.push({ cacheKey, kind: "dataset", data: entry.data });
    }
  }
  persistMemoryRuntimeQueryCaches(
    appId,
    dataGen,
    metricEntries,
    config.ttlMs,
    config.maxEntries,
  );
}

function maybeInvalidateRuntimeQueryCachesForCompileEpoch(compileEpoch, dataGeneration = "") {
  const next = String(compileEpoch || "").trim();
  const nextDataGen = String(dataGeneration || "").trim();
  if ((!next && !nextDataGen) || typeof window === "undefined") {
    return;
  }
  const last = String(window.__meiLastCompileEpoch || "").trim();
  const lastDataGen = String(window.__meiLastDataGeneration || "").trim();
  if ((last && last !== next) || (lastDataGen && nextDataGen && lastDataGen !== nextDataGen)) {
    clearRuntimeQueryCaches({ clearSession: true });
  }
  if (next) {
    window.__meiLastCompileEpoch = next;
  }
  if (nextDataGen) {
    window.__meiLastDataGeneration = nextDataGen;
  }
}

function datasetQueryCacheKey(api, payload, fingerprint = "") {
  const epoch = String(fingerprint || "").trim();
  return `dataset|${String(api || "").trim()}|${epoch}|${stableSerialize(
    normalizeDatasetQueryCachePayload(payload),
  )}`;
}

function stableSerialize(value) {
  if (Array.isArray(value)) {
    return `[${value.map((item) => stableSerialize(item)).join(",")}]`;
  }
  if (value && typeof value === "object") {
    return `{${Object.keys(value)
      .sort()
      .map((key) => `${JSON.stringify(key)}:${stableSerialize(value[key])}`)
      .join(",")}}`;
  }
  return JSON.stringify(value ?? null);
}

function normalizeMetricQueryCachePayload(payload = {}) {
  const normalized = normalizeDatasetQueryCachePayload({ ...(payload || {}) });
  if (Array.isArray(normalized.metric_ids)) {
    normalized.metric_ids = [...normalized.metric_ids]
      .map((value) => String(value || "").trim())
      .filter(Boolean)
      .sort();
  }
  return normalized;
}

function metricQueryPayloadVariants(payload = {}) {
  return datasetQueryPayloadVariants(payload);
}

function resolveBootstrapMetricCacheEntry(
  api,
  payload,
  fingerprint,
  requestedIds,
  now = Date.now(),
  props = null,
) {
  const fingerprints = props
    ? datasetQueryFingerprintCandidates(props, fingerprint)
    : [String(fingerprint || "").trim()].filter(Boolean);
  for (const fp of fingerprints) {
    for (const variant of metricQueryPayloadVariants(payload)) {
      const cacheKey = metricQueryCacheKey(api, variant, fp);
      const cached = cacheStore().metricResults.get(cacheKey);
      if (cached && cached.expiresAt > now) {
        return { cacheKey, cached, variant, fingerprint: fp };
      }
      const scopeKey = metricQueryScopeCacheKey(api, variant, fp);
      const scopeCached = findCoveringMetricScopeResult(scopeKey, requestedIds, now);
      if (scopeCached) {
        return {
          cacheKey,
          cached: scopeCached,
          variant,
          fingerprint: fp,
          scope: true,
        };
      }
    }
  }
  return null;
}

function metricQueryCacheKey(api, payload, fingerprint = "") {
  const epoch = String(fingerprint || "").trim();
  return `${String(api || "").trim()}|${epoch}|${stableSerialize(
    normalizeMetricQueryCachePayload(payload),
  )}`;
}

function metricQueryScopeCacheKey(api, payload, fingerprint = "") {
  const scopePayload = normalizeMetricQueryCachePayload(
    payload && typeof payload === "object" ? { ...payload } : {},
  );
  delete scopePayload.metric_ids;
  // preview_scope identifies the component mount, not the metric result.
  // Excluding it lets one warmed bundle result cover all mounts requesting
  // subsets of the same dataset/query state.
  delete scopePayload.preview_scope;
  const epoch = String(fingerprint || "").trim();
  return `scope|${String(api || "").trim()}|${epoch}|${stableSerialize(scopePayload)}`;
}

function metricQueryRequestedIds(payload) {
  return Array.isArray(payload?.metric_ids)
    ? payload.metric_ids.map((value) => String(value || "").trim()).filter(Boolean)
    : [];
}

function metricQueryScopeEntryCovers(entry, requestedIds) {
  if (!entry || !Array.isArray(requestedIds)) {
    return false;
  }
  if (entry.complete === true) {
    return true;
  }
  const coveredIds = entry.metricIds instanceof Set ? entry.metricIds : new Set(entry.metricIds || []);
  return requestedIds.every((metricId) => coveredIds.has(metricId));
}

function findCoveringMetricScopeResult(scopeKey, requestedIds, now = Date.now()) {
  const entries = cacheStore().metricScopeResults.get(scopeKey);
  if (!Array.isArray(entries) || entries.length === 0) {
    return null;
  }
  const active = entries.filter(
    (entry) => entry && Number.isFinite(entry.expiresAt) && entry.expiresAt > now
  );
  if (active.length !== entries.length) {
    if (active.length > 0) {
      cacheStore().metricScopeResults.set(scopeKey, active);
    } else {
      cacheStore().metricScopeResults.delete(scopeKey);
    }
  }
  return active.find((entry) => metricQueryScopeEntryCovers(entry, requestedIds)) || null;
}

function rememberMetricScopeResult(scopeKey, requestedIds, data, expiresAt) {
  const complete = requestedIds.length === 0;
  const metricIds = complete ? new Set() : new Set(requestedIds);
  const nextEntry = {
    data,
    expiresAt,
    metricIds,
    complete,
  };
  const existing = Array.isArray(cacheStore().metricScopeResults.get(scopeKey))
    ? cacheStore().metricScopeResults.get(scopeKey)
    : [];
  const filtered = existing.filter((entry) => !metricQueryScopeEntryCovers(entry, requestedIds));
  filtered.unshift(nextEntry);
  cacheStore().metricScopeResults.set(scopeKey, filtered.slice(0, 8));
}

function findCoveringMetricScopeInflight(scopeKey, requestedIds) {
  const entries = cacheStore().metricScopeInflight.get(scopeKey);
  if (!Array.isArray(entries) || entries.length === 0) {
    return null;
  }
  return entries.find((entry) => metricQueryScopeEntryCovers(entry, requestedIds)) || null;
}

function findAnyMetricScopeInflight(scopeKey) {
  const entries = cacheStore().metricScopeInflight.get(scopeKey);
  if (!Array.isArray(entries) || entries.length === 0) {
    return null;
  }
  return entries[0] || null;
}

function registerMetricScopeInflight(scopeKey, requestedIds, promise) {
  const complete = requestedIds.length === 0;
  const entry = {
    promise,
    metricIds: complete ? new Set() : new Set(requestedIds),
    complete,
  };
  const entries = Array.isArray(cacheStore().metricScopeInflight.get(scopeKey))
    ? cacheStore().metricScopeInflight.get(scopeKey)
    : [];
  entries.push(entry);
  cacheStore().metricScopeInflight.set(scopeKey, entries);
  return entry;
}

function unregisterMetricScopeInflight(scopeKey, entry) {
  const entries = Array.isArray(cacheStore().metricScopeInflight.get(scopeKey))
    ? cacheStore().metricScopeInflight.get(scopeKey)
    : [];
  const next = entries.filter((candidate) => candidate !== entry);
  if (next.length > 0) {
    cacheStore().metricScopeInflight.set(scopeKey, next);
  } else {
    cacheStore().metricScopeInflight.delete(scopeKey);
  }
}

function clearRuntimeQueryCaches(options = {}) {
  if (options?.clearSession === true && typeof window !== "undefined") {
    const appId = String(window.__meiRuntimeAppId || "").trim();
    if (appId) {
      clearSessionRuntimeQueryCaches(appId);
    }
  }
  cacheStore().metricInflight.clear();
  cacheStore().metricResults.clear();
  cacheStore().metricScopeInflight.clear();
  cacheStore().metricScopeResults.clear();
  SCENE_METRIC_BATCH_INFLIGHT.clear();
  SCENE_METRIC_BATCH_SCHEDULES.clear();
  cacheStore().datasetInflight.clear();
  cacheStore().datasetResults.clear();
}

function notifyClientRuntimeQueryCacheHit(kind) {
  if (typeof window === "undefined") {
    return;
  }
  try {
    window.dispatchEvent(
      new CustomEvent("mei:runtime-query-client-cache-hit", {
        detail: { kind: String(kind || "dataset").trim() || "dataset" },
      }),
    );
  } catch (_) {
    /* ignore */
  }
}

function withClientResultCachePerf(data, kind = "dataset") {
  if (!data || typeof data !== "object") {
    return data;
  }
  return {
    ...data,
    perf: {
      ...(data.perf && typeof data.perf === "object" ? data.perf : {}),
      client_result_cache_hit: 1,
      client_cache_kind: String(kind || "dataset").trim() || "dataset",
    },
  };
}

function withMetricScopeSharePerf(data, flags = {}) {
  if (!data || typeof data !== "object") {
    return data;
  }
  const cacheHit = flags.cacheHit === true;
  const inflightHit = flags.inflightHit === true;
  if (!cacheHit && !inflightHit) {
    return data;
  }
  return withClientResultCachePerf(
    {
      ...data,
      perf: {
        ...(data.perf && typeof data.perf === "object" ? data.perf : {}),
        client_metric_scope_cache_hit: cacheHit ? 1 : 0,
        client_metric_scope_inflight_hit: inflightHit ? 1 : 0,
      },
    },
    "metric_scope",
  );
}

function isPrefetchMetricRequest(meta = {}) {
  const component = String(meta?.component || "").trim();
  return component === "prefetch" || component === "prefetch_viewport";
}

function shouldUseScheduledSceneMetricBatch(meta = {}) {
  if (runtimePerfDisabled("scene_metric_batch")) {
    return false;
  }
  return !(
    meta?.scene_batch_disabled === true ||
    meta?.sceneBatchDisabled === true ||
    meta?.__skipSceneBatch === true
  );
}

function sharedAbortError() {
  try {
    return new DOMException("The operation was aborted.", "AbortError");
  } catch (_) {
    const error = new Error("The operation was aborted.");
    error.name = "AbortError";
    return error;
  }
}

const STRICT_AOT_METRIC_ARTIFACT_MISSING =
  "missing strict AOT metric result artifact";
const ACCESS_ARTIFACT_GATE_MESSAGE =
  "requires prebuilt access artifacts on access-only host";
const WARMUP_TRANSIENT_ERROR_MARKERS = [
  "not found in active scene resources",
  STRICT_AOT_METRIC_ARTIFACT_MISSING,
  ACCESS_ARTIFACT_GATE_MESSAGE,
  "该指标尚未装载",
];
const HOST_ACCESS_READY_POLL_URL = "/api/host/heartbeat";
const HOST_READY_POLL_MS = 400;
const HOST_READY_TIMEOUT_MS = 45_000;
let hostAccessReadyWaitPromise = null;
let cachedHostHeartbeatPayload = null;

export function getCachedHostHeartbeatPayload() {
  return cachedHostHeartbeatPayload;
}

function formatElapsedZh(elapsedMs) {
  const ms = Number(elapsedMs);
  if (!Number.isFinite(ms) || ms < 0) {
    return "刚刚";
  }
  if (ms < 1000) {
    return `${Math.max(1, Math.round(ms))} 秒`;
  }
  if (ms < 60_000) {
    return `${Math.max(1, Math.round(ms / 1000))} 秒`;
  }
  if (ms < 3_600_000) {
    const minutes = Math.floor(ms / 60_000);
    const seconds = Math.floor((ms % 60_000) / 1000);
    return seconds > 0 ? `${minutes} 分 ${seconds} 秒` : `${minutes} 分`;
  }
  const hours = Math.floor(ms / 3_600_000);
  const minutes = Math.floor((ms % 3_600_000) / 60_000);
  return minutes > 0 ? `${hours} 小时 ${minutes} 分` : `${hours} 小时`;
}

export function isHostWarmupInProgress(payload) {
  if (!payload || typeof payload !== "object") {
    return false;
  }
  const appId = currentRuntimeAppId();
  if (appId) {
    if (appAccessReadyFromHeartbeat(payload, appId)) {
      return payload.deferredWarmupPending === true || payload.deferred_warmup_pending === true;
    }
    const phase = String(payload.phase || "").trim().toLowerCase();
    return (
      phase === "starting" ||
      phase === "bound" ||
      phase === "building" ||
      phase === "verifying"
    );
  }
  if (payload.deferredWarmupPending === true || payload.deferred_warmup_pending === true) {
    return true;
  }
  const phase = String(payload.phase || "").trim().toLowerCase();
  return phase === "starting" || phase === "building" || phase === "verifying";
}

export function isWarmupTransientRuntimeError(message) {
  const text = String(message || "");
  return WARMUP_TRANSIENT_ERROR_MARKERS.some((marker) => text.includes(marker));
}

export function formatWarmupPendingUserMessage(payload) {
  const startedAt = Number(payload?.hostStartedAtMs ?? payload?.host_started_at_ms ?? 0);
  const elapsedMs = startedAt > 0 ? Date.now() - startedAt : null;
  const ago =
    elapsedMs != null && Number.isFinite(elapsedMs) && elapsedMs >= 0
      ? formatElapsedZh(elapsedMs)
      : "刚刚";
  const detail =
    payload?.deferredWarmupPending === true || payload?.deferred_warmup_pending === true
      ? "后台仍在装载 deferred 指标"
      : ["building", "verifying", "bound"].includes(String(payload?.phase || "").trim().toLowerCase())
        ? "后台正在编译与预热"
        : !hostAccessReadyFromPayload(payload)
          ? "启动预热尚未完成"
          : "访问态产物仍在装载";
  return `系统于 ${ago} 前刚刚启动，${detail}，该指标尚未装载，请稍候刷新页面。`;
}

export function formatRuntimeQueryUserMessage(rawMessage, hostPayload = null) {
  const text = String(rawMessage || "").trim();
  if (!text) {
    return text;
  }
  if (text.includes("该指标尚未装载")) {
    return text;
  }
  const payload = hostPayload || cachedHostHeartbeatPayload;
  if (payload && isHostWarmupInProgress(payload) && isWarmupTransientRuntimeError(text)) {
    return formatWarmupPendingUserMessage(payload);
  }
  return text;
}

export function formatRuntimeQueryDisplayMessage(rawMessage, hostPayload = null) {
  const message = formatRuntimeQueryUserMessage(rawMessage, hostPayload);
  if (message.includes("该指标尚未装载")) {
    return message;
  }
  return message ? `运行时查询失败: ${message}` : "运行时查询失败";
}

function runtimeQueryHttpError(rawMessage, hostPayload = null) {
  return new Error(formatRuntimeQueryUserMessage(rawMessage, hostPayload));
}

function shouldRetryStartupArtifactFetch(response, errorText, hostPayload = null) {
  const text = String(errorText || "");
  const status = Number(response?.status);
  const payload = hostPayload || cachedHostHeartbeatPayload;
  if (status === 503) {
    if (
      text.includes(STRICT_AOT_METRIC_ARTIFACT_MISSING) ||
      text.includes(ACCESS_ARTIFACT_GATE_MESSAGE) ||
      text.includes("该指标尚未装载")
    ) {
      return true;
    }
  }
  if (
    (status === 404 || status === 503) &&
    isWarmupTransientRuntimeError(text) &&
    isHostWarmupInProgress(payload)
  ) {
    return true;
  }
  return false;
}

function waitMsWithAbort(ms, signal) {
  return new Promise((resolve, reject) => {
    if (!signal) {
      window.setTimeout(resolve, ms);
      return;
    }
    if (signal.aborted) {
      reject(sharedAbortError());
      return;
    }
    const timer = window.setTimeout(() => {
      signal.removeEventListener("abort", onAbort);
      resolve();
    }, ms);
    function onAbort() {
      window.clearTimeout(timer);
      signal.removeEventListener("abort", onAbort);
      reject(sharedAbortError());
    }
    signal.addEventListener("abort", onAbort, { once: true });
  });
}

function currentRuntimeAppId() {
  return readRuntimeQueryAppId();
}

function appAccessReadyFromHeartbeat(payload, appId) {
  if (!payload || !appId) return false;
  const apps = Array.isArray(payload.apps) ? payload.apps : [];
  const entry = apps.find((app) => String(app?.appId || "").trim() === appId);
  if (entry) return entry.accessReady === true;
  return false;
}

function hostAccessReadyFromPayload(payload) {
  const appId = currentRuntimeAppId();
  if (appId) {
    return appAccessReadyFromHeartbeat(payload, appId);
  }
  return (
    payload?.access_ready === true ||
    payload?.accessReady === true ||
    payload?.anyAppAccessReady === true ||
    payload?.any_app_access_ready === true
  );
}

async function readHostAccessReadyState(signal) {
  const response = await fetch(HOST_ACCESS_READY_POLL_URL, {
    method: "GET",
    cache: "no-store",
    credentials: "same-origin",
    headers: { accept: "application/json" },
    signal,
  });
  let payload = null;
  try {
    payload = await response.json();
  } catch (_) {
    payload = null;
  }
  cachedHostHeartbeatPayload = payload && typeof payload === "object" ? payload : null;
  return {
    response,
    payload,
    ready: response.ok && hostAccessReadyFromPayload(payload),
  };
}

async function pollHostAccessReadyUntilDeadline(signal) {
  const deadline = Date.now() + HOST_READY_TIMEOUT_MS;
  while (Date.now() < deadline) {
    if (signal?.aborted) {
      throw sharedAbortError();
    }
    try {
      const state = await readHostAccessReadyState(signal);
      if (state.ready) {
        return true;
      }
    } catch (error) {
      if (isAbortError(error)) {
        throw error;
      }
    }
    await waitMsWithAbort(HOST_READY_POLL_MS, signal);
  }
  return false;
}

function waitForHostAccessReady(signal) {
  if (!hostAccessReadyWaitPromise) {
    hostAccessReadyWaitPromise = pollHostAccessReadyUntilDeadline().finally(() => {
      hostAccessReadyWaitPromise = null;
    });
  }
  return waitForSharedPromise(hostAccessReadyWaitPromise, signal);
}

async function waitForHostAccessReadyBeforeRuntimeFetch(signal) {
  let readyState = null;
  try {
    readyState = await readHostAccessReadyState(signal);
  } catch (error) {
    if (isAbortError(error)) {
      throw error;
    }
    return false;
  }
  if (readyState.ready) {
    return true;
  }
  return waitForHostAccessReady(signal);
}

/** @param payload dataset/metric query JSON — not fetch RequestInit */
async function fetchJsonWithStartupArtifactRetry(api, payload, signal) {
  await waitForHostAccessReadyBeforeRuntimeFetch(signal);
  let result = await fetchJsonWithClientPerf(api, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(payload),
    signal,
  });
  if (result.response.ok) {
    return result;
  }
  if (!shouldRetryStartupArtifactFetch(
    result.response,
    result.errorText,
    cachedHostHeartbeatPayload
  )) {
    return result;
  }
  // background-build allows the host to bind before prebuild completes, but
  // access strict AOT still follows "prebuild first, then use". During startup
  // we poll /api/host/heartbeat (always 200) and retry once, instead of
  // hammering /api/host/ready 503s or exposing transient strict-AOT errors.
  let readyState = null;
  try {
    readyState = await readHostAccessReadyState(signal);
  } catch (error) {
    if (isAbortError(error)) {
      throw error;
    }
  }
  if (readyState?.ready) {
    return result;
  }
  const becameReady = await waitForHostAccessReady(signal);
  if (!becameReady) {
    return result;
  }
  return fetchJsonWithClientPerf(api, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(payload),
    signal,
  });
}

function waitForMetricScopeInflight(scopeInflight, requestedIds, signal) {
  return waitForSharedPromise(
    scopeInflight.promise.then((data) => {
      const metrics = filterMetricsForRequestedIds(data?.metrics, requestedIds);
      return withMetricScopeSharePerf(
        {
          ...(data && typeof data === "object" ? data : {}),
          metrics,
        },
        { inflightHit: true }
      );
    }),
    signal
  );
}

function resolveMetricScopeInflight(scopeKey, requestedIds) {
  const covering = findCoveringMetricScopeInflight(scopeKey, requestedIds);
  if (covering) {
    return covering;
  }
  const entries = cacheStore().metricScopeInflight.get(scopeKey);
  if (!Array.isArray(entries) || entries.length === 0) {
    return null;
  }
  return entries.find((entry) => metricQueryScopeEntryCovers(entry, requestedIds)) || null;
}

function waitForSharedPromise(promise, signal) {
  if (!signal) {
    return promise;
  }
  if (signal.aborted) {
    return Promise.reject(sharedAbortError());
  }
  return new Promise((resolve, reject) => {
    const onAbort = () => {
      signal.removeEventListener("abort", onAbort);
      reject(sharedAbortError());
    };
    signal.addEventListener("abort", onAbort, { once: true });
    promise.then(
      (value) => {
        signal.removeEventListener("abort", onAbort);
        resolve(value);
      },
      (error) => {
        signal.removeEventListener("abort", onAbort);
        reject(error);
      }
    );
  });
}

async function fetchRuntimeMetricsUncached(api, payload, errorContext = {}, signal = undefined) {
  const context = errorContext || {};
  let response;
  let data;
  let clientPerf = {};
  let errorText = "";
  try {
    ({ response, data, clientPerf, errorText } = await fetchJsonWithStartupArtifactRetry(
      api,
      payload,
      signal,
    ));
  } catch (error) {
    if (isAbortError(error)) {
      throw error;
    }
    recordRuntimeDatasetQueryError({
      kind: "metric_query",
      datasetId: payload.dataset_id,
      metricId:
        Array.isArray(payload.metric_ids) && payload.metric_ids.length === 1
          ? String(payload.metric_ids[0] || "")
          : "",
      api,
      status: 0,
      message: String(error?.message || error || "network fetch failed"),
      sceneId: context.scene_id,
      target: context.target,
      component: context.component,
      panelId: context.panel_id,
      requestId: context.request_id,
      phase: "metric_fetch",
    });
    throw error;
  }
  if (!response.ok) {
    const text = String(errorText || "");
    recordRuntimeDatasetQueryError({
      kind: "metric_query",
      datasetId: payload.dataset_id,
      metricId:
        Array.isArray(payload.metric_ids) && payload.metric_ids.length === 1
          ? String(payload.metric_ids[0] || "")
          : "",
      api,
      status: response.status,
      message: text,
      sceneId: context.scene_id,
      target: context.target,
      component: context.component,
      panelId: context.panel_id,
      requestId: safeTrim(clientPerf.request_id) || context.request_id,
      phase: "metric_fetch",
    });
    throw runtimeQueryHttpError(text, cachedHostHeartbeatPayload);
  }
  const serverPerf = data && typeof data.perf === "object" ? data.perf : {};
  data.perf = mergeServerAndClientPerf(serverPerf, clientPerf);
  return data;
}

function sceneMetricBatchRequestKey(api, payload) {
  return `scene_metric_batch|${String(api || "").trim()}|${stableSerialize(payload)}`;
}

function singleMetricPayloadFromBatchPayload(batchPayload, group) {
  return {
    ...batchPayload,
    dataset_id: safeTrim(group?.dataset_id),
    metric_ids: Array.isArray(group?.metric_ids) ? [...group.metric_ids] : [],
  };
}

function sceneMetricBatchGroupPerf(serverPerf, clientPerf, groupCount) {
  const perf = mergeServerAndClientPerf(serverPerf, {});
  if (Number.isFinite(Number(clientPerf?.client_ttfb_ms))) {
    perf.client_batch_ttfb_ms = Number(clientPerf.client_ttfb_ms);
  }
  if (Number.isFinite(Number(clientPerf?.client_json_ms))) {
    perf.client_batch_json_ms = Number(clientPerf.client_json_ms);
  }
  if (Number.isFinite(Number(clientPerf?.client_total_ms))) {
    perf.client_batch_total_ms = Number(clientPerf.client_total_ms);
  }
  if (Number.isFinite(Number(clientPerf?.client_fetch_parse_ms))) {
    perf.client_batch_fetch_parse_ms = Number(clientPerf.client_fetch_parse_ms);
  }
  const requestId = safeTrim(clientPerf?.request_id);
  if (requestId) {
    perf.request_id = requestId;
  }
  perf.client_scene_batch = 1;
  perf.client_scene_batch_group_count = Number(groupCount) || 0;
  return perf;
}

function registerSceneMetricBatchGroupInflight(api, batchPayload, group, compileEpoch = "") {
  const singlePayload = singleMetricPayloadFromBatchPayload(batchPayload, group);
  const cacheKey = metricQueryCacheKey(api, singlePayload, compileEpoch);
  const scopeKey = metricQueryScopeCacheKey(api, singlePayload, compileEpoch);
  const requestedIds = metricQueryRequestedIds(singlePayload);
  let resolvePromise;
  let rejectPromise;
  const promise = new Promise((resolve, reject) => {
    resolvePromise = resolve;
    rejectPromise = reject;
  });
  const scopeEntry = registerMetricScopeInflight(scopeKey, requestedIds, promise);
  cacheStore().metricInflight.set(cacheKey, { promise });
  return {
    datasetId: safeTrim(group?.dataset_id),
    cacheKey,
    scopeKey,
    requestedIds,
    resolve: resolvePromise,
    reject: rejectPromise,
    cleanup() {
      cacheStore().metricInflight.delete(cacheKey);
      unregisterMetricScopeInflight(scopeKey, scopeEntry);
    },
  };
}

async function fetchSceneRuntimeMetricBatchUncached(
  api,
  payload,
  errorContext = {},
  signal = undefined
) {
  const context = errorContext || {};
  let response;
  let data;
  let clientPerf = {};
  let errorText = "";
  try {
    ({ response, data, clientPerf, errorText } = await fetchJsonWithStartupArtifactRetry(
      api,
      payload,
      signal,
    ));
  } catch (error) {
    if (isAbortError(error)) {
      throw error;
    }
    recordRuntimeDatasetQueryError({
      kind: "metric_query_batch",
      datasetId: "__scene_batch__",
      api,
      status: 0,
      message: String(error?.message || error || "network fetch failed"),
      sceneId: context.scene_id,
      target: context.target,
      component: context.component,
      panelId: context.panel_id,
      requestId: context.request_id,
      phase: "metric_batch_fetch",
    });
    throw error;
  }
  if (!response.ok) {
    const text = String(errorText || "");
    recordRuntimeDatasetQueryError({
      kind: "metric_query_batch",
      datasetId: "__scene_batch__",
      api,
      status: response.status,
      message: text,
      sceneId: context.scene_id,
      target: context.target,
      component: context.component,
      panelId: context.panel_id,
      requestId: safeTrim(clientPerf.request_id) || context.request_id,
      phase: "metric_batch_fetch",
    });
    throw runtimeQueryHttpError(text, cachedHostHeartbeatPayload);
  }
  return { data, clientPerf };
}

export async function fetchSceneRuntimeMetricBatch(
  props,
  groups,
  {
    queryStateId = "",
    search = "",
    filters = {},
    signal = undefined,
    meta = {},
  } = {}
) {
  if (!devEvalAllowsRuntimeQuery(props)) {
    return null;
  }
  const capability = metricBatchQueryCapabilityConfig(props);
  const api = capability.api;
  const normalizedGroups = normalizeSceneMetricBatchGroups(groups);
  if (!capability.enabled || normalizedGroups.length === 0) {
    return null;
  }
  const runtimeRef = resolveRuntimeMetricRef(props);
  if (!runtimeRef?.dataset_id) {
    return null;
  }
  const effectiveQueryStateId = String(queryStateId || queryStateIdOf(props) || "").trim();
  const queryFingerprint = runtimeQueryFingerprint(props);
  rememberHostRuntimeQueryMeta(props);
  maybeInvalidateRuntimeQueryCachesForCompileEpoch(
    runtimeCompileEpoch(props),
    runtimeDataGeneration(props),
  );
  const cacheConfig = clientQueryCacheConfig(props);
  const appId = String(props?._mei?.app_id || "").trim();
  const dataGen = runtimeDataGeneration(props);
  const queryStatePayload = mergedQueryStatePayload(effectiveQueryStateId, filters, {
    search,
    filterIntentSource: meta?.filter_intent_source ?? meta?.filterIntentSource,
  });
  const baseCoords = sceneQueryCoords(props, runtimeRef);
  const coords = capability.requiresSceneId
    ? requireSceneQualifiedRequest(baseCoords, "metric batch query", meta)
    : baseCoords;
  const basePayload = {
    ...coords,
    preview_scope: runtimePreviewScope(props) || undefined,
    search: queryStatePayload.search || undefined,
    filters: queryStatePayload.filters,
    query_state: {
      filters: queryStatePayload.filters,
      ...(safeTrim(queryStatePayload.search) ? { search: queryStatePayload.search } : {}),
      ...(queryStatePayload.group.length > 0 ? { group: queryStatePayload.group } : {}),
      ...(queryStatePayload.timeRange ? { time_range: queryStatePayload.timeRange } : {}),
    },
    filter_intents:
      queryStatePayload.filterIntents.length > 0 ? queryStatePayload.filterIntents : undefined,
  };
  const now = Date.now();
  pruneMetricQueryCaches(now);
  const pendingGroups = normalizedGroups.filter((group) => {
    const singlePayload = singleMetricPayloadFromBatchPayload(basePayload, group);
    const cacheKey = metricQueryCacheKey(api, singlePayload, queryFingerprint);
    const scopeKey = metricQueryScopeCacheKey(api, singlePayload, queryFingerprint);
    const requestedIds = metricQueryRequestedIds(singlePayload);
    const cached = cacheStore().metricResults.get(cacheKey);
    if (cached && cached.expiresAt > now) {
      return false;
    }
    const sessionCached = readSessionRuntimeQueryCache(appId, cacheKey, dataGen, now);
    if (sessionCached?.data) {
      cacheStore().metricResults.set(cacheKey, {
        data: sessionCached.data,
        expiresAt: sessionCached.expiresAt,
      });
      return false;
    }
    if (findCoveringMetricScopeResult(scopeKey, requestedIds, now)) {
      return false;
    }
    if (findCoveringMetricScopeInflight(scopeKey, requestedIds)) {
      return false;
    }
    return true;
  });
  if (pendingGroups.length === 0) {
    return null;
  }
  const payload = {
    ...basePayload,
    metric_groups: pendingGroups,
  };
  const errorContext = {
    scene_id: safeTrim(payload.scene_id || props?._mei?.active_scene_id),
    target: safeTrim(payload.target || props?._mei?.active_target_file),
    component: safeTrim(meta?.component),
    panel_id: safeTrim(meta?.panel_id || meta?.panelId),
    request_id: safeTrim(meta?.request_id || meta?.requestId),
  };
  const batchKey = sceneMetricBatchRequestKey(api, payload);
  let shared = SCENE_METRIC_BATCH_INFLIGHT.get(batchKey);
  if (!shared) {
    const managedController = createManagedAbortController();
    const registrations = pendingGroups.map((group) =>
      registerSceneMetricBatchGroupInflight(api, basePayload, group, queryFingerprint)
    );
    const promise = fetchSceneRuntimeMetricBatchUncached(
      api,
      payload,
      errorContext,
      managedController.signal,
    )
      .then(({ data, clientPerf }) => {
        const expiresAt = Date.now() + METRIC_QUERY_CACHE_TTL_MS;
        const groupMap = new Map(
          (Array.isArray(data?.groups) ? data.groups : []).map((group) => [
            safeTrim(group?.dataset_id),
            group,
          ])
        );
        for (const registration of registrations) {
          const group = groupMap.get(registration.datasetId);
          const normalized = {
            scene_id: safeTrim(data?.scene_id),
            scene_path: safeTrim(data?.scene_path) || undefined,
            dataset_id: registration.datasetId,
            total_rows: Number(group?.total_rows) || 0,
            metrics: Array.isArray(group?.metrics) ? group.metrics : [],
            perf: sceneMetricBatchGroupPerf(group?.perf, clientPerf, registrations.length),
          };
          cacheStore().metricResults.set(registration.cacheKey, {
            data: normalized,
            expiresAt,
          });
          writeSessionRuntimeQueryCache(
            appId,
            registration.cacheKey,
            dataGen,
            "metric",
            normalized,
            cacheConfig.ttlMs,
            cacheConfig.maxEntries,
          );
          rememberMetricScopeResult(
            registration.scopeKey,
            registration.requestedIds,
            normalized,
            expiresAt
          );
          registration.resolve(normalized);
        }
        return data;
      })
      .catch((error) => {
        registrations.forEach((registration) => registration.reject(error));
        throw error;
      })
      .finally(() => {
        managedController.__meiRelease?.();
        SCENE_METRIC_BATCH_INFLIGHT.delete(batchKey);
        registrations.forEach((registration) => registration.cleanup());
      });
    shared = { promise };
    SCENE_METRIC_BATCH_INFLIGHT.set(batchKey, shared);
  }
  return waitForSharedPromise(shared.promise, signal);
}

function sceneMetricBatchScheduleKey(api, props, queryStateId = "", filters = {}, search = "") {
  const scopeKey = sceneMetricBatchScopeKey(props, queryStateId, filters, search);
  if (!scopeKey) {
    return "";
  }
  return `${String(api || "").trim()}|${scopeKey}`;
}

function scheduledSceneMetricMeta(meta = {}) {
  return {
    ...(meta || {}),
    scene_batch_disabled: true,
    __skipSceneBatch: true,
  };
}

function scheduleSceneRuntimeMetricRequest(
  api,
  props,
  metricIds,
  {
    queryStateId = "",
    search = "",
    filters = {},
    signal = undefined,
    meta = {},
    datasetId: explicitDatasetId = "",
  } = {}
) {
  if (!devEvalAllowsRuntimeQuery(props)) {
    return staticMetricResultForBlockedQuery(props, metricIds);
  }
  if (shouldPauseHomeRuntimeMetricFetch(props)) {
    return null;
  }
  const capability = metricBatchQueryCapabilityConfig(props);
  if (!capability.enabled) {
    return null;
  }
  const runtimeRef = resolveRuntimeMetricRef(props);
  const datasetId = safeTrim(explicitDatasetId) || safeTrim(runtimeRef?.dataset_id);
  if (!datasetId) {
    return null;
  }
  const requestedIds = [...new Set(
    (Array.isArray(metricIds) ? metricIds : [])
      .map((value) => safeTrim(value))
      .filter(Boolean)
  )].sort();
  if (requestedIds.length === 0) {
    return null;
  }
  const effectiveQueryStateId = String(queryStateId || queryStateIdOf(props) || "").trim();
  const scheduleKey = sceneMetricBatchScheduleKey(
    api,
    props,
    effectiveQueryStateId,
    filters,
    search
  );
  if (!scheduleKey) {
    return null;
  }
  let schedule = SCENE_METRIC_BATCH_SCHEDULES.get(scheduleKey);
  if (!schedule) {
    schedule = {
      props,
      queryStateId: effectiveQueryStateId,
      search,
      filters,
      requests: [],
      cancelFlush: null,
      flush: null,
    };
    SCENE_METRIC_BATCH_SCHEDULES.set(scheduleKey, schedule);
  } else {
    schedule.props = props;
    schedule.queryStateId = effectiveQueryStateId;
    schedule.search = search;
    schedule.filters = filters;
  }
  let resolvePromise;
  let rejectPromise;
  const promise = new Promise((resolve, reject) => {
    resolvePromise = resolve;
    rejectPromise = reject;
  });
  schedule.requests.push({
    props,
    datasetId,
    metricIds: requestedIds,
    queryStateId: effectiveQueryStateId,
    search,
    filters,
    signal,
    meta,
    resolve: resolvePromise,
    reject: rejectPromise,
  });
  if (typeof schedule.flush !== "function") {
    schedule.flush = async () => {
      schedule.cancelFlush = null;
      const active = SCENE_METRIC_BATCH_SCHEDULES.get(scheduleKey);
      if (!active || active !== schedule) {
        return;
      }
      if (shouldPauseHomeRuntimeMetricFetch(schedule.props)) {
        scheduleSceneMetricBatchFlush(160);
        return;
      }
      SCENE_METRIC_BATCH_SCHEDULES.delete(scheduleKey);
      const requests = schedule.requests.splice(0);
      const liveRequests = requests.filter((request) => request?.signal?.aborted !== true);
      if (liveRequests.length === 0) {
        return;
      }
      const requestByDatasetId = new Map();
      for (const request of liveRequests) {
        const requestDatasetId =
          safeTrim(request.datasetId) ||
          safeTrim(resolveRuntimeMetricRef(request.props)?.dataset_id);
        if (!requestDatasetId || requestByDatasetId.has(requestDatasetId)) {
          continue;
        }
        requestByDatasetId.set(requestDatasetId, request);
      }
      const groups = normalizeSceneMetricBatchGroups(
        liveRequests.map((request) => ({
          dataset_id:
            safeTrim(request.datasetId) ||
            resolveRuntimeMetricRef(request.props)?.dataset_id ||
            datasetId,
          metric_ids: request.metricIds,
        }))
      );
      try {
        let sceneBatchData = null;
        if (groups.length > 1) {
          sceneBatchData = await fetchSceneRuntimeMetricBatch(schedule.props, groups, {
            queryStateId: schedule.queryStateId,
            search: schedule.search,
            filters: schedule.filters,
            meta: scheduledSceneMetricMeta({
              component: safeTrim(meta?.component) || "scene_batch_schedule",
              panel_id: meta?.panel_id ?? meta?.panelId,
              request_id: meta?.request_id ?? meta?.requestId,
            }),
          });
        }
        if (groups.length === 1) {
          const singleDatasetId = safeTrim(groups[0].dataset_id) || datasetId;
          const singleRequest =
            requestByDatasetId.get(singleDatasetId) || liveRequests[0] || schedule.requests[0] || null;
          const batchData = await fetchRuntimeMetrics(singleRequest?.props || schedule.props, {
            metricIds: groups[0].metric_ids,
            queryStateId: schedule.queryStateId,
            search: schedule.search,
            filters: schedule.filters,
            meta: scheduledSceneMetricMeta({
              component: safeTrim(meta?.component) || "scene_batch_schedule",
              panel_id: meta?.panel_id ?? meta?.panelId,
              request_id: meta?.request_id ?? meta?.requestId,
            }),
          });
          for (const request of liveRequests) {
            const reqDatasetId =
              safeTrim(request.datasetId) ||
              resolveRuntimeMetricRef(request.props)?.dataset_id ||
              singleDatasetId;
            const projected = projectScheduledSingleDatasetMetricResult(
              batchData,
              reqDatasetId,
              request.metricIds
            );
            if (projected) {
              request.resolve(projected);
            } else {
              request.reject(new Error("scene metric batch projection failed"));
            }
          }
          return;
        }
        if (groups.length > 1 && sceneBatchData) {
          const unresolved = [];
          for (const request of liveRequests) {
            const reqDatasetId =
              safeTrim(request.datasetId) ||
              resolveRuntimeMetricRef(request.props)?.dataset_id ||
              datasetId;
            const projected = projectScheduledSceneMetricBatchResult(
              sceneBatchData,
              reqDatasetId,
              request.metricIds
            );
            if (projected && Array.isArray(projected.metrics) && projected.metrics.length > 0) {
              request.resolve(projected);
            } else {
              unresolved.push(request);
            }
          }
          if (unresolved.length > 0) {
            await Promise.all(
              unresolved.map(async (request) => {
                const reqDatasetId =
                  safeTrim(request.datasetId) ||
                  safeTrim(resolveRuntimeMetricRef(request.props)?.dataset_id) ||
                  safeTrim(datasetId);
                try {
                  const data = await fetchRuntimeMetrics(request.props, {
                    metricIds: request.metricIds,
                    queryStateId: request.queryStateId,
                    search: request.search,
                    filters: request.filters,
                    signal: request.signal,
                    meta: scheduledSceneMetricMeta(request.meta),
                  });
                  const projected = projectScheduledSingleDatasetMetricResult(
                    data,
                    reqDatasetId,
                    request.metricIds
                  );
                  if (projected) {
                    request.resolve(projected);
                  } else {
                    request.reject(new Error("scene metric batch projection failed"));
                  }
                } catch (error) {
                  request.reject(error);
                }
              })
            );
          }
          return;
        }
        if (groups.length > 1) {
          const groupDataByDataset = new Map();
          await Promise.all(
            groups.map(async (group) => {
              const groupDatasetId = safeTrim(group.dataset_id);
              if (!groupDatasetId) {
                return;
              }
              const groupRequest = requestByDatasetId.get(groupDatasetId);
              if (!groupRequest?.props) {
                return;
              }
              const data = await fetchRuntimeMetrics(groupRequest.props, {
                metricIds: group.metric_ids,
                queryStateId: schedule.queryStateId,
                search: schedule.search,
                filters: schedule.filters,
                meta: scheduledSceneMetricMeta({
                  component: safeTrim(meta?.component) || "scene_batch_schedule",
                  panel_id: meta?.panel_id ?? meta?.panelId,
                  request_id: meta?.request_id ?? meta?.requestId,
                }),
              });
              groupDataByDataset.set(groupDatasetId, data);
            })
          );
          for (const request of liveRequests) {
            const reqDatasetId =
              safeTrim(request.datasetId) ||
              safeTrim(resolveRuntimeMetricRef(request.props)?.dataset_id) ||
              safeTrim(datasetId);
            const projected = projectScheduledSingleDatasetMetricResult(
              groupDataByDataset.get(reqDatasetId),
              reqDatasetId,
              request.metricIds
            );
            if (projected) {
              request.resolve(projected);
            } else {
              request.reject(new Error("scene metric batch projection failed"));
            }
          }
          return;
        }
        await Promise.all(
          liveRequests.map(async (request) => {
            const data = await fetchRuntimeMetrics(request.props, {
              metricIds: request.metricIds,
              queryStateId: request.queryStateId,
              search: request.search,
              filters: request.filters,
              signal: request.signal,
              meta: scheduledSceneMetricMeta(request.meta),
            });
            request.resolve(data);
          })
        );
      } catch (error) {
        liveRequests.forEach((request) => request.reject(error));
      }
    };
  }
  scheduleSceneMetricBatchFlush();
  return attachAbortRejectionGuard(waitForSharedPromise(promise, signal));
}

export async function fetchDatasetRows(
  props,
  {
    page = 1,
    pageSize = 0,
    queryStateId = "",
    search = "",
    filters = {},
    full = false,
    sort = [],
    columnState = null,
    summary = false,
    facetColumns = [],
    signal = undefined,
    meta = {},
  } = {}
) {
  if (!devEvalAllowsRuntimeQuery(props)) {
    return staticDatasetRowsForBlockedQuery(props);
  }
  const capability = resolveDatasetQueryCapability(props);
  const api = capability.api;
  const effectiveQueryStateId = String(queryStateId || queryStateIdOf(props) || "").trim();
  const metricRef = resolveRuntimeMetricRef(props);
  const dataRef = resolveRuntimeDataRef(props);
  const dataset = resolveDatasetLike(props);
  const datasetId = String(
    metricRef?.dataset_id || dataRef?.dataset_id || dataset?.id || ""
  ).trim();
  const metricId = String(metricRef?.metric_id || "").trim();
  if (!capability.enabled) {
    return null;
  }
  if (!datasetId) {
    console.warn('[runtime-query] fetchDatasetRows skipped: missing dataset_id in props', {
      metricRef,
      dataRef,
      dataset: dataset?.id,
      meta,
    });
    return null;
  }
  const runtimeRef = metricRef || dataRef;
  const baseCoords = sceneQueryCoords(props, runtimeRef);
  const coords = capability.requiresSceneId
    ? requireSceneQualifiedRequest(baseCoords, "dataset query", meta)
    : baseCoords;
  const normalizedPage = normalizePositiveInt(page, 1, { min: 1 });
  const normalizedPageSize = normalizePositiveInt(pageSize, 0, { min: 0 });
  const normalizedSort = Array.isArray(sort)
    ? sort
        .map((item) => ({
          field: String(item?.field || "").trim(),
          direction: String(item?.direction || "asc").trim().toLowerCase() || "asc",
        }))
        .filter((item) => item.field)
    : [];
  const normalizedColumnState = normalizeColumnStateForRequest(columnState);
  const queryStatePayload = mergedQueryStatePayload(effectiveQueryStateId, filters, {
    search,
    filterIntentSource: meta?.filter_intent_source ?? meta?.filterIntentSource,
  });
  const payload = {
    ...coords,
    preview_scope: runtimePreviewScope(props) || undefined,
    dataset_id: datasetId,
    metric_id: metricId || undefined,
    page: normalizedPage,
    page_size: normalizedPageSize,
    search: queryStatePayload.search || undefined,
    filters: queryStatePayload.filters,
    query_state: {
      filters: queryStatePayload.filters,
      ...(safeTrim(queryStatePayload.search) ? { search: queryStatePayload.search } : {}),
      ...(queryStatePayload.group.length > 0 ? { group: queryStatePayload.group } : {}),
      ...(queryStatePayload.timeRange ? { time_range: queryStatePayload.timeRange } : {}),
    },
    filter_intents:
      metricId && queryStatePayload.filterIntents.length > 0
        ? queryStatePayload.filterIntents
        : undefined,
    full: !!full,
    sort: normalizedSort.length > 0 ? normalizedSort : undefined,
    column_state: normalizedColumnState,
    summary: summary === true,
    ...(Array.isArray(facetColumns) && facetColumns.length > 0
      ? {
          facet_columns: [
            ...new Set(facetColumns.map((value) => String(value || "").trim()).filter(Boolean)),
          ],
        }
      : {}),
  };
  const errorContext = {
    scene_id: safeTrim(payload.scene_id || props?._mei?.active_scene_id),
    target: safeTrim(payload.target || props?._mei?.active_target_file),
    component: safeTrim(meta?.component),
    panel_id: safeTrim(meta?.panel_id || meta?.panelId),
    request_id: safeTrim(meta?.request_id || meta?.requestId),
  };
  const compileEpoch = runtimeCompileEpoch(props);
  const queryFingerprint = runtimeQueryFingerprint(props);
  rememberHostRuntimeQueryMeta(props);
  maybeInvalidateRuntimeQueryCachesForCompileEpoch(compileEpoch, runtimeDataGeneration(props));
  if (runtimePerfDisabled("runtime_dataset_share")) {
    const managedController = createManagedAbortController([signal]);
    return fetchDatasetRowsUncached(api, payload, errorContext, {
      metricId,
      datasetId,
      signal: managedController.signal,
    }).finally(() => {
      managedController.__meiRelease?.();
    });
  }
  const cacheKey = datasetQueryCacheKey(api, payload, queryFingerprint);
  const now = Date.now();
  const cacheConfig = clientQueryCacheConfig(props);
  const appId = String(props?._mei?.app_id || "").trim();
  const dataGen = runtimeDataGeneration(props);
  pruneDatasetQueryCaches(now);
  const evalStoreDataset = readEvalStoreDatasetData(api, payload, queryFingerprint);
  if (evalStoreDataset) {
    notifyClientRuntimeQueryCacheHit("dataset_eval_store");
    return waitForSharedPromise(
      Promise.resolve(withClientResultCachePerf(evalStoreDataset, "dataset_eval_store")),
      signal,
    );
  }
  const cached = cacheStore().datasetResults.get(cacheKey);
  if (cached && cached.expiresAt > now) {
    notifyClientRuntimeQueryCacheHit("dataset");
    return waitForSharedPromise(
      Promise.resolve(withClientResultCachePerf(cached.data, "dataset")),
      signal,
    );
  }
  const sessionCached = readSessionRuntimeQueryCache(appId, cacheKey, dataGen, now);
  if (sessionCached?.data) {
    cacheStore().datasetResults.set(cacheKey, {
      data: sessionCached.data,
      expiresAt: sessionCached.expiresAt,
    });
    notifyClientRuntimeQueryCacheHit("dataset_session");
    return waitForSharedPromise(
      Promise.resolve(withClientResultCachePerf(sessionCached.data, "dataset_session")),
      signal,
    );
  }
  if (
    packFirstAppliesToDatasetFetch(props, {
      metricId,
      queryStateId: effectiveQueryStateId,
      search,
      filters,
      page: normalizedPage,
    })
  ) {
    await awaitBootstrapSeedIfNeeded(props);
    const nowAfterSeed = Date.now();
    const cachedAfterSeed = cacheStore().datasetResults.get(cacheKey);
    if (cachedAfterSeed && cachedAfterSeed.expiresAt > nowAfterSeed) {
      notifyClientRuntimeQueryCacheHit("dataset_bootstrap");
      return waitForSharedPromise(
        Promise.resolve(withClientResultCachePerf(cachedAfterSeed.data, "dataset_bootstrap")),
        signal,
      );
    }
    const bootstrapHit = resolveBootstrapDatasetCacheEntry(
      api,
      payload,
      queryFingerprint,
      nowAfterSeed,
      props,
    );
    if (bootstrapHit?.cached) {
      cacheStore().datasetResults.set(cacheKey, bootstrapHit.cached);
      notifyClientRuntimeQueryCacheHit("dataset_bootstrap_variant");
      window.__meiEvalPackSource = window.__meiEvalPackSource || "bootstrap_seed";
      delete window.__meiEvalPackMissReason;
      return waitForSharedPromise(
        Promise.resolve(withClientResultCachePerf(bootstrapHit.cached.data, "dataset_bootstrap_variant")),
        signal,
      );
    }
    const sessionAfterSeed = readSessionRuntimeQueryCache(appId, cacheKey, dataGen, nowAfterSeed);
    if (sessionAfterSeed?.data) {
      cacheStore().datasetResults.set(cacheKey, {
        data: sessionAfterSeed.data,
        expiresAt: sessionAfterSeed.expiresAt,
      });
      notifyClientRuntimeQueryCacheHit("dataset_bootstrap_session");
      return waitForSharedPromise(
        Promise.resolve(withClientResultCachePerf(sessionAfterSeed.data, "dataset_bootstrap_session")),
        signal,
      );
    }
    if (bootstrapPackExpected() && bootstrapSeedReady()) {
      // map.bundle 未进 bootstrap 清单的大表不再永久 defer，落到下方网络/JIT 路径。
      window.__meiEvalPackMissReason = window.__meiEvalPackMissReason || "dataset_cache_miss_after_seed";
      if (recentScopeActivationMatches(props)) {
        const sceneId = String(
          props?._mei?.scene_id ?? props?._mei?.scene ?? props?.scene_id ?? "",
        ).trim();
        if (sceneId && typeof scheduleSceneMetricBatchFlush === "function") {
          scheduleSceneMetricBatchFlush(sceneId);
        }
        let jitHit = null;
        for (let attempt = 0; attempt < 3; attempt += 1) {
          await sleepMs(80);
          const retryNow = Date.now();
          const retryCached = cacheStore().datasetResults.get(cacheKey);
          if (retryCached && retryCached.expiresAt > retryNow) {
            jitHit = retryCached;
            break;
          }
          const retryBootstrap = resolveBootstrapDatasetCacheEntry(
            api,
            payload,
            queryFingerprint,
            retryNow,
            props,
          );
          if (retryBootstrap?.cached) {
            cacheStore().datasetResults.set(cacheKey, retryBootstrap.cached);
            jitHit = retryBootstrap.cached;
            break;
          }
        }
        if (jitHit?.data) {
          window.__meiEvalPackMissReason = "jit_batch_hit";
          delete window.__meiEvalPackFallbackNetwork;
          notifyClientRuntimeQueryCacheHit("dataset_jit_batch");
          return waitForSharedPromise(
            Promise.resolve(withClientResultCachePerf(jitHit.data, "dataset_jit_batch")),
            signal,
          );
        }
        window.__meiEvalPackMissReason = "jit_batch_miss";
      }
      if (typeof window !== "undefined") {
        window.__meiLastDatasetCacheMiss = {
          api,
          cacheKey,
          fingerprint: queryFingerprint,
          payload: normalizeDatasetQueryCachePayload(payload),
        };
        window.__meiEvalPackFallbackNetwork = 1;
      }
      // Bootstrap seed ran but cache keys diverged (scene/page_size/fingerprint) — fall back to network.
    }
  }
  let shared = cacheStore().datasetInflight.get(cacheKey);
  if (!shared) {
    const managedController = createManagedAbortController();
    const promise = fetchDatasetRowsUncached(api, payload, errorContext, {
      metricId,
      datasetId,
      signal: managedController.signal,
    })
      .then((data) => {
        const expiresAt = Date.now() + cacheConfig.ttlMs;
        cacheStore().datasetResults.set(cacheKey, {
          data,
          expiresAt,
        });
        writeSessionRuntimeQueryCache(
          appId,
          cacheKey,
          dataGen,
          "dataset",
          data,
          cacheConfig.ttlMs,
          cacheConfig.maxEntries,
        );
        return data;
      })
      .finally(() => {
        managedController.__meiRelease?.();
        cacheStore().datasetInflight.delete(cacheKey);
      });
    shared = { promise };
    cacheStore().datasetInflight.set(cacheKey, shared);
  }
  return waitForSharedPromise(shared.promise, signal);
}

async function fetchDatasetRowsUncached(
  api,
  payload,
  errorContext,
  { metricId = "", datasetId = "", signal = undefined } = {}
) {
  let response;
  let data;
  let clientPerf = {};
  let errorText = "";
  try {
    ({ response, data, clientPerf, errorText } = await fetchJsonWithStartupArtifactRetry(api, payload, signal));
  } catch (error) {
    if (isAbortError(error)) {
      throw error;
    }
    recordRuntimeDatasetQueryError({
      kind: metricId ? "metric_dataframe_query" : "dataset_query",
      datasetId: metricId ? `${datasetId}/${metricId}` : datasetId,
      metricId,
      api,
      status: 0,
      message: String(error?.message || error || "network fetch failed"),
      sceneId: errorContext.scene_id,
      target: errorContext.target,
      component: errorContext.component,
      panelId: errorContext.panel_id,
      requestId: errorContext.request_id,
      phase: metricId ? "metric_dataframe_fetch" : "dataset_fetch",
    });
    throw error;
  }
  if (!response.ok) {
    const text = String(errorText || "");
    recordRuntimeDatasetQueryError({
      kind: metricId ? "metric_dataframe_query" : "dataset_query",
      datasetId: metricId ? `${datasetId}/${metricId}` : datasetId,
      metricId,
      api,
      status: response.status,
      message: text,
      sceneId: errorContext.scene_id,
      target: errorContext.target,
      component: errorContext.component,
      panelId: errorContext.panel_id,
      requestId: safeTrim(clientPerf.request_id) || errorContext.request_id,
      phase: metricId ? "metric_dataframe_fetch" : "dataset_fetch",
    });
    throw runtimeQueryHttpError(text, cachedHostHeartbeatPayload);
  }
  const serverPerf = data && typeof data.perf === "object" ? data.perf : {};
  data.perf = mergeServerAndClientPerf(serverPerf, clientPerf);
  return data;
}

export async function fetchRuntimeMetrics(
  props,
  {
    metricIds = null,
    queryStateId = "",
    search = "",
    filters = {},
    signal = undefined,
    meta = {},
  } = {}
) {
  if (isStaticSkeletonDisplay(props)) {
    return null;
  }
  if (!devEvalAllowsRuntimeQuery(props)) {
    return null;
  }
  const capability = metricQueryCapabilityConfig(props);
  const api = capability.api;
  const effectiveQueryStateId = String(queryStateId || queryStateIdOf(props) || "").trim();
  const runtimeRef = resolveRuntimeMetricRef(props);
  if (!capability.enabled || !runtimeRef?.dataset_id) {
    return null;
  }
  const ids =
    Array.isArray(metricIds) && metricIds.length > 0
      ? metricIds
      : runtimeRef.metric_id
      ? [runtimeRef.metric_id]
      : [];
  if (shouldDeferUncoveredBootstrapMetricFetch(props, ids)) {
    window.__meiEvalPackSource = window.__meiEvalPackSource || "bootstrap_partial_defer";
    return null;
  }
  const queryStatePayload = mergedQueryStatePayload(effectiveQueryStateId, filters, {
    search,
    filterIntentSource: meta?.filter_intent_source ?? meta?.filterIntentSource,
  });
  const baseCoords = sceneQueryCoords(props, runtimeRef);
  const coords = capability.requiresSceneId
    ? requireSceneQualifiedRequest(baseCoords, "metric query", meta)
    : baseCoords;
  const payload = {
    ...coords,
    preview_scope: runtimePreviewScope(props) || undefined,
    dataset_id: runtimeRef.dataset_id,
    metric_ids: [...ids].sort(),
    search: queryStatePayload.search || undefined,
    filters: queryStatePayload.filters,
    query_state: {
      filters: queryStatePayload.filters,
      ...(safeTrim(queryStatePayload.search) ? { search: queryStatePayload.search } : {}),
      ...(queryStatePayload.group.length > 0 ? { group: queryStatePayload.group } : {}),
      ...(queryStatePayload.timeRange ? { time_range: queryStatePayload.timeRange } : {}),
    },
    filter_intents: queryStatePayload.filterIntents.length > 0 ? queryStatePayload.filterIntents : undefined,
  };
  const errorContext = {
    scene_id: safeTrim(payload.scene_id || props?._mei?.active_scene_id),
    target: safeTrim(payload.target || props?._mei?.active_target_file),
    component: safeTrim(meta?.component),
    panel_id: safeTrim(meta?.panel_id || meta?.panelId),
    request_id: safeTrim(meta?.request_id || meta?.requestId),
  };
  const compileEpoch = runtimeCompileEpoch(props);
  const queryFingerprint = runtimeQueryFingerprint(props);
  rememberHostRuntimeQueryMeta(props);
  maybeInvalidateRuntimeQueryCachesForCompileEpoch(compileEpoch, runtimeDataGeneration(props));
  if (runtimePerfDisabled("runtime_metric_share")) {
    if (shouldPauseHomeRuntimeMetricFetch(props)) {
      return null;
    }
    const managedController = createManagedAbortController([signal]);
    return fetchRuntimeMetricsUncached(
      api,
      payload,
      errorContext,
      managedController.signal,
    ).finally(() => {
      managedController.__meiRelease?.();
    });
  }
  const cacheKey = metricQueryCacheKey(api, payload, queryFingerprint);
  const scopeKey = metricQueryScopeCacheKey(api, payload, queryFingerprint);
  const requestedIds = metricQueryRequestedIds(payload);
  const now = Date.now();
  const cacheConfig = clientQueryCacheConfig(props);
  const appId = String(props?._mei?.app_id || "").trim();
  const dataGen = runtimeDataGeneration(props);
  pruneMetricQueryCaches(now);
  const evalStoreMetric = readEvalStoreMetricData(api, payload, queryFingerprint);
  if (evalStoreMetric) {
    notifyClientRuntimeQueryCacheHit("metric_eval_store");
    return waitForSharedPromise(
      Promise.resolve(withClientResultCachePerf(evalStoreMetric, "metric_eval_store")),
      signal,
    );
  }
  const cached = cacheStore().metricResults.get(cacheKey);
  if (cached && cached.expiresAt > now) {
    notifyClientRuntimeQueryCacheHit("metric");
    return waitForSharedPromise(
      Promise.resolve(withClientResultCachePerf(cached.data, "metric")),
      signal,
    );
  }
  const sessionCached = readSessionRuntimeQueryCache(appId, cacheKey, dataGen, now);
  if (sessionCached?.data) {
    cacheStore().metricResults.set(cacheKey, {
      data: sessionCached.data,
      expiresAt: sessionCached.expiresAt,
    });
    notifyClientRuntimeQueryCacheHit("metric_session");
    return waitForSharedPromise(
      Promise.resolve(withClientResultCachePerf(sessionCached.data, "metric_session")),
      signal,
    );
  }
  const scopeCached = findCoveringMetricScopeResult(scopeKey, requestedIds, now);
  if (scopeCached) {
    notifyClientRuntimeQueryCacheHit("metric_scope");
    return waitForSharedPromise(
      Promise.resolve(
        withMetricScopeSharePerf(
          {
            ...(scopeCached.data && typeof scopeCached.data === "object" ? scopeCached.data : {}),
            metrics: filterMetricsForRequestedIds(scopeCached.data?.metrics, requestedIds),
          },
          { cacheHit: true }
        )
      ),
      signal
    );
  }
  if (
    shouldEnforcePackFirst(props, {
      queryStateId: effectiveQueryStateId,
      search,
      filters,
    })
  ) {
    await awaitBootstrapSeedIfNeeded(props);
    const nowAfterSeed = Date.now();
    const cachedAfterSeed = cacheStore().metricResults.get(cacheKey);
    if (cachedAfterSeed && cachedAfterSeed.expiresAt > nowAfterSeed) {
      notifyClientRuntimeQueryCacheHit("metric_bootstrap");
      return waitForSharedPromise(
        Promise.resolve(withClientResultCachePerf(cachedAfterSeed.data, "metric_bootstrap")),
        signal,
      );
    }
    const sessionAfterSeed = readSessionRuntimeQueryCache(appId, cacheKey, dataGen, nowAfterSeed);
    if (sessionAfterSeed?.data) {
      cacheStore().metricResults.set(cacheKey, {
        data: sessionAfterSeed.data,
        expiresAt: sessionAfterSeed.expiresAt,
      });
      notifyClientRuntimeQueryCacheHit("metric_bootstrap_session");
      return waitForSharedPromise(
        Promise.resolve(withClientResultCachePerf(sessionAfterSeed.data, "metric_bootstrap_session")),
        signal,
      );
    }
    const scopeAfterSeed = findCoveringMetricScopeResult(scopeKey, requestedIds, nowAfterSeed);
    if (scopeAfterSeed) {
      notifyClientRuntimeQueryCacheHit("metric_bootstrap_scope");
      return waitForSharedPromise(
        Promise.resolve(
          withMetricScopeSharePerf(
            {
              ...(scopeAfterSeed.data && typeof scopeAfterSeed.data === "object" ? scopeAfterSeed.data : {}),
              metrics: filterMetricsForRequestedIds(scopeAfterSeed.data?.metrics, requestedIds),
            },
            { cacheHit: true },
          ),
        ),
        signal,
      );
    }
    const bootstrapMetricHit = resolveBootstrapMetricCacheEntry(
      api,
      payload,
      queryFingerprint,
      requestedIds,
      nowAfterSeed,
      props,
    );
    if (bootstrapMetricHit?.cached) {
      if (!bootstrapMetricHit.scope) {
        cacheStore().metricResults.set(cacheKey, bootstrapMetricHit.cached);
      }
      notifyClientRuntimeQueryCacheHit(
        bootstrapMetricHit.scope ? "metric_bootstrap_scope_variant" : "metric_bootstrap_variant",
      );
      window.__meiEvalPackSource = window.__meiEvalPackSource || "bootstrap_seed";
      delete window.__meiEvalPackMissReason;
      if (bootstrapMetricHit.scope) {
        return waitForSharedPromise(
          Promise.resolve(
            withMetricScopeSharePerf(
              {
                ...(bootstrapMetricHit.cached.data && typeof bootstrapMetricHit.cached.data === "object"
                  ? bootstrapMetricHit.cached.data
                  : {}),
                metrics: filterMetricsForRequestedIds(
                  bootstrapMetricHit.cached.data?.metrics,
                  requestedIds,
                ),
              },
              { cacheHit: true },
            ),
          ),
          signal,
        );
      }
      return waitForSharedPromise(
        Promise.resolve(
          withClientResultCachePerf(bootstrapMetricHit.cached.data, "metric_bootstrap_variant"),
        ),
        signal,
      );
    }
    if (bootstrapPackExpected() && bootstrapSeedReady()) {
      if (!bootstrapCoversRequestedMetrics(requestedIds)) {
        window.__meiEvalPackSource = window.__meiEvalPackSource || "bootstrap_partial";
      } else {
        // Seed wrote a nearby key (often without preview_scope) but this exact cache
        // key missed — fall through to network instead of leaving mei-text on `--`.
        window.__meiEvalPackMissReason =
          window.__meiEvalPackMissReason || "metric_cache_miss_after_seed";
        if (typeof window !== "undefined") {
          window.__meiLastMetricCacheMiss = {
            api,
            cacheKey,
            fingerprint: queryFingerprint,
            payload: normalizeMetricQueryCachePayload(payload),
          };
        }
      }
    }
  }
  if (
    !isDefaultExplanatoryQuery(props, {
      queryStateId: effectiveQueryStateId,
      search,
      filters,
    }) &&
    metricBatchQueryCapabilityConfig(props).enabled
  ) {
    try {
      await fetchSceneRuntimeMetricBatch(
        props,
        [
          {
            dataset_id: runtimeRef.dataset_id,
            metric_ids: [...ids].sort(),
          },
        ],
        {
          queryStateId: effectiveQueryStateId,
          search,
          filters,
          signal,
          meta,
        },
      );
      const jitCached = cacheStore().metricResults.get(cacheKey);
      if (jitCached && jitCached.expiresAt > Date.now()) {
        window.__meiEvalPackSource = "jit_metric_batch";
        return waitForSharedPromise(
          Promise.resolve(withClientResultCachePerf(jitCached.data, "metric_jit_batch")),
          signal,
        );
      }
    } catch (_) {
      /* fall through to single metric fetch */
    }
  }
  if (shouldPauseHomeRuntimeMetricFetch(props)) {
    return null;
  }
  let shared = cacheStore().metricInflight.get(cacheKey);
  if (!shared) {
    const scopeInflight = resolveMetricScopeInflight(scopeKey, requestedIds);
    if (scopeInflight) {
      return waitForMetricScopeInflight(scopeInflight, requestedIds, signal);
    }
    if (shouldUseScheduledSceneMetricBatch(meta)) {
      const scheduled = scheduleSceneRuntimeMetricRequest(api, props, ids, {
        queryStateId: effectiveQueryStateId,
        search,
        filters,
        signal,
        meta,
      });
      if (scheduled) {
        return scheduled;
      }
    }
    const managedController = createManagedAbortController();
    const promise = fetchRuntimeMetricsUncached(
      api,
      payload,
      errorContext,
      managedController.signal,
    )
      .then((data) => {
        const expiresAt = Date.now() + cacheConfig.ttlMs;
        cacheStore().metricResults.set(cacheKey, {
          data,
          expiresAt,
        });
        writeSessionRuntimeQueryCache(
          appId,
          cacheKey,
          dataGen,
          "metric",
          data,
          cacheConfig.ttlMs,
          cacheConfig.maxEntries,
        );
        rememberMetricScopeResult(scopeKey, requestedIds, data, expiresAt);
        return data;
      })
      .finally(() => {
        managedController.__meiRelease?.();
        cacheStore().metricInflight.delete(cacheKey);
        unregisterMetricScopeInflight(scopeKey, scopeEntry);
      });
    shared = { promise };
    const scopeEntry = registerMetricScopeInflight(scopeKey, requestedIds, promise);
    cacheStore().metricInflight.set(cacheKey, shared);
  }
  return waitForSharedPromise(shared.promise, signal);
}

export async function fetchPanelRuntimeMetrics(
  element,
  props,
  {
    queryStateId = "",
    search = "",
    filters = {},
    signal = undefined,
    meta = {},
    metricIds: explicitMetricIds = undefined,
  } = {}
) {
  if (!devEvalAllowsRuntimeQuery(props, element)) {
    return null;
  }
  const resolvedQueryStateId = String(queryStateId || queryStateIdOf(props) || "").trim();
  const requestedMetricIds = normalizeExplicitMetricIds(explicitMetricIds);
  if (requestedMetricIds.length > 0) {
    return fetchRuntimeMetrics(props, {
      metricIds: requestedMetricIds,
      queryStateId: resolvedQueryStateId,
      search,
      filters,
      signal,
      meta,
    });
  }
  const batchCapability = metricBatchQueryCapabilityConfig(props);
  if (batchCapability.enabled) {
    const metricIds = collectPanelRuntimeMetricIds(element, props, resolvedQueryStateId);
    return fetchRuntimeMetrics(props, {
      metricIds,
      queryStateId: resolvedQueryStateId,
      search,
      filters,
      signal,
      meta,
    });
  }
  const panel = resolveMetricBatchPanel(element, props, resolvedQueryStateId);
  if (!(panel instanceof Element)) {
    const metricIds = collectPanelRuntimeMetricIds(element, props, resolvedQueryStateId);
    return fetchRuntimeMetrics(props, {
      metricIds,
      queryStateId: resolvedQueryStateId,
      search,
      filters,
      signal,
      meta,
    });
  }
  return waitForSharedPromise(
    schedulePanelMetricBatch(panel, element, props, {
      queryStateId: resolvedQueryStateId,
      search,
      filters,
      meta,
    }),
    signal,
  );
}

function runtimePerfMetricLevel(ms) {
  if (!Number.isFinite(ms) || ms < 0) return "ok";
  if (ms >= 1500) return "bad";
  if (ms >= 500) return "warn";
  return "ok";
}

function runtimePerfMetricLabel(level) {
  if (level === "bad") return "SLOW";
  if (level === "warn") return "WARN";
  return "OK";
}

function resolveRuntimePerfHost() {
  try {
    if (window.parent && window.parent !== window) {
      const el = window.parent.document.getElementById("runtime-perf-diagnostics");
      if (el) return el;
    }
  } catch (_) {
    /* 父文档跨域或不可访问时忽略 */
  }
  return document.getElementById("runtime-perf-diagnostics");
}

function runtimePerfLineStyle(level) {
  const base =
    "display:block;margin:4px 0;padding:4px 6px;border-radius:6px;border:1px solid transparent;font-size:11px;line-height:18px;";
  if (level === "bad") {
    return (
      base +
      "color:#fecaca;background:rgba(127,29,29,.2);border-color:rgba(252,165,165,.35);"
    );
  }
  if (level === "warn") {
    return (
      base +
      "color:#fde68a;background:rgba(120,53,15,.2);border-color:rgba(252,211,77,.35);"
    );
  }
  return (
    base +
    "color:#bae6fd;background:rgba(15,23,42,.2);border-color:rgba(148,163,184,.3);"
  );
}

function runtimePerfChipStyle(level) {
  const base =
    "display:inline-block;margin-left:6px;padding:0 6px;border-radius:999px;border:1px solid transparent;font-size:10px;line-height:16px;";
  if (level === "bad") {
    return (
      base +
      "color:#fecaca;background:rgba(127,29,29,.28);border-color:rgba(252,165,165,.45);"
    );
  }
  if (level === "warn") {
    return (
      base +
      "color:#fde68a;background:rgba(120,53,15,.28);border-color:rgba(252,211,77,.45);"
    );
  }
  return (
    base +
    "color:#86efac;background:rgba(22,101,52,.22);border-color:rgba(134,239,172,.35);"
  );
}

function renderRuntimePerfHost() {
  const host = resolveRuntimePerfHost();
  if (!host) return;
  const history = Array.isArray(window.__meiRuntimePerfHistory)
    ? window.__meiRuntimePerfHistory
    : [];
  if (history.length === 0) {
    host.textContent = "尚无懒加载查询记录。";
    return;
  }
  host.innerHTML = history
    .map((entry) => {
      const chips = entry.items
        .map(
          (item) =>
            `<span style="${runtimePerfChipStyle(item.level)}">${escapeHtml(item.text)}</span>`
        )
        .join("");
      const ctx = entry.context
        ? `<span style="color:#94a3b8;font-size:10px;"> ${escapeHtml(entry.context)}</span>`
        : "";
      return `<div style="${runtimePerfLineStyle(entry.level)}">[${escapeHtml(
        entry.time
      )}] dataset=${escapeHtml(entry.datasetId)}${ctx} ${chips}</div>`;
    })
    .join("");
}

/** SPA 换文件时清空历史，避免其它页的慢查询误导当前页诊断。 */
export function clearRuntimePerfDiagnostics(reason = "") {
  window.__meiRuntimePerfHistory = [];
  const host = resolveRuntimePerfHost();
  if (host) {
    const hint = reason ? `（已清空：${reason}）` : "";
    host.textContent = `尚无懒加载查询记录。${hint}`;
  }
}

export function appendRuntimePerfDiagnostics(datasetId, perf, meta = {}) {
  if (!perf || typeof perf !== "object") return;
  const host = resolveRuntimePerfHost();
  if (!host) return;
  const now = new Date();
  const time = now.toLocaleTimeString("zh-CN", { hour12: false });
  const perfKeys = [
    "compile_cache_lock_wait_ms",
    "compile_cache_lookup_ms",
    "compile_ms",
    "file_cache_lookup_ms",
    "file_cache_load_ms",
    "file_cache_paginate_ms",
    "locate_dataset_ms",
    "query_api_ms",
    "query_total_ms",
    "server_handler_total_ms",
    "total_ms",
    "client_ttfb_ms",
    "client_json_ms",
    "client_total_ms",
    "client_fetch_parse_ms",
    "client_outside_server_ms",
  ];
  const hasHandlerTotal = Number.isFinite(Number(perf.server_handler_total_ms));
  const items = perfKeys
    .map((key) => {
      if (key === "total_ms" && hasHandlerTotal) return null;
      const value = Number(perf[key]);
      if (!Number.isFinite(value) || value < 0) return null;
      const level = runtimePerfMetricLevel(value);
      return {
        text: `${key}=${value}ms (${runtimePerfMetricLabel(level)})`,
        level,
      };
    })
    .filter(Boolean);
  const cacheHit = Number(perf.compile_cache_hit);
  if (Number.isFinite(cacheHit)) {
    const hit = cacheHit >= 1;
    items.unshift({
      text: `compile_cache_hit=${hit ? "1(HIT)" : "0(MISS)"}`,
      level: hit ? "ok" : "warn",
    });
  }
  const fileCacheHit = Number(perf.file_cache_hit);
  if (Number.isFinite(fileCacheHit)) {
    const hit = fileCacheHit >= 1;
    items.unshift({
      text: `file_cache_hit=${hit ? "1(HIT)" : "0(MISS)"}`,
      level: hit ? "ok" : "warn",
    });
  }
  const evictCount = Number(perf.file_cache_evict_count);
  if (Number.isFinite(evictCount)) {
    items.unshift({
      text: `file_cache_evict_count=${evictCount}`,
      level: evictCount > 0 ? "warn" : "ok",
    });
  }
  if (items.length === 0) return;
  const lineLevel = items.some((item) => item.level === "bad")
    ? "bad"
    : items.some((item) => item.level === "warn")
    ? "warn"
    : "ok";
  const contextParts = [];
  if (meta.component) contextParts.push(String(meta.component));
  if (meta.scene_id) contextParts.push(`scene=${meta.scene_id}`);
  if (meta.target) contextParts.push(`file=${meta.target}`);
  if (meta.panel_id) contextParts.push(`panel=${meta.panel_id}`);
  if (meta.query_state_id) contextParts.push(`query_state=${meta.query_state_id}`);
  const requestId = safeTrim(meta.request_id || perf.request_id);
  if (requestId) contextParts.push(`req=${requestId}`);
  if (meta.aborted) contextParts.push("aborted");
  const line = {
    time,
    datasetId: String(datasetId || ""),
    level: lineLevel,
    items,
    context: contextParts.join(" · "),
  };
  const history = Array.isArray(window.__meiRuntimePerfHistory)
    ? window.__meiRuntimePerfHistory
    : [];
  history.unshift(line);
  window.__meiRuntimePerfHistory = history.slice(0, 20);
  renderRuntimePerfHost();
}

if (typeof window !== "undefined") {
  window.__meiClearRuntimePerfDiagnostics = clearRuntimePerfDiagnostics;
  window.__meiDatasetRuntime = Object.assign(window.__meiDatasetRuntime || {}, {
    fetchDatasetRows,
    fetchPanelRuntimeMetrics,
    prefetchVisiblePanelMetrics,
    syncRuntimeQueryAppContextFromPage,
    mergeFilters,
    resolveDatasetQueryCapability,
    getQueryState,
    setQueryState,
    setQueryStateFilter,
    sharedFiltersForQueryStateId,
    sharedFilterIntentsForQueryStateId,
    sharedSearchForQueryStateId,
    resolveRuntimeDataRef,
    resolveRuntimeMetricRef,
    isSeedableBootstrapPack,
    findRuntimeMetricInResults,
    isYearMonthMatrixMetricConfig,
  });
}

function runtimeCandidates(props) {
  return [
    props?.content,
    props?.value,
    props?.data,
    props?.metric,
    props?.totalMetric,
    props?.total_metric,
    props?.numerMetric,
    props?.numer_metric,
    props?.noViolMetric,
    props?.no_viol_metric,
    props?.dataset?.dataset,
    props?.dataset,
  ].filter(Boolean);
}

export const MEI_HOME_RUNTIME_RESUME = "meilang:home-runtime-resume";

let homeRuntimeResumeObserver = null;
let homeRuntimeResumeOverlayOpen = false;

function ensureHomeRuntimeResumeObserver() {
  if (homeRuntimeResumeObserver || typeof document === "undefined" || !document.body) {
    return;
  }
  homeRuntimeResumeOverlayOpen = isDrilldownOverlayOpen();
  homeRuntimeResumeObserver = new MutationObserver(() => {
    const nowOpen = isDrilldownOverlayOpen();
    if (homeRuntimeResumeOverlayOpen && !nowOpen) {
      window.dispatchEvent(new CustomEvent(MEI_HOME_RUNTIME_RESUME));
      scheduleSceneMetricBatchFlush(0);
    }
    homeRuntimeResumeOverlayOpen = nowOpen;
  });
  homeRuntimeResumeObserver.observe(document.body, {
    attributes: true,
    attributeFilter: ["class"],
  });
}

/** overlay 关闭后通知主屏组件补拉 metric（关闭时不发 page 级 preview-updated）。 */
export function subscribeHomeRuntimeResume(fn) {
  if (typeof window === "undefined" || typeof fn !== "function") {
    return () => {};
  }
  ensureHomeRuntimeResumeObserver();
  window.addEventListener(MEI_HOME_RUNTIME_RESUME, fn);
  return () => window.removeEventListener(MEI_HOME_RUNTIME_RESUME, fn);
}

function ensureStore() {
  if (!window[STORE_KEY] || typeof window[STORE_KEY] !== "object") {
    window[STORE_KEY] = {};
  }
  return window[STORE_KEY];
}

function normalizeQueryState(raw) {
  const source = raw && typeof raw === "object" ? raw : {};
  const filters = mergeFilters(source.filters);
  const search = String(source.search ?? source.keyword ?? "").trim();
  const group = normalizeQueryGroup(source.group ?? source.groups ?? source.group_by ?? source.groupBy);
  const time_range = normalizeQueryTimeRange(source.time_range ?? source.timeRange);
  const filterIntentSource = normalizeFilterIntentSource(
    source.filter_intent_source ?? source.filterIntentSource,
    "query_state"
  );
  const sort = Array.isArray(source.sort)
    ? source.sort
        .map((item) => ({
          field: String(item?.field || "").trim(),
          direction: String(item?.direction || "asc").trim().toLowerCase() || "asc",
        }))
        .filter((item) => item.field)
    : [];
  const rawColumnState = source.column_state ?? source.columnState ?? null;
  const parsedColumnState =
    typeof rawColumnState === "string"
      ? (() => {
          try {
            return JSON.parse(rawColumnState);
          } catch (_) {
            return null;
          }
        })()
      : rawColumnState;
  const column_state = {
    columns: Array.isArray(parsedColumnState?.columns)
      ? parsedColumnState.columns
          .map((entry, index) => {
            const key = String(entry?.key || entry?.field || entry?.name || "").trim();
            if (!key) return null;
            const order = Number(entry?.order ?? index);
            return {
              key,
              hidden: entry?.hidden === true || entry?.hidden === "true",
              order: Number.isFinite(order) ? Math.round(order) : index,
              width: Number.isFinite(Number(entry?.width)) && Number(entry?.width) > 0 ? Math.round(Number(entry?.width)) : null,
              min_width:
                Number.isFinite(Number(entry?.min_width ?? entry?.minWidth)) &&
                Number(entry?.min_width ?? entry?.minWidth) > 0
                  ? Math.round(Number(entry?.min_width ?? entry?.minWidth))
                  : null,
              max_width:
                Number.isFinite(Number(entry?.max_width ?? entry?.maxWidth)) &&
                Number(entry?.max_width ?? entry?.maxWidth) > 0
                  ? Math.round(Number(entry?.max_width ?? entry?.maxWidth))
                  : null,
              align: ["left", "center", "right", "justify"].includes(
                String(entry?.align || "").trim().toLowerCase()
              )
                ? String(entry?.align || "").trim().toLowerCase()
                : null,
              valign: ["top", "middle", "bottom"].includes(
                String(entry?.valign || entry?.verticalAlign || "").trim().toLowerCase()
              )
                ? String(entry?.valign || entry?.verticalAlign || "").trim().toLowerCase()
                : null,
              header_align: ["left", "center", "right", "justify"].includes(
                String(entry?.header_align || entry?.headerAlign || "").trim().toLowerCase()
              )
                ? String(entry?.header_align || entry?.headerAlign || "").trim().toLowerCase()
                : null,
              header_valign: ["top", "middle", "bottom"].includes(
                String(entry?.header_valign || entry?.headerValign || "").trim().toLowerCase()
              )
                ? String(entry?.header_valign || entry?.headerValign || "").trim().toLowerCase()
                : null,
              wrap:
                entry?.wrap === true || entry?.wrap === false
                  ? entry.wrap
                  : String(entry?.wrap || "").trim().toLowerCase() === "true"
                    ? true
                    : String(entry?.wrap || "").trim().toLowerCase() === "false"
                      ? false
                      : null,
              header_wrap:
                entry?.header_wrap === true || entry?.header_wrap === false
                  ? entry.header_wrap
                  : String(entry?.header_wrap ?? entry?.headerWrap ?? "").trim().toLowerCase() === "true"
                    ? true
                    : String(entry?.header_wrap ?? entry?.headerWrap ?? "").trim().toLowerCase() === "false"
                      ? false
                      : null,
            };
          })
          .filter(Boolean)
      : [],
  };
  const rawFilterIntents = Array.isArray(source.filter_intents)
    ? source.filter_intents
    : Array.isArray(source.filterIntents)
      ? source.filterIntents
      : [];
  const filter_intents = rawFilterIntents.length > 0
    ? rawFilterIntents
        .map((entry) => normalizeFilterIntent(entry))
        .filter(Boolean)
    : filterIntentsFromFilters(filters, filterIntentSource);
  const last_transition = normalizeQueryTransition(
    source.last_transition ?? source.lastTransition ?? {
      source: source.transition_source ?? source.transitionSource ?? source.filter_intent_source ?? source.filterIntentSource,
    }
  );
  return {
    filters,
    search,
    group,
    time_range,
    sort,
    column_state,
    filter_intents,
    last_transition,
  };
}

function normalizePositiveInt(value, fallback = 0, { min = 0 } = {}) {
  const num = Number(value);
  if (!Number.isFinite(num)) return fallback;
  return Math.max(min, Math.round(num));
}

function normalizeColumnStateForRequest(rawColumnState) {
  const parsedColumnState =
    typeof rawColumnState === "string"
      ? (() => {
          try {
            return JSON.parse(rawColumnState);
          } catch (_) {
            return null;
          }
        })()
      : rawColumnState;
  if (!parsedColumnState || typeof parsedColumnState !== "object") {
    return undefined;
  }
  const columns = Array.isArray(parsedColumnState?.columns)
    ? parsedColumnState.columns
    : Array.isArray(parsedColumnState)
      ? parsedColumnState
      : [];
  const normalizedColumns = columns
    .map((entry, index) => {
      const key = String(entry?.key || entry?.field || entry?.name || "").trim();
      if (!key) return null;
      const order = normalizePositiveInt(entry?.order, index, { min: 0 });
      const width = normalizePositiveInt(entry?.width, 0, { min: 0 });
      const minWidth = normalizePositiveInt(entry?.min_width ?? entry?.minWidth, 0, { min: 0 });
      const maxWidth = normalizePositiveInt(entry?.max_width ?? entry?.maxWidth, 0, { min: 0 });
      const align = String(entry?.align || "").trim().toLowerCase();
      const valign = String(entry?.valign || entry?.verticalAlign || "").trim().toLowerCase();
      const headerAlign = String(entry?.header_align || entry?.headerAlign || "").trim().toLowerCase();
      const headerValign = String(entry?.header_valign || entry?.headerValign || "").trim().toLowerCase();
      const wrapRaw = String(entry?.wrap ?? "").trim().toLowerCase();
      const headerWrapRaw = String(entry?.header_wrap ?? entry?.headerWrap ?? "").trim().toLowerCase();
      return {
        key,
        hidden: entry?.hidden === true || entry?.hidden === "true",
        order,
        width: width > 0 ? width : null,
        min_width: minWidth > 0 ? minWidth : null,
        max_width: maxWidth > 0 ? maxWidth : null,
        align: ["left", "center", "right", "justify"].includes(align) ? align : null,
        valign: ["top", "middle", "bottom"].includes(valign) ? valign : null,
        header_align: ["left", "center", "right", "justify"].includes(headerAlign)
          ? headerAlign
          : null,
        header_valign: ["top", "middle", "bottom"].includes(headerValign)
          ? headerValign
          : null,
        wrap:
          entry?.wrap === true || entry?.wrap === false
            ? entry.wrap
            : wrapRaw === "true"
              ? true
              : wrapRaw === "false"
                ? false
                : null,
        header_wrap:
          entry?.header_wrap === true || entry?.header_wrap === false
            ? entry.header_wrap
            : entry?.headerWrap === true || entry?.headerWrap === false
              ? entry.headerWrap
              : headerWrapRaw === "true"
                ? true
                : headerWrapRaw === "false"
                  ? false
                  : null,
      };
    })
    .filter(Boolean);
  return normalizedColumns.length > 0 ? { columns: normalizedColumns } : undefined;
}

function normalizeFilterIntentSource(value, fallback = "unknown") {
  const normalized = String(value || "")
    .trim()
    .toLowerCase()
    .replaceAll(/[\s-]+/g, "_");
  if (
    [
      "query_state",
      "filter_bar",
      "metric_click",
      "chart_selection",
      "table_selection",
      "drilldown",
      "unknown",
    ].includes(normalized)
  ) {
    return normalized;
  }
  return String(fallback || "unknown").trim() || "unknown";
}

function normalizeFilterIntent(entry) {
  if (!entry || typeof entry !== "object" || Array.isArray(entry)) return null;
  const dimension = String(entry.dimension || entry.field || "").trim();
  const value = String(entry.value ?? "").trim();
  if (!dimension || !value) return null;
  return {
    dimension,
    operator: String(entry.operator || "eq").trim().toLowerCase() || "eq",
    value,
    source: normalizeFilterIntentSource(entry.source, "unknown"),
  };
}

function filterIntentsFromFilters(filters = {}, fallbackSource = "query_state", sourceByDimension = null) {
  const out = [];
  for (const [key, value] of Object.entries(filters || {})) {
    const dimension = String(key || "").trim();
    const normalizedValue = String(value ?? "").trim();
    if (!dimension || !normalizedValue) continue;
    out.push({
      dimension,
      operator: "eq",
      value: normalizedValue,
      source: normalizeFilterIntentSource(sourceByDimension?.get(dimension), fallbackSource),
    });
  }
  return out;
}

function filterIntentSourceMap(intents = []) {
  const map = new Map();
  for (const entry of intents) {
    const normalized = normalizeFilterIntent(entry);
    if (!normalized) continue;
    map.set(normalized.dimension, normalized.source);
  }
  return map;
}

function mergedQueryStatePayload(queryStateId = "", filters = {}, options = {}) {
  const sharedState = queryStateId ? getQueryState(queryStateId) : normalizeQueryState({});
  const mergedFilters = mergeFilters(sharedState.filters, filters);
  const mergedSearch = String(options?.search || sharedState.search || "").trim();
  const mergedGroup = normalizeQueryGroup(options?.group ?? sharedState.group);
  const mergedTimeRange = normalizeQueryTimeRange(options?.timeRange ?? options?.time_range ?? sharedState.time_range);
  const sourceByDimension = filterIntentSourceMap(sharedState.filter_intents);
  const extraSource = normalizeFilterIntentSource(
    options?.filterIntentSource ?? options?.filter_intent_source,
    "unknown"
  );
  for (const [key, value] of Object.entries(filters || {})) {
    const dimension = String(key || "").trim();
    const normalizedValue = String(value ?? "").trim();
    if (!dimension || !normalizedValue) continue;
    sourceByDimension.set(dimension, extraSource);
  }
  return {
    filters: mergedFilters,
    search: mergedSearch,
    group: mergedGroup,
    timeRange: mergedTimeRange,
    filterIntents: filterIntentsFromFilters(mergedFilters, "query_state", sourceByDimension),
  };
}

function normalizeQueryTransition(raw) {
  if (!raw || typeof raw !== "object" || Array.isArray(raw)) return null;
  const source = normalizeFilterIntentSource(raw.source, "");
  if (!source) return null;
  const at = Number(raw.at);
  return {
    source,
    at: Number.isFinite(at) && at > 0 ? Math.round(at) : Date.now(),
  };
}

function normalizeQueryGroup(raw) {
  const source = Array.isArray(raw) ? raw : [];
  const values = source
    .map((entry) => String(entry || "").trim())
    .filter(Boolean);
  return [...new Set(values)];
}

function normalizeQueryTimeRange(raw) {
  if (!raw || typeof raw !== "object" || Array.isArray(raw)) return null;
  const dimension = String(raw.dimension ?? raw.field ?? "").trim();
  const start = String(raw.start ?? raw.from ?? "").trim();
  const end = String(raw.end ?? raw.to ?? "").trim();
  const preset = String(raw.preset ?? "").trim();
  if (!dimension && !start && !end && !preset) return null;
  return {
    dimension: dimension || null,
    start: start || null,
    end: end || null,
    preset: preset || null,
  };
}
