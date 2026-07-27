/** 表格行 / 指标槽位共用的 scene-open 元数据解析（保留 drilldown 事件别名作兼容）。 */

export const DRILLDOWN_EVENT_NAME = "mei:metric-drilldown";
export const ANALYSIS_OPEN_EVENT_NAME = "mei:analysis-open";
export const POPUP_OPEN_EVENT_NAME = "mei:popup-open";
export const SCENE_OPEN_EVENT_NAME = "mei:scene-open";

function nonEmptyString(...values) {
  for (const value of values) {
    // Skip unresolved IR (param_ref / maps); String({}) === "[object Object]".
    if (value == null || typeof value === "object") continue;
    const text = String(value).trim();
    if (text) return text;
  }
  return "";
}

/** Prefer author fields/preset; keep resolved string rowset_dataset_id (not param_ref). */
function mergeAnalyticsFilterSchema(resolved, author) {
  const resolvedOk = resolved && typeof resolved === "object" && !Array.isArray(resolved);
  const authorOk = author && typeof author === "object" && !Array.isArray(author);
  if (!resolvedOk && !authorOk) return null;
  if (!authorOk) return resolved;
  if (!resolvedOk) return author;
  const resolvedRowset = nonEmptyString(resolved.rowset_dataset_id, resolved.rowsetDatasetId);
  const authorRowset = nonEmptyString(author.rowset_dataset_id, author.rowsetDatasetId);
  return {
    ...resolved,
    ...author,
    rowset_dataset_id: authorRowset || resolvedRowset || undefined,
    rowsetDatasetId: authorRowset || resolvedRowset || undefined,
  };
}

