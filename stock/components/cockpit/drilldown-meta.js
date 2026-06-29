/** 表格行 / 指标槽位共用的 scene-open 元数据解析（保留 drilldown 事件别名作兼容）。 */

export const DRILLDOWN_EVENT_NAME = "mei:metric-drilldown";
export const ANALYSIS_OPEN_EVENT_NAME = "mei:analysis-open";
export const POPUP_OPEN_EVENT_NAME = "mei:popup-open";
export const SCENE_OPEN_EVENT_NAME = "mei:scene-open";

function nonEmptyString(...values) {
  for (const value of values) {
    const text = String(value || "").trim();
    if (text) return text;
  }
  return "";
}

function metricRefId(value) {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return "";
  }
  const runtimeRef = value.__mei_runtime_ref;
  if (runtimeRef && typeof runtimeRef === "object" && !Array.isArray(runtimeRef)) {
    return nonEmptyString(runtimeRef.metric_id, runtimeRef.metricId);
  }
  return nonEmptyString(value.metric_id, value.metricId);
}

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

export function sceneDrilldownContextValue(props, key) {
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
  const shellContract = raw.shell_contract ?? raw.shellContract;
  if (shellContract && typeof shellContract === "object" && !Array.isArray(shellContract)) {
    out.shell_contract = shellContract;
  }
  const params = raw.params;
  if (params && typeof params === "object" && !Array.isArray(params)) {
    out.params = params;
  }
  const context = raw.context;
  if (context && typeof context === "object" && !Array.isArray(context)) {
    out.context = context;
  }
  const target = raw.target;
  if (target && typeof target === "object" && !Array.isArray(target)) {
    out.target = target;
  }
  const presentation = raw.presentation;
  if (presentation && typeof presentation === "object" && !Array.isArray(presentation)) {
    out.presentation = presentation;
  }
  const accepts = raw.accepts;
  if (accepts && typeof accepts === "object" && !Array.isArray(accepts)) {
    out.accepts = accepts;
  }
  if (Array.isArray(raw.capabilities)) {
    out.capabilities = raw.capabilities;
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
  const targetRaw =
    raw.target && typeof raw.target === "object" && !Array.isArray(raw.target)
      ? raw.target
      : null;
  const presentationRaw =
    raw.presentation && typeof raw.presentation === "object" && !Array.isArray(raw.presentation)
      ? raw.presentation
      : null;
  const contextRaw =
    raw.context && typeof raw.context === "object" && !Array.isArray(raw.context)
      ? raw.context
      : null;
  const sceneRef = raw.scene && typeof raw.scene === "object" && !Array.isArray(raw.scene) ? raw.scene : null;
  const isBoardLink =
    raw.__kind === "board_link" || String(raw.mode || "").trim() === "board_link";
  const isPanelPopup =
    !isBoardLink &&
    (raw.__kind === "popup_panel" || String(raw.mode || "").trim() === "popup_panel");
  const isSceneOpen = String(raw.kind || "").trim() === "scene_open";
  const mode = isBoardLink ? "board_link" : isPanelPopup ? "popup_panel" : String(raw.mode || "").trim();
  let template = String(raw.template || raw.legacy_template || "").trim();
  if (template === "metric_default") {
    template = "metric_board_default";
  }
  const sceneFile = String(
    raw.scene_file ||
      raw.sceneFile ||
      targetRaw?.scene_file ||
      targetRaw?.sceneFile ||
      sceneRef?.scene_file ||
      sceneRef?.sceneFile ||
      "",
  ).trim();
  const sceneId = String(
    raw.scene_id ||
      raw.sceneId ||
      targetRaw?.scene_id ||
      targetRaw?.sceneId ||
      sceneRef?.scene_id ||
      sceneRef?.sceneId ||
      sceneRef?.scene?.id ||
      "",
  ).trim();
  const projection = String(raw.projection || presentationRaw?.projection || "overlay").trim() || "overlay";
  const popupType = String(raw.type || presentationRaw?.type || "").trim();
  const overlaySize = String(
    raw.overlay_size || raw.overlaySize || presentationRaw?.overlay_size || presentationRaw?.overlaySize || "",
  ).trim();
  const overlayWorkspace =
    raw.overlay_workspace && typeof raw.overlay_workspace === "object" && !Array.isArray(raw.overlay_workspace)
      ? raw.overlay_workspace
      : raw.overlayWorkspace && typeof raw.overlayWorkspace === "object" && !Array.isArray(raw.overlayWorkspace)
        ? raw.overlayWorkspace
        : presentationRaw?.overlay_workspace &&
            typeof presentationRaw.overlay_workspace === "object" &&
            !Array.isArray(presentationRaw.overlay_workspace)
          ? presentationRaw.overlay_workspace
          : presentationRaw?.overlayWorkspace &&
              typeof presentationRaw.overlayWorkspace === "object" &&
              !Array.isArray(presentationRaw.overlayWorkspace)
            ? presentationRaw.overlayWorkspace
            : null;
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
  const assemblyById = sceneDrilldownContextValue(props, "scene_projection_assembly_by_id");
  const assemblyEntry =
    sceneId && assemblyById && typeof assemblyById === "object" && !Array.isArray(assemblyById)
      ? assemblyById[sceneId]
      : null;
  const projectionSlots = normalizeProjectionSlots(
    raw.projection_slots ??
      raw.projectionSlots ??
      assemblyEntry?.projection_slots ??
      assemblyEntry?.projectionSlots,
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
    !isSceneOpen &&
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
  const scene = sceneRef || (sceneId || sceneFile ? { scene_id: sceneId, scene_file: sceneFile } : null);
  const params =
    raw.params && typeof raw.params === "object" && !Array.isArray(raw.params)
      ? raw.params
      : contextRaw?.params && typeof contextRaw.params === "object" && !Array.isArray(contextRaw.params)
        ? contextRaw.params
        : null;
  const accepts =
    raw.accepts && typeof raw.accepts === "object" && !Array.isArray(raw.accepts)
      ? raw.accepts
      : targetRaw?.accepts && typeof targetRaw.accepts === "object" && !Array.isArray(targetRaw.accepts)
        ? targetRaw.accepts
        : assemblyEntry?.accepts && typeof assemblyEntry.accepts === "object" && !Array.isArray(assemblyEntry.accepts)
          ? assemblyEntry.accepts
          : assemblyEntry?.params && typeof assemblyEntry.params === "object" && !Array.isArray(assemblyEntry.params)
            ? assemblyEntry.params
            : null;
  const capabilities = Array.isArray(raw.capabilities)
    ? raw.capabilities
    : Array.isArray(targetRaw?.capabilities)
      ? targetRaw.capabilities
      : Array.isArray(assemblyEntry?.capabilities)
        ? assemblyEntry.capabilities
        : [];
  const target =
    targetRaw ||
    (sceneId || sceneFile
      ? {
          kind: "board",
          scene_id: sceneId,
          scene_file: sceneFile,
          ...(accepts ? { accepts } : {}),
          ...(capabilities.length ? { capabilities } : {}),
        }
      : null);
  const presentation =
    presentationRaw ||
    {
      kind: "overlay_board",
      projection,
      type: popupType || "popup",
      ...(overlaySize ? { overlay_size: overlaySize } : {}),
      ...(overlayWorkspace ? { overlay_workspace: overlayWorkspace } : {}),
    };
  return {
    ...boardLinkPassthroughFields(raw),
    kind: isSceneOpen ? "scene_open" : String(raw.kind || "").trim(),
    mode: mode || (isBoardLink ? "board_link" : isPanelPopup ? "popup_panel" : "popup"),
    type: popupType || "popup",
    template,
    focus,
    entry,
    entry_tab: entry,
    scene_file: nonEmptyString(sceneFile, assemblyEntry?.target_file, assemblyEntry?.targetFile),
    scene_id: sceneId,
    scene,
    projection,
    ...(overlaySize ? { overlay_size: overlaySize } : {}),
    ...(overlayWorkspace ? { overlay_workspace: overlayWorkspace } : {}),
    local_nav: localNav || assemblyEntry?.local_nav || assemblyEntry?.localNav || null,
    entry_overrides: entryOverrides,
    bindings: entryOverrides,
    slots: entryOverrides,
    metrics: entryOverrides,
    title,
    params,
    context: contextRaw || (params ? { params } : null),
    target,
    presentation,
    accepts,
    capabilities,
    projection_slots: projectionSlots,
    world_scene_file: worldSceneFile,
    world_scene_id: worldSceneId,
  };
}

export function sceneOpenMeta(props) {
  const popup = popupConfigOf(props);
  const boardSceneId = nonEmptyString(
    popup?.scene_id,
    popup?.sceneId,
    popup?.scene?.scene_id,
    popup?.target?.scene_id,
    popup?.target?.sceneId,
  );
  const boardSceneFile = nonEmptyString(
    popup?.scene_file,
    popup?.sceneFile,
    popup?.scene?.scene_file,
    popup?.scene?.sceneFile,
    popup?.target?.scene_file,
    popup?.target?.sceneFile,
  );
  if (!popup || !boardSceneId || !boardSceneFile) {
    return null;
  }
  const params =
    popup?.params && typeof popup.params === "object" && !Array.isArray(popup.params)
      ? popup.params
      : popup?.context?.params && typeof popup.context.params === "object" && !Array.isArray(popup.context.params)
        ? popup.context.params
        : {};
  return {
    kind: "scene_open",
    popup,
    scene_open: {
      target:
        popup.target ||
        {
          kind: "board",
          scene_id: boardSceneId,
          scene_file: boardSceneFile,
        },
      params,
      presentation:
        popup.presentation ||
        {
          kind: "overlay_board",
          projection: String(popup.projection || "overlay").trim() || "overlay",
          type: String(popup.type || "popup").trim() || "popup",
          ...(popup.overlay_size ? { overlay_size: popup.overlay_size } : {}),
          ...(popup.overlay_workspace ? { overlay_workspace: popup.overlay_workspace } : {}),
        },
    },
    board_scene_id: boardSceneId,
    board_scene_file: boardSceneFile,
    projection: String(popup.projection || "overlay").trim() || "overlay",
    host_scene_id: String(props?._mei?.active_scene_id || "").trim(),
    host_scene_file: String(props?._mei?.active_target_file || "").trim(),
    scene_local_nav_by_target: sceneDrilldownContextValue(props, "scene_local_nav_by_target"),
    scene_bindings_by_id: sceneDrilldownContextValue(props, "scene_bindings_by_id"),
    scene_examples_by_id: sceneDrilldownContextValue(props, "scene_examples_by_id"),
    scene_projection_assembly_by_id: sceneDrilldownContextValue(
      props,
      "scene_projection_assembly_by_id",
    ),
  };
}

function drilldownMetricRuntimeRef(props) {
  const raw =
    props?.drilldownMetric ??
    props?.drilldown_metric ??
    props?.drilldown ??
    props?.action_metric ??
    props?.actionMetric ??
    props?.action_content ??
    props?.actionContent ??
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
  const base = sceneOpenMeta(props);
  const popup = base?.popup || null;
  const queryStateId = String(props?.query_state || props?.queryState || "").trim();
  const ref = drilldownMetricRuntimeRef(props);
  if (!base || !ref) {
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
  const boardSceneId = String(
    popup.scene_id ||
      popup.sceneId ||
      popup.scene?.scene_id ||
      popup.scene?.sceneId ||
      "",
  ).trim();
  const assemblyById = sceneDrilldownContextValue(props, "scene_projection_assembly_by_id");
  const assemblyEntry =
    boardSceneId && assemblyById && typeof assemblyById === "object" && !Array.isArray(assemblyById)
      ? assemblyById[boardSceneId]
      : null;
  const projectionSlots = normalizeProjectionSlots(
    popup.projection_slots ??
      popup.projectionSlots ??
      assemblyEntry?.projection_slots ??
      assemblyEntry?.projectionSlots,
  );
  const hasProjectionSlots = projectionSlots.length > 0;
  const popupParams =
    popup?.params && typeof popup.params === "object" && !Array.isArray(popup.params)
      ? popup.params
      : null;
  const hasPopupMetricParams = Boolean(metricRefId(popupParams?.metric));
  if (
    !contract &&
    !hasProjectionSlots &&
    !hasPopupMetricParams &&
    !boardSceneId
  ) {
    return null;
  }
  const filterSchema =
    popup.filter_schema ??
    popup.filterSchema ??
    assemblyEntry?.filter_schema ??
    assemblyEntry?.filterSchema ??
    null;
  const shellContract =
    popup.shell_contract ??
    popup.shellContract ??
    assemblyEntry?.shell_contract ??
    assemblyEntry?.shellContract ??
    null;
  const popupOut = {
    ...popup,
    ...(hasProjectionSlots ? { projection_slots: projectionSlots } : {}),
    ...(filterSchema && typeof filterSchema === "object" && !Array.isArray(filterSchema)
      ? { filter_schema: filterSchema }
      : {}),
    ...(shellContract && typeof shellContract === "object" && !Array.isArray(shellContract)
      ? { shell_contract: shellContract }
      : {}),
  };
  const enrichedPopup = {
    ...popupOut,
    scene_id: boardSceneId || popupOut.scene_id,
    scene_file: nonEmptyString(
      popupOut.scene_file,
      popupOut.sceneFile,
      popupOut.scene?.scene_file,
      popupOut.scene?.sceneFile,
      assemblyEntry?.target_file,
    ),
    local_nav:
      popupOut.local_nav ||
      popupOut.localNav ||
      assemblyEntry?.local_nav ||
      assemblyEntry?.localNav ||
      null,
  };
  return {
    ...base,
    popup: enrichedPopup,
    analysis_contract: contract,
    metric_id: metricId,
    dataset_id: datasetId,
    host_scene_id: String(ref.scene_id || base.host_scene_id || "").trim(),
    host_scene_file: String(ref.scene_path || base.host_scene_file || "").trim(),
    page_scene_id: String(props?._mei?.active_scene_id || "").trim(),
    page_scene_file: String(
      props?._mei?.active_target_file || props?._mei?.entry_target || "",
    ).trim(),
    scene_id: String(ref.scene_id || props?._mei?.active_scene_id || "").trim(),
    scene_path: String(ref.scene_path || props?._mei?.active_target_file || "").trim(),
    query_state_id: queryStateId,
    board_scene_file: nonEmptyString(enrichedPopup.scene_file, popupOut.scene_file || ""),
    board_scene_id: boardSceneId,
    projection: String(popup.projection || "overlay").trim() || "overlay",
  };
}

function rowDrilldownBinding(props) {
  const raw = props?.row_drilldown ?? props?.rowDrilldown;
  return raw && typeof raw === "object" && !Array.isArray(raw) ? raw : null;
}

function firstNonEmptyRowValue(row, fields) {
  if (!row || typeof row !== "object") {
    return "";
  }
  for (const field of fields) {
    const key = String(field || "").trim();
    if (!key) continue;
    const value = String(row[key] ?? "").trim();
    if (value) return value;
  }
  return "";
}

function buildRowDrilldownFilters(meta, row = {}, props = {}) {
  const binding = rowDrilldownBinding(props);
  if (binding) {
    const filterKey = String(binding.filter_key ?? binding.filterKey ?? "").trim();
    const filterFields = Array.isArray(binding.filter_fields ?? binding.filterFields)
      ? binding.filter_fields ?? binding.filterFields
      : filterKey
        ? [filterKey]
        : [];
    const value = firstNonEmptyRowValue(row, filterFields);
    if (value) {
      return { [filterKey || filterFields[0]]: value };
    }
    return {};
  }

  const genericValue = String(row?.value ?? "").trim();
  if (genericValue) {
    const idKey = Object.keys(row).find((key) => {
      const value = String(row[key] ?? "").trim();
      return value && /(^id$|_id$|ID$)/.test(key);
    });
    if (idKey) {
      return { [idKey]: genericValue };
    }
  }
  return {};
}

function buildRowDrilldownLabel(meta, row = {}, filters = {}, props = {}) {
  const binding = rowDrilldownBinding(props);
  if (binding) {
    const labelFields = Array.isArray(binding.label_fields ?? binding.labelFields)
      ? binding.label_fields ?? binding.labelFields
      : [];
    const parts = labelFields
      .map((field) => {
        const key = String(field || "").trim();
        if (!key) return "";
        return String(filters[key] ?? row?.[key] ?? "").trim();
      })
      .filter(Boolean);
    if (parts.length) {
      return parts.join(" · ");
    }
    const fallback = String(binding.label_fallback ?? binding.labelFallback ?? "").trim();
    if (fallback) return fallback;
  }
  return String(row?.label ?? row?.value ?? "").trim();
}

export function buildTableRowDrilldownDetail(meta, row = {}, props = {}) {
  if (!meta) {
    return null;
  }
  const panelId =
    props?._mei?.panel_id ||
    props?.panel_id ||
    "";
  const filters = buildRowDrilldownFilters(meta, row, props);
  const label = buildRowDrilldownLabel(meta, row, filters, props);
  const filterKey = String(
    Object.keys(filters)[0] ||
      rowDrilldownBinding(props)?.filter_key ||
      rowDrilldownBinding(props)?.filterKey ||
      "",
  ).trim();
  const value = String(
    (filterKey && filters[filterKey]) ||
      row?.value ||
      "",
  ).trim();
  const detail = {
    ...meta,
    panel_id: String(panelId || "").trim(),
    label,
    value,
    desc: label,
  };
  if (Object.keys(filters).length) {
    detail.drilldown_filters = filters;
    detail.default_filters = filters;
  }
  return detail;
}

export function emitTableRowDrilldown(host, detail) {
  if (!host || !detail) {
    return;
  }
  host.dispatchEvent(
    new CustomEvent(SCENE_OPEN_EVENT_NAME, {
      bubbles: true,
      composed: true,
      detail,
    }),
  );
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
