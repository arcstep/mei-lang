/** 表格行 / 指标槽位共用的 scene-open 元数据解析（保留 drilldown 事件别名作兼容）。 */

export const DRILLDOWN_EVENT_NAME = "mei:metric-drilldown";
export const ANALYSIS_OPEN_EVENT_NAME = "mei:analysis-open";
export const POPUP_OPEN_EVENT_NAME = "mei:popup-open";

function globalSceneDrilldownContext() {
  if (typeof window === "undefined") {
    return null;
  }
  const cached = window.__meiSceneDrilldownContext;
  if (cached && typeof cached === "object") {
    return cached;
  }
  const script = document.getElementById("mei-scene-drilldown-context");
  const raw = String(script?.textContent || "").trim();
  if (!raw) {
    return null;
  }
  try {
    const parsed = JSON.parse(raw);
    if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
      window.__meiSceneDrilldownContext = parsed;
      return parsed;
    }
  } catch (_) {
    /* ignore */
  }
  return null;
}

function sceneDrilldownContextValue(props, key) {
  const local = props?._mei?.[key];
  if (local && typeof local === "object" && !Array.isArray(local)) {
    return local;
  }
  const global = globalSceneDrilldownContext();
  const value = global?.[key];
  return value && typeof value === "object" && !Array.isArray(value) ? value : null;
}

/** Lowered board_link analytics fields must survive popupConfigOf → drilldown host. */
export function boardLinkPassthroughFields(raw) {
  if (!raw || typeof raw !== "object" || Array.isArray(raw)) {
    return {};
  }
  const out = {};
  const filterSchema = raw.filter_schema ?? raw.filterSchema;
  if (filterSchema && typeof filterSchema === "object" && !Array.isArray(filterSchema)) {
    out.filter_schema = filterSchema;
  }
  const layoutMode = String(raw.layout_mode ?? raw.layoutMode ?? "").trim();
  if (layoutMode) {
    out.layout_mode = layoutMode;
  }
  const overlaySize = String(raw.overlay_size ?? raw.overlaySize ?? "").trim();
  if (overlaySize) {
    out.overlay_size = overlaySize;
  }
  const queryStateId = String(raw.query_state_id ?? raw.queryStateId ?? "").trim();
  if (queryStateId) {
    out.query_state_id = queryStateId;
  }
  return out;
}

function normalizeProjectionSlots(raw) {
  if (!Array.isArray(raw)) {
    return [];
  }
  return raw
    .map((entry) => {
      if (!entry || typeof entry !== "object" || Array.isArray(entry)) {
        return null;
      }
      const metricId = String(entry.metric_id || entry.metricId || entry.metric?.id || "").trim();
      const datasetId = String(entry.dataset_id || entry.datasetId || "").trim();
      const supportRole = String(entry.support_role || entry.supportRole || "").trim();
      const id = String(
        entry.id ||
          entry.explain_block_id ||
          entry.explainBlockId ||
          entry.tab ||
          supportRole ||
          "",
      ).trim();
      if (!id && !metricId) {
        return null;
      }
      return {
        ...entry,
        id: id || metricId,
        metric_id: metricId,
        dataset_id: datasetId,
        support_role: supportRole || entry.support_role,
        default: Boolean(entry.default),
      };
    })
    .filter(Boolean);
}