function metricRefId(value) {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return "";
  }
  if (value.__ref === "metric") return nonEmptyString(value.id);
  if (value.__ref === "metric_ref") {
    return nonEmptyString(
      value.__args?.arg0,
      value.__args?.[0],
      value.id,
      value.metric_id,
      value.metricId,
    );
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
  // T1 真源用 `popup`；部分作者误写 `row_drilldown_popup`，一并兼容。
  const raw = props?.popup ?? props?.analysis ?? props?.row_drilldown_popup ?? props?.rowDrilldownPopup;
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

function synthesizeMetricRuntimeRef(raw, props = null) {
  if (!raw || typeof raw !== "object" || Array.isArray(raw)) {
    return null;
  }
  const popup = popupConfigOf(props || {});
  const popupParams =
    popup?.params && typeof popup.params === "object" && !Array.isArray(popup.params)
      ? popup.params
      : null;
  const preferredRowset = nonEmptyString(
    popupParams?.rowset_dataset_id,
    popupParams?.rowsetDatasetId,
  );
  const existing = raw.__mei_runtime_ref;
  if (existing && typeof existing === "object" && !Array.isArray(existing)) {
    if (String(existing.kind || "").trim() === "metric") {
      const existingDataset = String(existing.dataset_id || "").trim();
      // Remap bundle-qualified dataset paths onto the popup rowset when available.
      if (preferredRowset && (!existingDataset || existingDataset.includes("::"))) {
        return {
          ...existing,
          dataset_id: preferredRowset,
          scene_id: nonEmptyString(existing.scene_id, popup?.scene_id, popup?.sceneId),
          scene_path: nonEmptyString(existing.scene_path, popup?.scene_file, popup?.sceneFile),
        };
      }
      return existing;
    }
  }
  const metricId = nonEmptyString(
    raw.id,
    raw.metric_id,
    raw.metricId,
    metricRefId(raw),
  );
  // Prefer explicit rowset id from popup (warning_list / typical_cases) over
  // bundle-qualified from_dataset paths used by golden-case metric_ref(bundle=...).
  const datasetId = nonEmptyString(
    preferredRowset,
    raw.dataset_id,
    raw.datasetId,
    raw.from_dataset,
    raw.fromDataset,
  );
  if (!metricId || !datasetId) {
    return null;
  }
  return {
    kind: "metric",
    metric_id: metricId,
    dataset_id: datasetId,
    scene_id: nonEmptyString(popup?.scene_id, popup?.sceneId),
    scene_path: nonEmptyString(popup?.scene_file, popup?.sceneFile),
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
  return synthesizeMetricRuntimeRef(raw, props);
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
  const filterSchema = mergeAnalyticsFilterSchema(
    popup.filter_schema ??
      popup.filterSchema ??
      assemblyEntry?.filter_schema ??
      assemblyEntry?.filterSchema,
    assemblyEntry?.bindings?.filter_schema ?? assemblyEntry?.bindings?.filterSchema,
  );
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
  const caps =
    props?.capabilities && typeof props.capabilities === "object" && !Array.isArray(props.capabilities)
      ? props.capabilities
      : null;
  const raw = props?.row_drilldown ?? props?.rowDrilldown ?? caps?.row_drilldown ?? caps?.rowDrilldown;
  return raw && typeof raw === "object" && !Array.isArray(raw) ? raw : null;
}

function objectLocatorBinding(props) {
  const caps =
    props?.capabilities && typeof props.capabilities === "object" && !Array.isArray(props.capabilities)
      ? props.capabilities
      : null;
  const rowBinding = rowDrilldownBinding(props);
  const raw =
    props?.object_locator ??
    props?.objectLocator ??
    caps?.object_locator ??
    caps?.objectLocator ??
    rowBinding?.object_locator ??
    rowBinding?.objectLocator;
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

  const locator = objectLocatorBinding(props);
  if (locator) {
    const identityFields = [
      locator.identity_field,
      locator.identityField,
      ...(Array.isArray(locator.identity_aliases ?? locator.identityAliases)
        ? locator.identity_aliases ?? locator.identityAliases
        : []),
    ];
    const value = firstNonEmptyRowValue(row, identityFields);
    const filterKey = nonEmptyString(locator.legacy_filter_key, locator.legacyFilterKey);
    if (value && filterKey) return { [filterKey]: value };
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
  const locator = objectLocatorBinding(props);
  if (locator) {
    const identityFields = [
      locator.identity_field,
      locator.identityField,
      ...(Array.isArray(locator.identity_aliases ?? locator.identityAliases)
        ? locator.identity_aliases ?? locator.identityAliases
        : []),
    ];
    const identityLabel = firstNonEmptyRowValue(row, identityFields);
    if (identityLabel) return identityLabel;
  }
  const filterValues = Object.values(filters || {})
    .map((value) => String(value ?? "").trim())
    .filter(Boolean);
  if (filterValues.length) return filterValues[0];
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
  const locator = objectLocatorBinding(props);
  if (locator) {
    const identityFields = [
      locator.identity_field,
      locator.identityField,
      ...(Array.isArray(locator.identity_aliases ?? locator.identityAliases)
        ? locator.identity_aliases ?? locator.identityAliases
        : []),
    ];
    const objectKey = firstNonEmptyRowValue(row, identityFields);
    const objectType = nonEmptyString(locator.object_type, locator.objectType);
    if (objectType && objectKey) {
      detail.object_locator = { objectType, objectKey };
      detail.object_intents = ["select", "open_projection"];
    }
  }
  if (Object.keys(filters).length) {
    // Identity only — 禁止双写进 default_filters（024005）。
    detail.drilldown_filters = filters;
  }
  return detail;
}

export function emitTableRowDrilldown(host, detail) {
  if (!host || !detail) {
    return;
  }
  const locator = detail.object_locator;
  if (locator?.objectType && locator?.objectKey != null) {
    const interaction = window.MeiInteraction || window.__meiLangBoot?.interactionRuntime;
    interaction?.dispatchMany?.(detail.object_intents || ["select", "open_projection"], {
      ...locator,
      source: "cockpit.data-table",
    });
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

function readPresentationFieldLinks() {
  if (typeof window === "undefined") return {};
  const fromBoot = window.__mei?.presentation_map?.objectFieldLinksByObjectType;
  if (fromBoot && typeof fromBoot === "object") return fromBoot;
  const node = document.getElementById?.("mei-presentation-map");
  if (
    typeof HTMLScriptElement !== "undefined" &&
    node instanceof HTMLScriptElement &&
    node.textContent
  ) {
    try {
      return JSON.parse(node.textContent)?.objectFieldLinksByObjectType || {};
    } catch (_) {
      return {};
    }
  }
  return {};
}

export function resolveObjectFieldLinks(props = {}) {
  const direct =
    props?.object_field_links ||
    props?.objectFieldLinks ||
    props?.capabilities?.object_field_links ||
    props?.capabilities?.objectFieldLinks;
  if (direct && typeof direct === "object" && !Array.isArray(direct)) {
    return direct;
  }
  const locator = objectLocatorBinding(props);
  const objectType = nonEmptyString(locator?.object_type, locator?.objectType);
  if (!objectType) return {};
  const byType = readPresentationFieldLinks();
  const links = byType?.[objectType];
  return links && typeof links === "object" && !Array.isArray(links) ? links : {};
}

function expandMappingTargets(raw) {
  if (Array.isArray(raw)) {
    return raw.map((value) => String(value ?? "").trim()).filter(Boolean);
  }
  const text = String(raw ?? "").trim();
  return text ? [text] : [];
}

/** Build composite mapping keys from cell value + optional qualifier sibling fields. */
export function buildMappingLookupKeys(row = {}, cellValue = "", qualifierFields = []) {
  const base = String(cellValue ?? "").trim();
  if (!base) return [];
  const quals = (Array.isArray(qualifierFields) ? qualifierFields : [])
    .map((field) => String(field || "").trim())
    .filter(Boolean)
    .map((field) => String(row?.[field] ?? "").trim())
    .filter(Boolean);
  const keys = [];
  // Longest composite first: name|level|rule … then prefixes … then bare name.
  for (let n = quals.length; n >= 1; n -= 1) {
    keys.push([base, ...quals.slice(0, n)].join("|"));
  }
  // Also try name|each-qualifier alone (e.g. name|规则类型 without level).
  quals.forEach((q) => {
    const key = `${base}|${q}`;
    if (!keys.includes(key)) keys.push(key);
  });
  keys.push(base);
  return keys;
}

function resolveMappingTargetsForCell(spec, row, cellValue) {
  const map = spec?.targetsByValue || spec?.targets_by_value || {};
  const qualifierFields =
    Array.isArray(spec?.qualifierFields) && spec.qualifierFields.length
      ? spec.qualifierFields
      : Array.isArray(spec?.qualifier_fields) && spec.qualifier_fields.length
        ? spec.qualifier_fields
        : ["预警等级", "规则类型"];
  const keys = buildMappingLookupKeys(row, cellValue, qualifierFields);
  for (const key of keys) {
    if (!Object.prototype.hasOwnProperty.call(map, key)) continue;
    const mapped = expandMappingTargets(map[key]);
    if (mapped.length) return mapped;
  }
  // IssueResult 等行常缺「规则类型」：按 base + 已有 qualifier 前缀回收候选（多值则交给选择器）。
  const base = String(cellValue ?? "").trim();
  if (!base || !map || typeof map !== "object") return [];
  const quals = qualifierFields
    .map((field) => String(row?.[field] ?? "").trim())
    .filter(Boolean);
  const found = [];
  const seen = new Set();
  for (const [key, raw] of Object.entries(map)) {
    if (key !== base && !key.startsWith(`${base}|`)) continue;
    const keyQuals = key.split("|").slice(1);
    if (quals.length && !quals.every((q) => keyQuals.includes(q))) continue;
    for (const id of expandMappingTargets(raw)) {
      if (seen.has(id)) continue;
      seen.add(id);
      found.push(id);
    }
  }
  return found;
}

/**
 * Split multi-value association IDs from Excel cells.
 * Supports newline/whitespace separators and ignorable "1." / "2." prefixes.
 */
export function splitMultiObjectKeys(raw) {
  const text = normalizeObjectIdentityText(raw);
  if (!text) return [];
  return text
    // 健全机制等多值常用顿号/逗号；ID 类也兼容空白换行
    .split(/[\n\r\s、，,;；]+/)
    .map((part) =>
      normalizeObjectIdentityText(
        String(part ?? "")
          .replace(/^\d+\.\s*/, "")
          .replace(/^[《]+|[》]+$/g, ""),
      ),
    )
    .filter(Boolean);
}

/** Excel/Parquet 整型 ID 常为 number 或 "2025001.0"；统一成无小数文本。 */
export function normalizeObjectIdentityText(raw) {
  if (raw == null) return "";
  if (typeof raw === "number" && Number.isFinite(raw)) {
    if (Math.abs(raw % 1) < Number.EPSILON) return String(Math.trunc(raw));
    return String(raw);
  }
  let text = String(raw).trim();
  if (!text) return "";
  if (/^-?\d+\.0+$/.test(text)) text = text.replace(/\.0+$/, "");
  // 健全机制展示常带书名号；身份匹配/过滤时剥离
  text = text.replace(/^[《]+|[》]+$/g, "").trim();
  return text;
}

/** Resolve clickable object targets for one cell from object_field_links IR. */
export function resolveObjectFieldTargets(props = {}, row = {}, columnKey = "") {
  const field = String(columnKey || "").trim();
  if (!field || !row || typeof row !== "object") return [];
  const links = resolveObjectFieldLinks(props);
  const specs = Array.isArray(links[field]) ? links[field] : [];
  if (!specs.length) return [];
  const cellValue = String(row[field] ?? "").trim();
  const out = [];
  for (const spec of specs) {
    if (!spec || typeof spec !== "object") continue;
    const objectType = nonEmptyString(spec.objectType, spec.object_type);
    if (!objectType) continue;
    const resolve = String(spec.resolve || "row_value").trim().toLowerCase();
    const keyMode = String(spec.keyMode || spec.key_mode || "identity")
      .trim()
      .toLowerCase();
    const filterKey = nonEmptyString(spec.filterKey, spec.filter_key);
    const hasDetail = spec.hasDetail === true || spec.has_detail === true;
    const openPopup =
      (spec.openPopup && typeof spec.openPopup === "object" && !Array.isArray(spec.openPopup)
        ? spec.openPopup
        : null) ||
      (spec.open_popup && typeof spec.open_popup === "object" && !Array.isArray(spec.open_popup)
        ? spec.open_popup
        : null);
    const detailPage = nonEmptyString(spec.detailPage, spec.detail_page);
    const relation = nonEmptyString(spec.relation);
    const role = nonEmptyString(spec.role, "relation");

    if (resolve === "mapping") {
      const mapped = resolveMappingTargetsForCell(spec, row, cellValue);
      for (const objectKey of mapped) {
        out.push({
          role,
          relation,
          objectType,
          objectKey,
          keyMode: "identity",
          filterKey,
          hasDetail,
          openPopup,
          detailPage,
          label: objectKey,
        });
      }
      continue;
    }

    if (resolve === "row_sibling" || resolve === "row-sibling" || resolve === "identity_field") {
      // 入口列本身需有展示值（如序号），objectKey 取同行身份字段。
      if (!cellValue) continue;
      const keyField = nonEmptyString(spec.keyField, spec.key_field);
      if (!keyField) continue;
      const siblingText = String(row[keyField] ?? "").trim();
      if (!siblingText) continue;
      const objectKeys = splitMultiObjectKeys(siblingText);
      for (const objectKey of objectKeys) {
        out.push({
          role,
          relation,
          objectType,
          objectKey,
          keyMode,
          filterKey,
          hasDetail,
          openPopup,
          detailPage,
          label: objectKey,
        });
      }
      continue;
    }

    if (!cellValue) continue;
    const objectKeys = splitMultiObjectKeys(cellValue);
    for (const objectKey of objectKeys) {
      out.push({
        role,
        relation,
        objectType,
        objectKey,
        keyMode,
        filterKey,
        hasDetail,
        openPopup,
        detailPage,
        label: objectKey,
      });
    }
  }
  return out;
}

/**
 * When a cell maps to multiple object types (e.g. AlertModel + SupervisionMatter),
 * prefer a unique preferred type so the UI can open directly without a chooser.
 */
export function preferUniqueObjectTargets(targets = [], preferredObjectTypes = []) {
  const list = Array.isArray(targets) ? targets.filter(Boolean) : [];
  if (list.length <= 1) return list;
  const preferred = (Array.isArray(preferredObjectTypes) ? preferredObjectTypes : [])
    .map((type) => String(type || "").trim())
    .filter(Boolean);
  if (!preferred.length) return list;
  for (const type of preferred) {
    const matched = list.filter(
      (target) => String(target?.objectType || target?.object_type || "").trim() === type,
    );
    if (matched.length === 1) return matched;
    if (matched.length > 1) return matched;
  }
  return list;
}

export function emitObjectFieldOpen(host, target, row = {}, props = {}) {
  if (!host || !target) return;
  const objectType = nonEmptyString(target.objectType, target.object_type);
  const objectKey = normalizeObjectIdentityText(target.objectKey ?? target.object_key);
  if (!objectType || !objectKey) return;

  const openPopup =
    (target.openPopup && typeof target.openPopup === "object" ? target.openPopup : null) ||
    null;
  // 字段链接自带详情页 openPopup 时只走页面弹层，避免再派 open_projection
  //（会打开 recipe/alert 默认卡，盖住作者配置的 row_form）。
  const hasObjectDetailPopup = Boolean(
    openPopup && nonEmptyString(openPopup.scene_id, openPopup.sceneId),
  );
  const intents =
    target.hasDetail === false || hasObjectDetailPopup
      ? ["select"]
      : ["select", "open_projection"];
  const interaction = window.MeiInteraction || window.__meiLangBoot?.interactionRuntime;
  interaction?.dispatchMany?.(intents, {
    objectType,
    objectKey,
    source: "cockpit.data-table.field-link",
  });

  const filters = {};
  let filterKey = nonEmptyString(target.filterKey, target.filter_key);
  // 机制文档身份常是「机制名称」长文本，启发式以前可能没写出 filterKey。
  if (
    !filterKey &&
    (objectType === "zhifa.MechanismDocument" || objectType.endsWith(".MechanismDocument"))
  ) {
    filterKey = "mechanismName";
  }
  const keyMode = String(target.keyMode || target.key_mode || "identity")
    .trim()
    .toLowerCase();
  if (filterKey && (keyMode === "foreign_key" || keyMode === "foreign-key")) {
    filters[filterKey] = objectKey;
  } else if (filterKey && keyMode === "identity") {
    filters[filterKey] = objectKey;
  }

  const rowMeta = tableDrilldownMeta(props);
  const isSelf =
    String(target.role || "").trim() === "self" ||
    nonEmptyString(objectLocatorBinding(props)?.object_type, objectLocatorBinding(props)?.objectType) ===
      objectType;

  // 无详情页时才回退表级 row_drilldown；有 openPopup 时绝不能走它（会像刷新分析列表）。
  if (!hasObjectDetailPopup && isSelf && rowMeta) {
    const detail = buildTableRowDrilldownDetail(rowMeta, row, props);
    if (detail) {
      if (!nonEmptyString(detail.label) && objectKey) {
        detail.label = nonEmptyString(target.label, objectKey);
        detail.desc = detail.label;
        detail.value = nonEmptyString(detail.value, objectKey);
      }
      emitTableRowDrilldown(host, detail);
      return;
    }
  }

  if (!hasObjectDetailPopup) {
    return;
  }

  const title = resolveObjectOpenTitle(objectType, row, objectKey, target);
  const detail = {
    ...openPopup,
    popup: openPopup,
    // Warning 详情卡页签固定用预警ID；其它对象仍优先业务名称（风险事项/预警模型等）。
    label: title,
    value: objectKey,
    desc: title,
    object_locator: { objectType, objectKey },
    object_intents: intents,
  };
  if (Object.keys(filters).length) {
    // Identity only — 禁止双写进 default_filters（024005）。
    detail.drilldown_filters = filters;
  }
  host.dispatchEvent(
    new CustomEvent(SCENE_OPEN_EVENT_NAME, {
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

function resolveObjectOpenTitle(objectType, row, objectKey, target) {
  const type = String(objectType || "").trim();
  if (type === "zhifa.Warning" || type.endsWith(".Warning")) {
    return nonEmptyString(
      firstNonEmptyRowValue(row, ["预警ID", "warning_id", "warningId"]),
      objectKey,
      target?.label,
    );
  }
  if (type === "zhifa.IssueResult" || type.endsWith(".IssueResult")) {
    // 监督成效详情页签标题固定用处理结果ID，勿被预警模型等业务名抢占。
    return nonEmptyString(
      firstNonEmptyRowValue(row, ["处理结果ID", "resultId", "result_id"]),
      objectKey,
      target?.label,
    );
  }
  if (type === "zhifa.MechanismDocument" || type.endsWith(".MechanismDocument")) {
    // 多值「健全机制」行上点开单份文档时，标题必须用当前 objectKey，勿回落到整格顿号串。
    return nonEmptyString(
      objectKey,
      target?.label,
      firstNonEmptyRowValue(row, ["机制名称", "健全机制"]),
    );
  }
  // 页签标题优先用行内业务名称（如风险事项/预警模型），避免序号类主键直接当标题。
  return nonEmptyString(
    firstNonEmptyRowValue(row, [
      "风险事项",
      "监督事项",
      "预警模型",
      "预警ID",
      "处理结果ID",
      "label",
      "title",
    ]),
    objectKey,
    target?.label,
    type && objectKey ? `${type}:${objectKey}` : "",
  );
}

if (typeof window !== "undefined") {
  window.MeiDrilldownMeta = {
    ...(window.MeiDrilldownMeta || {}),
    resolveObjectFieldLinks,
    resolveObjectFieldTargets,
    emitObjectFieldOpen,
    splitMultiObjectKeys,
    normalizeObjectIdentityText,
    buildMappingLookupKeys,
    preferUniqueObjectTargets,
  };
}
