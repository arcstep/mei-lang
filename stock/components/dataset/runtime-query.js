const STORE_KEY = "__meiQueryStateStore";
const EVENT_NAME = "mei:query-state-change";
const METRIC_QUERY_INFLIGHT = new Map();
const METRIC_QUERY_RESULT_CACHE = new Map();
const METRIC_QUERY_SCOPE_INFLIGHT = new Map();
const METRIC_QUERY_SCOPE_RESULT_CACHE = new Map();
const METRIC_QUERY_CACHE_TTL_MS = 300_000;
const DATASET_QUERY_INFLIGHT = new Map();
const DATASET_QUERY_RESULT_CACHE = new Map();
const DATASET_QUERY_CACHE_TTL_MS = 300_000;
const SCENE_METRIC_BATCH_INFLIGHT = new Map();
const SCENE_METRIC_BATCH_SCHEDULES = new Map();
const SCENE_METRIC_BATCH_FLUSH_DELAY_MS = 32;
let sceneMetricBatchFlushTimer = null;
const ACTIVE_RUNTIME_FETCH_CONTROLLERS = new Set();
const PARSED_DATA_PROPS_CACHE = new WeakMap();
export const MEI_DRILLDOWN_OVERLAY_ID = "mei-access-drilldown-overlay";
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
    scene_id: safeTrim(props?._mei?.active_scene_id) || undefined,
    target:
      safeTrim(props?._mei?.active_target_file || props?._mei?.entry_target) ||
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

/** scene-first 寻址：优先 runtime ref，其次 SSR 注入的 active_scene_id */
function sceneQueryCoords(props, runtimeRef) {
  const sceneId = String(
    runtimeRef?.scene_id ?? props?._mei?.active_scene_id ?? ""
  ).trim();
  const scenePath = String(
    runtimeRef?.scene_path ?? props?._mei?.active_target_file ?? ""
  ).trim();
  const target = datasetCompileTarget(props);
  const coords = {};
  if (sceneId) coords.scene_id = sceneId;
  if (target) coords.target = target;
  if (scenePath && !coords.target) coords.target = scenePath;
  return coords;
}

function runtimeCapabilityMap(props) {
  const raw = props?._mei?.runtime_capabilities ?? props?._mei?.runtimeCapabilities;
  if (!raw || typeof raw !== "object" || Array.isArray(raw)) {
    return {};
  }
  return raw;
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

/** 与 `mei-lang/app/assets/manage-tabs.js` 发出的标签切换事件一致 */
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
  return document.body?.classList?.contains("access-drilldown-open") === true;
}

/**
 * 二级看板 overlay 打开时，主屏组件不应响应 preview-updated；overlay 内组件仍应刷新。
 */