export function popupConfigOf(props) {
  const raw = props?.popup ?? props?.analysis;
  if (!raw || typeof raw !== "object" || Array.isArray(raw)) {
    return null;
  }
  const sceneRef = raw.scene && typeof raw.scene === "object" && !Array.isArray(raw.scene) ? raw.scene : null;
  const isBoardLink =
    raw.__kind === "board_link" || String(raw.mode || "").trim() === "board_link";
  const isPanelPopup =
    !isBoardLink &&
    (raw.__kind === "popup_panel" || String(raw.mode || "").trim() === "popup_panel");
  const mode = isBoardLink ? "board_link" : isPanelPopup ? "popup_panel" : String(raw.mode || "").trim();
  let template = String(raw.template || raw.legacy_template || "").trim();
  if (template === "metric_default") {
    template = "metric_board_default";
  }
  const sceneFile = String(
    raw.scene_file || raw.sceneFile || sceneRef?.scene_file || sceneRef?.sceneFile || "",
  ).trim();
  const sceneId = String(
    raw.scene_id ||
      raw.sceneId ||
      sceneRef?.scene_id ||
      sceneRef?.sceneId ||
      sceneRef?.scene?.id ||
      "",
  ).trim();
  const projection = String(raw.projection || "overlay").trim() || "overlay";
  const popupType = String(raw.type || "").trim();
  const entry = String(
    raw.entry ||
      raw.entry_tab ||
      raw.entryTab ||
      sceneRef?.entry ||
      sceneRef?.entry_tab ||
      sceneRef?.entryTab ||
      raw.focus ||
      raw.default_focus ||
      raw.defaultFocus ||
      "",
  ).trim();
  const focus = entry || String(raw.focus || "").trim();
  const entryOverrides =
    raw.bindings && typeof raw.bindings === "object" && !Array.isArray(raw.bindings)
      ? raw.bindings
      : raw.entry_overrides && typeof raw.entry_overrides === "object" && !Array.isArray(raw.entry_overrides)
        ? raw.entry_overrides
        : raw.entryOverrides && typeof raw.entryOverrides === "object" && !Array.isArray(raw.entryOverrides)
          ? raw.entryOverrides
          : raw.slots && typeof raw.slots === "object" && !Array.isArray(raw.slots)
            ? raw.slots
            : raw.metrics && typeof raw.metrics === "object" && !Array.isArray(raw.metrics)
              ? raw.metrics
              : null;
  const title = String(raw.title || "").trim();
  const localNav =
    raw.local_nav && typeof raw.local_nav === "object" && !Array.isArray(raw.local_nav)
      ? raw.local_nav
      : raw.localNav && typeof raw.localNav === "object" && !Array.isArray(raw.localNav)
        ? raw.localNav
        : sceneRef?.local_nav && typeof sceneRef.local_nav === "object" && !Array.isArray(sceneRef.local_nav)
          ? sceneRef.local_nav
          : null;
  const projectionSlots = normalizeProjectionSlots(
    raw.projection_slots ?? raw.projectionSlots,
  );
  const worldRaw = raw.world && typeof raw.world === "object" && !Array.isArray(raw.world) ? raw.world : null;
  const worldSceneFile = String(
    raw.world_scene_file ||
      raw.worldSceneFile ||
      worldRaw?.scene_file ||
      worldRaw?.sceneFile ||
      worldRaw?.scene_path ||
      worldRaw?.scenePath ||
      "",
  ).trim();
  const worldSceneId = String(
    raw.world_scene_id || raw.worldSceneId || worldRaw?.scene_id || worldRaw?.sceneId || "",
  ).trim();
  if (
    !mode &&
    !template &&
    !focus &&
    !entryOverrides &&
    !sceneFile &&
    !sceneId &&
    !localNav &&
    !projectionSlots.length &&
    !worldSceneFile &&
    !worldSceneId
  ) {
    return null;
  }
  return {
    ...boardLinkPassthroughFields(raw),
    mode: mode || (isBoardLink ? "board_link" : isPanelPopup ? "popup_panel" : "popup"),
    type: popupType || "popup",
    template,
    focus,
    entry,
    entry_tab: entry,
    scene_file: sceneFile,
    scene_id: sceneId,
    scene: sceneRef,
    projection,
    local_nav: localNav,
    entry_overrides: entryOverrides,
    bindings: entryOverrides,
    slots: entryOverrides,
    metrics: entryOverrides,
    title,
    projection_slots: projectionSlots,
    world_scene_file: worldSceneFile,
    world_scene_id: worldSceneId,
  };
}

function drilldownMetricRuntimeRef(props) {
  const raw =
    props?.drilldownMetric ??
    props?.drilldown_metric ??
    props?.drilldown ??
    props?.content ??
    props?.dataset;
  if (!raw || typeof raw !== "object" || Array.isArray(raw)) {
    return null;
  }
  const ref = raw.__mei_runtime_ref;
  if (!ref || typeof ref !== "object" || Array.isArray(ref)) {
    return null;
  }
  if (ref.kind !== "metric") {
    return null;
  }
  return ref;
}

