  function resolveAccessAppBasePath(pathname = window.location.pathname) {
    if (typeof isUnifiedViewRoute === "function" && isUnifiedViewRoute(pathname)) {
      const appId =
        typeof appIdFromAppsPathname === "function"
          ? nonEmptyString(appIdFromAppsPathname(pathname))
          : "";
      if (appId) return `/apps/${appId}/`;
    }
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
          supportRole: nonEmptyString(entry.support_role, entry.supportRole) ||
            (/composition/i.test(nonEmptyString(entry.explain_block_id, entry.explainBlockId, entry.id))
              ? "composition"
              : /trend/i.test(nonEmptyString(entry.explain_block_id, entry.explainBlockId, entry.id))
                ? "trend"
                : nonEmptyString(entry.component, entry.as) || "data_table"),
          default: Boolean(entry.default),
          fields,
          by,
          chartKind: nonEmptyString(entry.chart_kind, entry.chartKind),
        topN: positiveInt(entry.top_n, entry.topN),
        valueField: nonEmptyString(entry.value_field, entry.valueField),
        trendField: nonEmptyString(entry.trend_field, entry.date_field, entry.dateField),
        dateField: nonEmptyString(entry.date_field, entry.dateField, entry.trend_field),
        grain: nonEmptyString(entry.grain, entry.trend_grain, entry.trendGrain),
        compositionAgg: nonEmptyString(entry.agg, entry.composition_agg, entry.compositionAgg),
        mapping:
            entry.mapping && typeof entry.mapping === "object" && !Array.isArray(entry.mapping)
              ? entry.mapping
              : null,
          explainBlockId: nonEmptyString(entry.explain_block_id, entry.explainBlockId),
          layoutZone: nonEmptyString(entry.layout_zone, entry.layoutZone),
          columnState:
            entry.column_state && typeof entry.column_state === "object"
              ? entry.column_state
              : entry.columnState && typeof entry.columnState === "object"
                ? entry.columnState
                : null,
          pageSize: positiveInt(entry.page_size, entry.pageSize),
          columnTemplate: nonEmptyString(entry.column_template, entry.columnTemplate),
          columnFormats:
            entry.column_formats && typeof entry.column_formats === "object"
              ? entry.column_formats
              : entry.columnFormats && typeof entry.columnFormats === "object"
                ? entry.columnFormats
                : null,
        };
      })
      .filter((slot) => slot.metricId || slot.datasetId);
  }

  function resolveSceneShell(sceneAssembly) {
    return normalizeSceneShellContract(
      sceneAssembly?.frame,
      sceneAssembly?.panels,
      sceneAssembly?.shell_contract,
    );
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
      return { fields: [], rowsetDatasetId: "", defaultCollapsed: false, allowExtra: false, title: "" };
    }
    const fields = Array.isArray(raw.fields)
      ? raw.fields
          .filter((entry) => entry && typeof entry === "object" && !Array.isArray(entry))
          .map((entry) => ({
            key: nonEmptyString(entry.key),
            label: nonEmptyString(entry.label, entry.key),
            column: nonEmptyString(entry.column, entry.key),
            control: nonEmptyString(entry.control, entry.type, "text"),
            operator: nonEmptyString(entry.operator, entry.default_operator, entry.defaultOperator),
            visible: entry.visible !== false && entry.hidden !== true,
            options_from: nonEmptyString(entry.options_from, entry.optionsFrom),
            options_field: nonEmptyString(entry.options_field, entry.optionsField, entry.column, entry.key),
            options: Array.isArray(entry.options) ? entry.options : undefined,
          }))
          .filter((entry) => entry.key && entry.visible !== false)
      : [];
    return {
      fields,
      rowsetDatasetId: nonEmptyString(raw.rowset_dataset_id, raw.rowsetDatasetId),
      defaultCollapsed:
        raw.default_collapsed === true ||
        raw.defaultCollapsed === true ||
        raw.collapsed === true,
      allowExtra: raw.allow_extra === true || raw.allowExtra === true,
      title: nonEmptyString(raw.title),
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
    const previewMappingSlot = projectionSlots.find(
      (slot) =>
        slot?.mapping &&
        typeof slot.mapping === "object" &&
        !Array.isArray(slot.mapping) &&
        String(slot.component || "").trim() === "summary",
    );
    const previewMapping = previewMappingSlot?.mapping || null;
    const suppressOverlayTitle =
      Boolean(previewMapping?.preview_only || previewMapping?.previewOnly) &&
      (previewMapping?.show_header === false || previewMapping?.showHeader === false);
    const title = suppressOverlayTitle
      ? ""
      : nonEmptyString(
          popup?.title,
          detail?.label,
          defaultSlot?.label,
          metricId,
          "指标下钻",
        );
    const sceneAssembly = sceneProjectionAssembly(
      boardSceneId,
      sceneDrilldownAssemblyById(detail),
    );
    const boardSceneFile = nonEmptyString(
      detail?.board_scene_file,
      boardFields?.sceneFile,
      popup?.scene_file,
      popup?.sceneFile,
      sceneAssembly?.target_file,
      sceneAssembly?.targetFile,
    );
    const sceneLocalNav =
      normalizeSceneLocalNav(popup?.local_nav || popup?.localNav) ||
      normalizeSceneLocalNav(sceneAssembly?.local_nav || sceneAssembly?.localNav) ||
      resolveSceneLocalNav(boardSceneFile, detail?.scene_local_nav_by_target) ||
      null;
    const sceneShell =
      resolveSceneShell(sceneAssembly) ||
      normalizeSceneShellContract(
        null,
        null,
        popup?.shell_contract || popup?.shellContract,
      );
    const structuredBoard = Boolean(sceneShell?.layoutMode) && sceneShell.layoutMode !== "generic_tabs";
    const overlaySize = resolveDrilldownOverlaySize({
      popup,
      boardFields,
      structuredBoard,
      sceneShell,
    });
    const filterSchema = normalizeAnalyticsFilterSchema(
      popup?.filter_schema ||
        popup?.filterSchema ||
        sceneAssembly?.filter_schema ||
        sceneAssembly?.filterSchema,
    );
    const paramRowsetDatasetId = sceneParamRowsetDatasetId(boardFields?.params || popup?.params);
    const rawRowsetDatasetId = nonEmptyString(filterSchema.rowsetDatasetId, paramRowsetDatasetId);
    const rowsetDatasetId = structuredBoard
      ? rawRowsetDatasetId
      : qualifyDatasetIdForScene(rawRowsetDatasetId, ownerScenePath);
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
    const runtimeSceneId = structuredBoard
      ? nonEmptyString(boardSceneId, hostSceneId)
      : nonEmptyString(hostSceneId, boardSceneId);
    const runtimeSceneFile = structuredBoard
      ? nonEmptyString(boardSceneFile, ownerScenePath, detail?.host_scene_file)
      : nonEmptyString(ownerScenePath, detail?.host_scene_file, boardSceneFile);
    return {
      enabled: Boolean(boardSceneId),
      genericDrilldown: !structuredBoard || genericSceneShell,
      structuredBoard,
      sceneLocalNav,
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
      rowsetDatasetId,
      params: boardFields?.params || normalizeSceneParams(popup?.params),
      sceneId: hostSceneId,
      hostSceneId,
      hostSceneFile: nonEmptyString(ownerScenePath, detail?.host_scene_file),
      pageSceneId: nonEmptyString(detail?.page_scene_id),
      pageSceneFile: nonEmptyString(detail?.page_scene_file),
      runtimeSceneId,
      runtimeSceneFile,
      boardSceneId,
      boardSceneFile: nonEmptyString(
        boardSceneFile,
        "templates/cockpit/drilldown/generic-drilldown-board.mei",
      ),
      projection,
      title,
      note: "",
      tableMetricId: nonEmptyString(
        metricId,
        resolvePopupPassedMetricId(detail, {
          params: boardFields?.params || normalizeSceneParams(popup?.params),
          popup,
        }),
        defaultTableSlot?.metricId,
      ),
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
            by: slot.by[0] || "",
            dateField: nonEmptyString(slot.dateField, slot.trendField),
            trendField: nonEmptyString(slot.trendField, slot.dateField),
            grain: nonEmptyString(slot.grain),
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
            dateField: nonEmptyString(slot.dateField, slot.trendField),
            trendField: nonEmptyString(slot.trendField, slot.dateField),
            grain: nonEmptyString(slot.grain),
            chartKind: slot.chartKind,
            topN: slot.topN,
            valueField: slot.valueField,
            compositionAgg: slot.compositionAgg,
            mapping:
              slot.mapping && typeof slot.mapping === "object" ? slot.mapping : null,
            by: slot.by[0] || "",
            fields: slot.fields,
            compositionBy: slot.by,
            supportRole: slot.supportRole,
            column_state: slot.columnState || slot.column_state || undefined,
            pageSize: positiveInt(slot.pageSize, slot.page_size) || undefined,
            column_template: nonEmptyString(slot.columnTemplate, slot.column_template) || undefined,
            column_formats:
              slot.columnFormats && typeof slot.columnFormats === "object"
                ? slot.columnFormats
                : slot.column_formats && typeof slot.column_formats === "object"
                  ? slot.column_formats
                  : undefined,
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

  /** @deprecated Internal legacy adapter; callers must use resolveSceneOpenRequest. */