export function shouldReactToPreviewUpdated(event, element) {
  const scope = previewUpdatedScope(event);
  const inOverlay =
    element instanceof Element &&
    Boolean(element.closest(`#${MEI_DRILLDOWN_OVERLAY_ID}`));
  if (scope === "drilldown") {
    return inOverlay;
  }
  if (isDrilldownOverlayOpen() && !inOverlay) {
    return false;
  }
  return true;
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
  let fallbackTimer = null;

  const cleanupWatchers = () => {
    document.removeEventListener(MEI_MANAGE_TAB_CHANGE, onManageTab);
    window.removeEventListener("pageshow", onPageShow);
    window.removeEventListener("meilang:preview-updated", onPreviewUpdated);
    if (io) {
      try {
        io.disconnect();
      } catch (_) {
        /* ignore */
      }
      io = null;
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

export function resolveRuntimeDataRef(props) {
  for (const candidate of runtimeCandidates(props)) {
    const ref = candidate?.__mei_runtime_ref;
    if (ref && ref.kind === "data" && ref.dataset_id) {
      return ref;
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
      void schedule.flush();
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

export function abortRuntimeQueries(reason = "") {
  abortPendingPanelMetricBatches();
  abortPendingSceneMetricBatchSchedules();
  for (const controller of [...ACTIVE_RUNTIME_FETCH_CONTROLLERS]) {
    try {
      controller.abort(sharedAbortError());
    } catch (_) {
      /* ignore */
    }
  }
  clearRuntimeQueryCaches();
  if (typeof window !== "undefined") {
    window.__meiLastRuntimeAbortReason = String(reason || "").trim();
  }
}

export function collectPanelRuntimeMetricIdsFromPanel(panel, anchorProps, queryStateId = "") {
  const runtimeRef = resolveRuntimeMetricRef(anchorProps);
  const metricId = safeTrim(runtimeRef?.metric_id);
  if (!runtimeRef?.dataset_id) {
    return metricId ? [metricId] : [];
  }
  if (!(panel instanceof Element)) {
    return metricId ? [metricId] : [];
  }
  const currentQueryStateId = String(queryStateId || queryStateIdOf(anchorProps) || "").trim();
  const currentCoords = sceneQueryCoords(anchorProps, runtimeRef);
  const ids = new Set();
  if (metricId) {
    ids.add(metricId);
  }
  panel.querySelectorAll("[data-props]").forEach((node) => {
    const candidateProps = parseProps(node);
    const candidateRef = resolveRuntimeMetricRef(candidateProps);
    if (!candidateRef?.dataset_id || !candidateRef?.metric_id) return;
    if (safeTrim(candidateRef.dataset_id) !== safeTrim(runtimeRef.dataset_id)) return;
    const candidateQueryStateId = String(queryStateIdOf(candidateProps) || "").trim();
    if (currentQueryStateId && candidateQueryStateId && candidateQueryStateId !== currentQueryStateId) {
      return;
    }
    const candidateCoords = sceneQueryCoords(candidateProps, candidateRef);
    if (!sameSceneQueryCoords(currentCoords, candidateCoords)) return;
    ids.add(String(candidateRef.metric_id).trim());
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
    batch.promise = new Promise((resolve, reject) => {
      batch.resolve = resolve;
      batch.reject = reject;
    });
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
    void batch.flush();
  }, { aggressive: true });

  return batch.promise;
}

export function prefetchPanelRuntimeMetrics(panel, anchor, props, options = {}) {
  if (!(panel instanceof Element) || !(anchor instanceof Element)) {
    return Promise.resolve(null);
  }
  if (!elementIsDisplayed(panel) || isDrilldownOverlayOpen()) {
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
  if (typeof document === "undefined" || isDrilldownOverlayOpen()) {
    return;
  }
  const scopeRoot = root && root.querySelectorAll ? root : document;
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
      if (!(node instanceof Element) || !node.isConnected) {
        return;
      }
      const props = parseProps(node);
      const runtimeRef = resolveRuntimeMetricRef(props);
      if (!runtimeRef?.dataset_id || !runtimeRef?.metric_id) {
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
      const datasetId = safeTrim(runtimeRef.dataset_id);
      if (!datasetId) {
        return;
      }
      let entry = group.entries.get(datasetId);
      if (!entry) {
        entry = { datasetId, metricIds: new Set(), props };
        group.entries.set(datasetId, entry);
      }
      entry.metricIds.add(String(runtimeRef.metric_id).trim());
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
  if (typeof document === "undefined" || isDrilldownOverlayOpen()) {
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
    if (!(panel instanceof Element) || !elementIsDisplayed(panel)) {
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

if (typeof window !== "undefined") {
  window.addEventListener(MEI_ABORT_RUNTIME_QUERIES, (event) => {
    abortRuntimeQueries(event?.detail?.reason || "");
  });
  window.addEventListener("meilang:preview-updated", (event) => {
    if (previewUpdatedScope(event) === "page") {
      scheduleSceneMetricBatchFlush(0);
    }
    if (event?.detail?.resetRuntimeQueryCache === false) {
      return;
    }
    clearRuntimeQueryCaches();
  });
  window.addEventListener("pagehide", () => {
    abortRuntimeQueries("pagehide");
  });
  window.addEventListener(MEI_PREFETCH_PANEL_METRICS, () => {
    prefetchVisiblePanelMetrics();
  });
  window.dispatchEvent(
    new CustomEvent(MEI_RUNTIME_QUERY_READY, {
      detail: { source: "runtime-query" },
    }),
  );
  window.__meiAbortRuntimeQueries = abortRuntimeQueries;
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
  if (!host) return;
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

function isAbortError(error) {
  if (!error) return false;
  if (error.name === "AbortError") return true;
  const msg = String(error.message || error || "");
  return msg.includes("aborted") || msg.includes("AbortError");
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
  for (const [key, entry] of METRIC_QUERY_RESULT_CACHE.entries()) {
    if (!entry || !Number.isFinite(entry.expiresAt) || entry.expiresAt <= now) {
      METRIC_QUERY_RESULT_CACHE.delete(key);
    }
  }
  for (const [key, entries] of METRIC_QUERY_SCOPE_RESULT_CACHE.entries()) {
    const next = (Array.isArray(entries) ? entries : []).filter(
      (entry) => entry && Number.isFinite(entry.expiresAt) && entry.expiresAt > now
    );
    if (next.length > 0) {
      METRIC_QUERY_SCOPE_RESULT_CACHE.set(key, next);
    } else {
      METRIC_QUERY_SCOPE_RESULT_CACHE.delete(key);
    }
  }
}

function pruneDatasetQueryCaches(now = Date.now()) {
  for (const [key, entry] of DATASET_QUERY_RESULT_CACHE.entries()) {
    if (!entry || !Number.isFinite(entry.expiresAt) || entry.expiresAt <= now) {
      DATASET_QUERY_RESULT_CACHE.delete(key);
    }
  }
}

function runtimeCompileEpoch(props) {
  return String(props?._mei?.compile_epoch || "").trim();
}

function maybeInvalidateRuntimeQueryCachesForCompileEpoch(compileEpoch) {
  const next = String(compileEpoch || "").trim();
  if (!next || typeof window === "undefined") {
    return;
  }
  const last = String(window.__meiLastCompileEpoch || "").trim();
  if (last && last !== next) {
    clearRuntimeQueryCaches();
  }
  window.__meiLastCompileEpoch = next;
}

function datasetQueryCacheKey(api, payload, compileEpoch = "") {
  const epoch = String(compileEpoch || "").trim();
  return `dataset|${String(api || "").trim()}|${epoch}|${stableSerialize(payload)}`;
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

function metricQueryCacheKey(api, payload, compileEpoch = "") {
  const epoch = String(compileEpoch || "").trim();
  return `${String(api || "").trim()}|${epoch}|${stableSerialize(payload)}`;
}

function metricQueryScopeCacheKey(api, payload, compileEpoch = "") {
  const scopePayload = payload && typeof payload === "object" ? { ...payload } : {};
  delete scopePayload.metric_ids;
  const epoch = String(compileEpoch || "").trim();
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
  const entries = METRIC_QUERY_SCOPE_RESULT_CACHE.get(scopeKey);
  if (!Array.isArray(entries) || entries.length === 0) {
    return null;
  }
  const active = entries.filter(
    (entry) => entry && Number.isFinite(entry.expiresAt) && entry.expiresAt > now
  );
  if (active.length !== entries.length) {
    if (active.length > 0) {
      METRIC_QUERY_SCOPE_RESULT_CACHE.set(scopeKey, active);
    } else {
      METRIC_QUERY_SCOPE_RESULT_CACHE.delete(scopeKey);
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
  const existing = Array.isArray(METRIC_QUERY_SCOPE_RESULT_CACHE.get(scopeKey))
    ? METRIC_QUERY_SCOPE_RESULT_CACHE.get(scopeKey)
    : [];
  const filtered = existing.filter((entry) => !metricQueryScopeEntryCovers(entry, requestedIds));
  filtered.unshift(nextEntry);
  METRIC_QUERY_SCOPE_RESULT_CACHE.set(scopeKey, filtered.slice(0, 8));
}

function findCoveringMetricScopeInflight(scopeKey, requestedIds) {
  const entries = METRIC_QUERY_SCOPE_INFLIGHT.get(scopeKey);
  if (!Array.isArray(entries) || entries.length === 0) {
    return null;
  }
  return entries.find((entry) => metricQueryScopeEntryCovers(entry, requestedIds)) || null;
}

function findAnyMetricScopeInflight(scopeKey) {
  const entries = METRIC_QUERY_SCOPE_INFLIGHT.get(scopeKey);
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
  const entries = Array.isArray(METRIC_QUERY_SCOPE_INFLIGHT.get(scopeKey))
    ? METRIC_QUERY_SCOPE_INFLIGHT.get(scopeKey)
    : [];
  entries.push(entry);
  METRIC_QUERY_SCOPE_INFLIGHT.set(scopeKey, entries);
  return entry;
}

function unregisterMetricScopeInflight(scopeKey, entry) {
  const entries = Array.isArray(METRIC_QUERY_SCOPE_INFLIGHT.get(scopeKey))
    ? METRIC_QUERY_SCOPE_INFLIGHT.get(scopeKey)
    : [];
  const next = entries.filter((candidate) => candidate !== entry);
  if (next.length > 0) {
    METRIC_QUERY_SCOPE_INFLIGHT.set(scopeKey, next);
  } else {
    METRIC_QUERY_SCOPE_INFLIGHT.delete(scopeKey);
  }
}

function clearRuntimeQueryCaches() {
  METRIC_QUERY_INFLIGHT.clear();
  METRIC_QUERY_RESULT_CACHE.clear();
  METRIC_QUERY_SCOPE_INFLIGHT.clear();
  METRIC_QUERY_SCOPE_RESULT_CACHE.clear();
  SCENE_METRIC_BATCH_INFLIGHT.clear();
  SCENE_METRIC_BATCH_SCHEDULES.clear();
  DATASET_QUERY_INFLIGHT.clear();
  DATASET_QUERY_RESULT_CACHE.clear();
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
  return {
    ...data,
    perf: {
      ...(data.perf && typeof data.perf === "object" ? data.perf : {}),
      client_metric_scope_cache_hit: cacheHit ? 1 : 0,
      client_metric_scope_inflight_hit: inflightHit ? 1 : 0,
    },
  };
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
  const entries = METRIC_QUERY_SCOPE_INFLIGHT.get(scopeKey);
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
    ({ response, data, clientPerf, errorText } = await fetchJsonWithClientPerf(api, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(payload),
      signal,
    }));
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
    throw new Error(text);
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
  METRIC_QUERY_INFLIGHT.set(cacheKey, { promise });
  return {
    datasetId: safeTrim(group?.dataset_id),
    cacheKey,
    scopeKey,
    requestedIds,
    resolve: resolvePromise,
    reject: rejectPromise,
    cleanup() {
      METRIC_QUERY_INFLIGHT.delete(cacheKey);
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
    ({ response, data, clientPerf, errorText } = await fetchJsonWithClientPerf(api, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(payload),
      signal,
    }));
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
    throw new Error(text);
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
  const capability = metricBatchQueryCapabilityConfig(props);
  const api = capability.api;
  const normalizedGroups = normalizeSceneMetricBatchGroups(groups);
  if (!capability.enabled || normalizedGroups.length <= 1) {
    return null;
  }
  const runtimeRef = resolveRuntimeMetricRef(props);
  if (!runtimeRef?.dataset_id) {
    return null;
  }
  const effectiveQueryStateId = String(queryStateId || queryStateIdOf(props) || "").trim();
  const compileEpoch = runtimeCompileEpoch(props);
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
    search: queryStatePayload.search || undefined,
    filters: queryStatePayload.filters,
    query_state: {
      filters: queryStatePayload.filters,
      search: queryStatePayload.search || undefined,
      group: queryStatePayload.group.length > 0 ? queryStatePayload.group : undefined,
      time_range: queryStatePayload.timeRange || undefined,
    },
    filter_intents:
      queryStatePayload.filterIntents.length > 0 ? queryStatePayload.filterIntents : undefined,
  };
  const now = Date.now();
  pruneMetricQueryCaches(now);
  const pendingGroups = normalizedGroups.filter((group) => {
    const singlePayload = singleMetricPayloadFromBatchPayload(basePayload, group);
    const cacheKey = metricQueryCacheKey(api, singlePayload, compileEpoch);
    const scopeKey = metricQueryScopeCacheKey(api, singlePayload, compileEpoch);
    const requestedIds = metricQueryRequestedIds(singlePayload);
    const cached = METRIC_QUERY_RESULT_CACHE.get(cacheKey);
    if (cached && cached.expiresAt > now) {
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
  if (pendingGroups.length <= 1) {
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
      registerSceneMetricBatchGroupInflight(api, basePayload, group, compileEpoch)
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
          METRIC_QUERY_RESULT_CACHE.set(registration.cacheKey, {
            data: normalized,
            expiresAt,
          });
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
          for (const request of liveRequests) {
            const projected = projectScheduledSceneMetricBatchResult(
              sceneBatchData,
              safeTrim(request.datasetId) ||
                resolveRuntimeMetricRef(request.props)?.dataset_id ||
                datasetId,
              request.metricIds
            );
            if (projected && Array.isArray(projected.metrics) && projected.metrics.length > 0) {
              request.resolve(projected);
            } else {
              request.reject(new Error("scene metric batch projection failed"));
            }
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
  return waitForSharedPromise(promise, signal);
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
    signal = undefined,
    meta = {},
  } = {}
) {
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
  const runtimeRef = metricRef || dataRef;
  const baseCoords = sceneQueryCoords(props, runtimeRef);
  const coords = capability.requiresSceneId
    ? requireSceneQualifiedRequest(baseCoords, "dataset query", meta)
    : baseCoords;
  const normalizedSort = Array.isArray(sort)
    ? sort
        .map((item) => ({
          field: String(item?.field || "").trim(),
          direction: String(item?.direction || "asc").trim().toLowerCase() || "asc",
        }))
        .filter((item) => item.field)
    : [];
  const queryStatePayload = mergedQueryStatePayload(effectiveQueryStateId, filters, {
    search,
    filterIntentSource: meta?.filter_intent_source ?? meta?.filterIntentSource,
  });
  const payload = {
    ...coords,
    dataset_id: datasetId,
    metric_id: metricId || undefined,
    page,
    page_size: pageSize,
    search: queryStatePayload.search || undefined,
    filters: queryStatePayload.filters,
    query_state: {
      filters: queryStatePayload.filters,
      search: queryStatePayload.search || undefined,
      group: queryStatePayload.group.length > 0 ? queryStatePayload.group : undefined,
      time_range: queryStatePayload.timeRange || undefined,
    },
    filter_intents:
      metricId && queryStatePayload.filterIntents.length > 0
        ? queryStatePayload.filterIntents
        : undefined,
    full: !!full,
    sort: normalizedSort.length > 0 ? normalizedSort : undefined,
    column_state:
      columnState && typeof columnState === "object" && !Array.isArray(columnState)
        ? columnState
        : undefined,
    summary: summary === true,
  };
  const errorContext = {
    scene_id: safeTrim(payload.scene_id || props?._mei?.active_scene_id),
    target: safeTrim(payload.target || props?._mei?.active_target_file),
    component: safeTrim(meta?.component),
    panel_id: safeTrim(meta?.panel_id || meta?.panelId),
    request_id: safeTrim(meta?.request_id || meta?.requestId),
  };
  const compileEpoch = runtimeCompileEpoch(props);
  maybeInvalidateRuntimeQueryCachesForCompileEpoch(compileEpoch);
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
  const cacheKey = datasetQueryCacheKey(api, payload, compileEpoch);
  const now = Date.now();
  pruneDatasetQueryCaches(now);
  const cached = DATASET_QUERY_RESULT_CACHE.get(cacheKey);
  if (cached && cached.expiresAt > now) {
    return waitForSharedPromise(Promise.resolve(cached.data), signal);
  }
  let shared = DATASET_QUERY_INFLIGHT.get(cacheKey);
  if (!shared) {
    const managedController = createManagedAbortController();
    const promise = fetchDatasetRowsUncached(api, payload, errorContext, {
      metricId,
      datasetId,
      signal: managedController.signal,
    })
      .then((data) => {
        DATASET_QUERY_RESULT_CACHE.set(cacheKey, {
          data,
          expiresAt: Date.now() + DATASET_QUERY_CACHE_TTL_MS,
        });
        return data;
      })
      .finally(() => {
        managedController.__meiRelease?.();
        DATASET_QUERY_INFLIGHT.delete(cacheKey);
      });
    shared = { promise };
    DATASET_QUERY_INFLIGHT.set(cacheKey, shared);
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
    ({ response, data, clientPerf, errorText } = await fetchJsonWithClientPerf(api, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(payload),
      signal,
    }));
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
    throw new Error(text);
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
    dataset_id: runtimeRef.dataset_id,
    metric_ids: [...ids].sort(),
    search: queryStatePayload.search || undefined,
    filters: queryStatePayload.filters,
    query_state: {
      filters: queryStatePayload.filters,
      search: queryStatePayload.search || undefined,
      group: queryStatePayload.group.length > 0 ? queryStatePayload.group : undefined,
      time_range: queryStatePayload.timeRange || undefined,
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
  maybeInvalidateRuntimeQueryCachesForCompileEpoch(compileEpoch);
  if (runtimePerfDisabled("runtime_metric_share")) {
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
  const cacheKey = metricQueryCacheKey(api, payload, compileEpoch);
  const scopeKey = metricQueryScopeCacheKey(api, payload, compileEpoch);
  const requestedIds = metricQueryRequestedIds(payload);
  const now = Date.now();
  pruneMetricQueryCaches(now);
  const cached = METRIC_QUERY_RESULT_CACHE.get(cacheKey);
  if (cached && cached.expiresAt > now) {
    return waitForSharedPromise(Promise.resolve(cached.data), signal);
  }
  const scopeCached = findCoveringMetricScopeResult(scopeKey, requestedIds, now);
  if (scopeCached) {
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
  let shared = METRIC_QUERY_INFLIGHT.get(cacheKey);
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
        METRIC_QUERY_RESULT_CACHE.set(cacheKey, {
          data,
          expiresAt: Date.now() + METRIC_QUERY_CACHE_TTL_MS,
        });
        rememberMetricScopeResult(
          scopeKey,
          requestedIds,
          data,
          Date.now() + METRIC_QUERY_CACHE_TTL_MS
        );
        return data;
      })
      .finally(() => {
        managedController.__meiRelease?.();
        METRIC_QUERY_INFLIGHT.delete(cacheKey);
        unregisterMetricScopeInflight(scopeKey, scopeEntry);
      });
    shared = { promise };
    const scopeEntry = registerMetricScopeInflight(scopeKey, requestedIds, promise);
    METRIC_QUERY_INFLIGHT.set(cacheKey, shared);
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
  } = {}
) {
  const resolvedQueryStateId = String(queryStateId || queryStateIdOf(props) || "").trim();
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
    mergeFilters,
    resolveDatasetQueryCapability,
    sharedFiltersForQueryStateId,
    sharedFilterIntentsForQueryStateId,
    sharedSearchForQueryStateId,
    resolveRuntimeDataRef,
    resolveRuntimeMetricRef,
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
    props?.noViolMetric,
    props?.no_viol_metric,
    props?.dataset?.dataset,
    props?.dataset,
  ].filter(Boolean);
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