export function tableDrilldownMeta(props) {
  const popup = popupConfigOf(props);
  const queryStateId = String(props?.query_state || props?.queryState || "").trim();
  const ref = drilldownMetricRuntimeRef(props);
  if (!ref) {
    return null;
  }
  if (!popup || (popup.mode !== "board_link" && popup.mode !== "popup_panel" && popup.mode !== "popup")) {
    return null;
  }
  const metricId = String(ref.metric_id || "").trim();
  const datasetId = String(ref.dataset_id || "").trim();
  if (!metricId || !datasetId) {
    return null;
  }
  const contract =
    ref.analysis_contract && typeof ref.analysis_contract === "object" && !Array.isArray(ref.analysis_contract)
      ? ref.analysis_contract
      : null;
  const projectionSlots = normalizeProjectionSlots(
    popup.projection_slots ?? popup.projectionSlots,
  );
  const hasProjectionSlots = projectionSlots.length > 0;
  if (!contract && !hasProjectionSlots) {
    return null;
  }
  const popupOut = hasProjectionSlots ? { ...popup, projection_slots: projectionSlots } : popup;
  return {
    popup: popupOut,
    analysis_contract: contract,
    metric_id: metricId,
    dataset_id: datasetId,
    host_scene_id: String(ref.scene_id || props?._mei?.active_scene_id || "").trim(),
    host_scene_file: String(ref.scene_path || props?._mei?.active_target_file || "").trim(),
    scene_id: String(ref.scene_id || props?._mei?.active_scene_id || "").trim(),
    scene_path: String(ref.scene_path || props?._mei?.active_target_file || "").trim(),
    query_state_id: queryStateId,
    board_scene_file: String(popup.scene_file || "").trim(),
    board_scene_id: String(popup.scene_id || "").trim(),
    projection: String(popup.projection || "overlay").trim() || "overlay",
    scene_local_nav_by_target: sceneDrilldownContextValue(props, "scene_local_nav_by_target"),
    scene_bindings_by_id: sceneDrilldownContextValue(props, "scene_bindings_by_id"),
    scene_examples_by_id: sceneDrilldownContextValue(props, "scene_examples_by_id"),
    scene_projection_assembly_by_id: sceneDrilldownContextValue(
      props,
      "scene_projection_assembly_by_id"
    ),
  };
}

export function buildTableRowDrilldownDetail(meta, row = {}, props = {}) {
  if (!meta) {
    return null;
  }
  const panelId =
    props?._mei?.panel_id ||
    props?.panel_id ||
    "";
  const resultId = String(row?.value ?? row?.处理结果ID ?? "").trim();
  const label = String(row?.label ?? row?.案例名称 ?? "").trim();
  const detail = {
    ...meta,
    panel_id: String(panelId || "").trim(),
    label,
    value: resultId,
    desc: label,
  };
  if (resultId) {
    detail.drilldown_filters = { caseResultId: resultId };
    detail.default_filters = { caseResultId: resultId };
  }
  return detail;
}

export function emitTableRowDrilldown(host, detail) {
  if (!host || !detail) {
    return;
  }
  host.dispatchEvent(
    new CustomEvent(DRILLDOWN_EVENT_NAME, {
      bubbles: true,
      composed: true,
      detail,
    }),
  );
  host.dispatchEvent(
    new CustomEvent(ANALYSIS_OPEN_EVENT_NAME, {
      bubbles: true,
      composed: true,
      detail,
    }),
  );
  host.dispatchEvent(
    new CustomEvent(POPUP_OPEN_EVENT_NAME, {
      bubbles: true,
      composed: true,
      detail,
    }),
  );
}

export const TABLE_ROW_SELECT_EVENT_NAME = "mei:table-row-select";

export function tableRowSelectionMode(props) {
  const raw = String(
    props?.row_selection_mode ?? props?.rowSelectionMode ?? "",
  ).trim();
  return raw === "single" ? "single" : "";
}

export function emitTableRowSelect(host, detail) {
  if (!host || !detail) {
    return;
  }
  host.dispatchEvent(
    new CustomEvent(TABLE_ROW_SELECT_EVENT_NAME, {
      bubbles: true,
      composed: true,
      detail,
    }),
  );
}
