  function resolveAccessAppBasePath(pathname = window.location.pathname) {
    if (typeof appIdFromAppsPathname === "function") {
      const appId = nonEmptyString(appIdFromAppsPathname(pathname));
      // stage 路径 /apps/{app}/{stage} 与 view 路径均以 /apps/{app}/ 为应用根
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
        delimiter: nonEmptyString(entry.delimiter),
        selectionFilterEncode: nonEmptyString(
          entry.selection_filter_encode,
          entry.selectionFilterEncode,
        ),
        categoryOrder: Array.isArray(entry.category_order)
          ? entry.category_order.map((item) => String(item || "").trim()).filter(Boolean)
          : Array.isArray(entry.categoryOrder)
            ? entry.categoryOrder.map((item) => String(item || "").trim()).filter(Boolean)
            : null,
        paletteMode: nonEmptyString(entry.palette_mode, entry.paletteMode),
        yAxisInteger:
          entry.y_axis_integer === true ||
          entry.y_axis_integer === "true" ||
          entry.yAxisInteger === true ||
          entry.yAxisInteger === "true",
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

  /** Merge author bindings.filter_schema onto resolved assembly.filter_schema. */
  function mergeAnalyticsFilterSchemaPreference(resolved, author) {
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

  function normalizeAnalyticsFilterSchema(raw) {
    if (!raw || typeof raw !== "object" || Array.isArray(raw)) {
      return {
        fields: [],
        rowsetDatasetId: "",
        defaultCollapsed: false,
        allowExtra: false,
        title: "",
        presetFilterCount: undefined,
      };
    }
    const fields = Array.isArray(raw.fields)
      ? raw.fields
          .map((entry) => resolveFilterFieldEntry(entry))
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
    const presetRaw =
      raw.preset_filter_count ?? raw.presetFilterCount ?? raw.default_preset_count ?? raw.defaultPresetCount;
    const presetParsed = Number(presetRaw);
    return {
      fields,
      rowsetDatasetId: nonEmptyString(raw.rowset_dataset_id, raw.rowsetDatasetId),
      defaultCollapsed:
        raw.default_collapsed === true ||
        raw.defaultCollapsed === true ||
        raw.collapsed === true,
      allowExtra: raw.allow_extra === true || raw.allowExtra === true,
      title: nonEmptyString(raw.title),
      presetFilterCount:
        Number.isFinite(presetParsed) && presetParsed >= 0 ? Math.floor(presetParsed) : undefined,
    };
  }

  /** `filter_field(...)` 可能以 `{__call:"filter_field", __args:{...}}` IR 残留在 bindings 里。 */
  function resolveFilterFieldEntry(entry) {
    if (!entry || typeof entry !== "object" || Array.isArray(entry)) return null;
    if (entry.__call === "filter_field" && entry.__args && typeof entry.__args === "object") {
      return entry.__args;
    }
    return entry;
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
    // Prefer author fields/preset from bindings, but keep resolved string rowset_dataset_id
    // (bindings often still hold unresolved `{__ref:"param_ref"}`).
    const filterSchema = normalizeAnalyticsFilterSchema(
      mergeAnalyticsFilterSchemaPreference(
        popup?.filter_schema ||
          popup?.filterSchema ||
          sceneAssembly?.filter_schema ||
          sceneAssembly?.filterSchema,
        sceneAssembly?.bindings?.filter_schema || sceneAssembly?.bindings?.filterSchema,
      ),
    );
    const paramRowsetDatasetId = sceneParamRowsetDatasetId(boardFields?.params || popup?.params);
    const rawRowsetDatasetId = nonEmptyString(filterSchema.rowsetDatasetId, paramRowsetDatasetId);
    const rowsetDatasetId = structuredBoard
      ? rawRowsetDatasetId
      : qualifyDatasetIdForScene(rawRowsetDatasetId, ownerScenePath);
    // 与 tableMetricId 对齐：用 popup/link_decl 分析指标，勿用入口 KPI count，
    // 否则待办/在办/办结会各占一份 query_state，且与看板 fetch 键不一致。
    const boardMetricForQueryState = nonEmptyString(
      resolvePopupPassedMetricId(detail, {
        params: boardFields?.params || normalizeSceneParams(popup?.params),
        popup,
      }),
      metricId,
      defaultSlot?.metricId,
    );
    const queryStateId = structuredBoard
      ? nonEmptyString(
          popup?.query_state_id,
          popup?.queryStateId,
          boardMetricForQueryState ? `drilldown::${boardMetricForQueryState}` : "",
        )
      : "";
    const slotsByZone = groupProjectionSlotsByZone(projectionSlots);
    const defaultTableSlot =
      projectionSlots.find((slot) => slot.component === "data_table") || defaultSlot;
    const rowPreviewZone = sceneShellZonesByRole(sceneShell, "row_preview")[0] || null;
    const rowPreviewZoneId = rowPreviewZone?.id || "";
    const rowPreviewSlot = rowPreviewZoneId ? (slotsByZone[rowPreviewZoneId] || [])[0] || null : null;
    // 仅在真有 row_preview 区时才启用“点行刷新预览”；分析表不要回退成整表 selectable。
    const rowPreviewSourceZoneId = rowPreviewZone
      ? nonEmptyString(
          rowPreviewZone?.selectionSource,
          sceneShellFirstSlotZone(sceneShell, "data_table")?.id,
        )
      : "";
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
      scopeFilters: (() => {
        const params = boardFields?.params || normalizeSceneParams(popup?.params) || {};
        const fromDetail =
          detail?.scope_filters && typeof detail.scope_filters === "object" && !Array.isArray(detail.scope_filters)
            ? detail.scope_filters
            : detail?.scopeFilters && typeof detail.scopeFilters === "object" && !Array.isArray(detail.scopeFilters)
              ? detail.scopeFilters
              : null;
        if (fromDetail) return fromDetail;
        const fromParams =
          params?.scope_filters && typeof params.scope_filters === "object" && !Array.isArray(params.scope_filters)
            ? params.scope_filters
            : params?.scopeFilters && typeof params.scopeFilters === "object" && !Array.isArray(params.scopeFilters)
              ? params.scopeFilters
              : null;
        return fromParams || undefined;
      })(),
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
      // link_decl / popup.params.metric（如 issue_handling_analytics）必须优先于
      // 入口 KPI 卡片自身的 count metric（warnings_pending_count），否则看板会误拉
      // count::__scalar_rowset__ → uncovered_pipeline。
      tableMetricId: nonEmptyString(
        resolvePopupPassedMetricId(detail, {
          params: boardFields?.params || normalizeSceneParams(popup?.params),
          popup,
        }),
        metricId,
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
            selection_filter_encode: slot.selectionFilterEncode || undefined,
            category_order:
              Array.isArray(slot.categoryOrder) && slot.categoryOrder.length > 0
                ? slot.categoryOrder
                : undefined,
            palette_mode: nonEmptyString(slot.paletteMode, slot.palette_mode) || undefined,
            y_axis_integer: slot.yAxisInteger === true || slot.y_axis_integer === true || undefined,
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
              sceneId: nonEmptyString(runtimeSceneId, boardSceneId, hostSceneId),
              scenePath: nonEmptyString(
                runtimeSceneFile,
                boardSceneFile,
                ownerScenePath,
                detail?.host_scene_file,
                detail?.scene_path,
              ),
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
