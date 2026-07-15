  function isAnalyticsChartPresentation(config) {
    return (
      Boolean(config?.hasChartZone) ||
      (Array.isArray(config?.chartSlots) && config.chartSlots.length > 0)
    );
  }

  function chartMappingHasMultipleSeries(mapping) {
    if (!mapping || typeof mapping !== "object") return false;
    const y = mapping.y;
    if (Array.isArray(y)) return y.length > 1;
    return false;
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
    const mapping = overrides?.mapping && typeof overrides.mapping === "object"
      ? overrides.mapping
      : config?.mapping;
    const multiSeries = chartMappingHasMultipleSeries(mapping);
    const props = {
      compact: true,
      gridContainLabel: true,
      label_max_chars: 6,
      category_label_rotate: 30,
      showLegend: multiSeries,
      chartHeight: 300,
      color_palette: ["#38bdf8", "#34d399", "#f59e0b", "#a78bfa", "#f87171", "#facc15", "#22d3ee", "#fb7185"],
      ...overrides,
    };
    if (multiSeries && overrides.showLegend === undefined && overrides.show_legend === undefined) {
      props.showLegend = true;
    }
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

  function isAnalyticsDetailTableConfig(config = null) {
    const popupParams =
      config?.popup && typeof config.popup === "object" && !Array.isArray(config.popup)
        ? config.popup.params
        : null;
    return Boolean(
      config?.hasChartZone ||
        (Array.isArray(config?.chartSlots) && config.chartSlots.length > 0) ||
        nonEmptyString(
          config?.filterSchema?.rowsetDatasetId,
          config?.rowsetDatasetId,
          sceneParamRowsetDatasetId(config?.params),
          sceneParamRowsetDatasetId(popupParams),
        ),
    );
  }

  function resolveDeclaredRowDrilldownPopup(config = null) {
    const localNav = config?.sceneLocalNav;
    const raw =
      config?.rowDrilldownPopup ||
      config?.row_drilldown_popup ||
      localNav?.rowDrilldownPopup ||
      localNav?.row_drilldown_popup ||
      null;
    if (!raw || typeof raw !== "object" || Array.isArray(raw)) {
      return null;
    }
    return raw;
  }

  function resolveDeclaredRowDrilldownSpec(config = null) {
    const localNav = config?.sceneLocalNav;
    const raw =
      config?.rowDrilldown ||
      config?.row_drilldown ||
      localNav?.rowDrilldown ||
      localNav?.row_drilldown ||
      null;
    if (!raw || typeof raw !== "object" || Array.isArray(raw)) {
      return null;
    }
    return raw;
  }

  function popupBoardSceneFields(popup) {
    if (!popup || typeof popup !== "object") {
      return { sceneId: "", sceneFile: "" };
    }
    const sceneRef =
      popup.scene && typeof popup.scene === "object" && !Array.isArray(popup.scene)
        ? popup.scene
        : null;
    return {
      sceneId: nonEmptyString(
        popup.scene_id,
        popup.sceneId,
        sceneRef?.scene_id,
        sceneRef?.sceneId,
      ),
      sceneFile: nonEmptyString(
        popup.scene_file,
        popup.sceneFile,
        sceneRef?.scene_file,
        sceneRef?.sceneFile,
      ),
    };
  }

  function resolveAnalyticsRowsetDatasetId(config = null) {
    const popupParams =
      config?.popup && typeof config.popup === "object" && !Array.isArray(config.popup)
        ? config.popup.params
        : null;
    return nonEmptyString(
      config?.filterSchema?.rowsetDatasetId,
      config?.rowsetDatasetId,
      sceneParamRowsetDatasetId(config?.params),
      sceneParamRowsetDatasetId(popupParams),
    );
  }

  function resolveAnalyticsTableRowDrilldown(config = null, detail = null) {
    if (!isAnalyticsDetailTableConfig(config)) {
      return null;
    }
    const declaredPopup = resolveDeclaredRowDrilldownPopup(config);
    if (!declaredPopup) {
      return null;
    }
    const { sceneId: boardSceneId, sceneFile: boardSceneFile } = popupBoardSceneFields(declaredPopup);
    if (!boardSceneId || !boardSceneFile) {
      return null;
    }
    const rowsetId = resolveAnalyticsRowsetDatasetId(config);
    const localRowsetId = localDatasetIdFromSelector(rowsetId);
    const popupParams =
      declaredPopup.params && typeof declaredPopup.params === "object" && !Array.isArray(declaredPopup.params)
        ? declaredPopup.params
        : {};
    const popupMetricId = metricRefId(popupParams.metric);
    if (!popupMetricId) {
      return null;
    }
    const rowsetFromPopup = nonEmptyString(
      popupParams.rowset_dataset_id,
      popupParams.rowsetDatasetId,
      localRowsetId,
    );
    // 本地 metric id（无 `.mei::` / `__` 前缀）不要用父级 host_scene_file 做 path 前缀。
    // 否则会变成 `.../c-warnings-analytics/content.mei::supervision_models_count`，
    // 与目标 board / rowset 错位。世界 capsule 路径仍可由已带前缀的 metric id 保留。
    const alreadyScoped =
      popupMetricId.includes(".mei::") || popupMetricId.startsWith("__");
    const localMetricId = normalizeMetricLocalId(popupMetricId) || popupMetricId;
    const tableMetricId = resolveCardMetricRowsetId(
      alreadyScoped ? popupMetricId : localMetricId,
    );
    const runtimeRef = {
      kind: "metric",
      metric_id: tableMetricId,
      dataset_id: nonEmptyString(
        rowsetFromPopup,
        qualifyDatasetIdForScene(
          rowsetFromPopup,
          nonEmptyString(boardSceneFile, config?.hostSceneFile),
        ),
      ),
      scene_id: boardSceneId,
      scene_path: boardSceneFile,
    };
    const overlaySize = nonEmptyString(
      declaredPopup.overlay_size,
      declaredPopup.overlaySize,
      config?.sceneLocalNav?.overlaySize,
      "large",
    );
    const overlayWorkspace =
      (declaredPopup.overlay_workspace &&
        typeof declaredPopup.overlay_workspace === "object" &&
        !Array.isArray(declaredPopup.overlay_workspace) &&
        declaredPopup.overlay_workspace) ||
      (declaredPopup.overlayWorkspace &&
        typeof declaredPopup.overlayWorkspace === "object" &&
        !Array.isArray(declaredPopup.overlayWorkspace) &&
        declaredPopup.overlayWorkspace) ||
      null;
    const popupTitle = nonEmptyString(declaredPopup.title, declaredPopup.label);
    return {
      popup: {
        mode: "popup",
        type: nonEmptyString(declaredPopup.type, "popup"),
        projection: nonEmptyString(declaredPopup.projection, "overlay"),
        overlay_size: overlaySize,
        ...(overlayWorkspace ? { overlay_workspace: overlayWorkspace } : {}),
        ...(popupTitle ? { title: popupTitle } : {}),
        scene_id: boardSceneId,
        scene_file: boardSceneFile,
        params: {
          ...popupParams,
          metric: { __mei_runtime_ref: runtimeRef },
          rowset_dataset_id: rowsetFromPopup,
        },
      },
      drilldownMetric: { __mei_runtime_ref: runtimeRef },
      previewCompileAnchor: {
        sceneId: boardSceneId,
        scenePath: boardSceneFile,
        ownerScenePath: nonEmptyString(
          importedCapsuleScenePathFromMetricId(popupMetricId),
          boardSceneFile,
        ),
      },
      rowDrilldown: resolveDeclaredRowDrilldownSpec(config),
    };
  }

  function applyAnalyticsTableRowDrilldown(props, config, detail = null) {
    if (!props) {
      return props;
    }
    const rowDrilldown = resolveAnalyticsTableRowDrilldown(config, detail);
    if (!rowDrilldown) {
      return props;
    }
    return {
      ...props,
      popup: rowDrilldown.popup,
      drilldownMetric: rowDrilldown.drilldownMetric,
      previewCompileAnchor: rowDrilldown.previewCompileAnchor,
      rowDrilldown: rowDrilldown.rowDrilldown || undefined,
      row_drilldown: rowDrilldown.rowDrilldown || undefined,
    };
  }

  /** analytics board 明细表/翻页应绑定 board scene，而非 home host scene。 */
  function resolveAnalyticsBoardQuerySceneId(config, detail, runtimeRefConfig = {}) {
    const boardSceneId = nonEmptyString(
      config?.boardSceneId,
      config?.pageSceneId,
      config?.runtimeSceneId,
    );
    if (config?.structuredBoard && boardSceneId) {
      return boardSceneId;
    }
    return nonEmptyString(
      config?.runtimeSceneId,
      runtimeRefConfig.sceneId,
      runtimeRefConfig.scene_id,
      config?.hostSceneId,
      config?.sceneId,
      detail?.host_scene_id,
      detail?.scene_id,
      resolveDrilldownSceneId(detail, runtimeDrilldownConfig(detail)),
    );
  }

  function resolveAnalyticsBoardQueryScenePath(config, detail, runtimeRefConfig = {}) {
    const boardSceneFile = nonEmptyString(
      config?.boardSceneFile,
      config?.pageSceneFile,
      config?.runtimeSceneFile,
    );
    if (config?.structuredBoard && boardSceneFile) {
      return boardSceneFile;
    }
    return nonEmptyString(
      config?.runtimeSceneFile,
      runtimeRefConfig.scenePath,
      runtimeRefConfig.scene_path,
      config?.hostSceneFile,
      detail?.host_scene_file,
      detail?.scene_path,
    );
  }

  function buildDrilldownTableProps(detail, config) {
    const runtimeRefConfig = config?.runtimeRef && typeof config.runtimeRef === "object" ? config.runtimeRef : {};
    const queryStateId = nonEmptyString(config?.queryStateId, detail?.query_state_id, detail?.queryStateId);
    const sceneId = resolveAnalyticsBoardQuerySceneId(config, detail, runtimeRefConfig);
    if (!sceneId) return null;
    const appPath = resolvePreviewAppId();
    if (!appPath) return null;
    const datasetId = resolveDrilldownDatasetId(detail, config);
    if (!datasetId) return null;
    const metricId = resolveDrilldownDetailTableMetricId(config, detail);
    const scenePathMetricId = nonEmptyString(
      detail?.metric_id,
      detail?.__mei_runtime_ref?.metric_id,
      normalizeMetricLocalId(metricId),
      metricId,
    );
    const preferredScenePath = resolveAnalyticsBoardQueryScenePath(config, detail, runtimeRefConfig);
    const ownerScenePath = nonEmptyString(
      preferredScenePath,
      config?.runtimeSceneFile,
      importedCapsuleScenePathFromMetricId(scenePathMetricId),
      importedCapsuleScenePathFromWorldMetricsDatasetId(datasetId),
      importedCapsuleScenePathFromMetricId(detail?.metric_id),
      importedCapsuleScenePathFromWorldMetricsDatasetId(detail?.dataset_id),
      resolveMetricOwnerScenePath(
        config?.slotByTab ? Object.values(config.slotByTab) : [],
        detail,
      ),
      detail?.host_scene_file,
      detail?.scene_path,
      config?.hostSceneFile,
    );
    const previewAnchor = config?.previewCompileAnchor;
    const resolvedSceneId = nonEmptyString(previewAnchor?.sceneId, sceneId);
    const resolvedScenePath = nonEmptyString(previewAnchor?.scenePath, ownerScenePath);
    const previewScope = nonEmptyString(
      config?.previewScope,
      config?.preview_scope,
      detail?.preview_scope,
      detail?._mei?.preview_scope,
      config?.pageSceneId,
      config?.boardSceneId,
      resolvedSceneId,
    );
    const runtimeRef = metricId
      ? {
          kind: "metric",
          scene_id: resolvedSceneId,
          scene_path: resolvedScenePath,
          dataset_id: datasetId,
          metric_id: metricId,
        }
      : {
          kind: "data",
          scene_id: resolvedSceneId,
          scene_path: resolvedScenePath,
          dataset_id: datasetId,
        };
    const columns = Array.isArray(config?.columns) ? config.columns : [];
    const tableScrollX =
      config?.tableScrollX === true ||
      config?.table_scroll_x === true ||
      columns.length >= 7;
    const inferredFormats = inferDrilldownColumnFormats(columns);
    const inferredColumnState = inferDrilldownColumnState(columns);
    const explicitColumnState =
      config?.columnState && typeof config.columnState === "object"
        ? config.columnState
        : config?.column_state && typeof config.column_state === "object"
          ? config.column_state
          : null;
    const hasExplicitColumnState = Array.isArray(explicitColumnState?.columns) && explicitColumnState.columns.length > 0;
    const explicitColumnFormats =
      config?.columnFormats && typeof config.columnFormats === "object"
        ? config.columnFormats
        : config?.column_formats && typeof config.column_formats === "object"
          ? config.column_formats
          : null;
    const columnFormats = explicitColumnFormats
      ? { ...inferredFormats, ...explicitColumnFormats }
      : inferredFormats;
    const columnState = hasExplicitColumnState ? explicitColumnState : inferredColumnState;
    const columnTemplate = nonEmptyString(config?.column_template, config?.columnTemplate);
    const hasExplicitLayout = Boolean(columnTemplate) || hasExplicitColumnState;
    const columnMinWidth =
      Number(config?.columnMinWidth) > 0
        ? Number(config.columnMinWidth)
        : tableScrollX
          ? 96
          : 64;
    const drilldownFilters =
      detail?.drilldown_filters && typeof detail.drilldown_filters === "object" && !Array.isArray(detail.drilldown_filters)
        ? detail.drilldown_filters
        : detail?.default_filters && typeof detail.default_filters === "object" && !Array.isArray(detail.default_filters)
          ? detail.default_filters
          : null;
    const autoSelectFirstRow = Boolean(
      drilldownFilters &&
        (config?.hasRowPreviewZone ||
          nonEmptyString(config?.rowPreviewZoneId, config?.row_preview_zone_id)),
    );
    const staticMode = isStaticDataMode();
    const staticRows = staticMode ? buildStaticTableRows(columns) : [];
    const tableProps = {
      columns,
      headers: Array.isArray(config?.headers) && config.headers.length > 0 ? config.headers : undefined,
      column_state: columnState,
      column_template: columnTemplate || undefined,
      layoutPreset: tableScrollX ? "" : config?.layoutPreset || "default",
      default_filters: drilldownFilters || undefined,
      embedded: true,
      autoSelectFirstRow: autoSelectFirstRow || undefined,
      rowSelectionMode:
        nonEmptyString(config?.rowSelectionMode) || (autoSelectFirstRow ? "single" : ""),
      tableScrollX,
      autoFitColumns: hasExplicitLayout ? false : true,
      fitColumnsFromSample: hasExplicitLayout ? false : true,
      columnWidthSampleSize: 100,
      cellOverflowMinChars: 10,
      pageSize: Number(config?.pageSize ?? config?.page_size) > 0 ? Number(config?.pageSize ?? config?.page_size) : 8,
      cellPreviewMaxChars:
        Number.isFinite(Number(config?.cellPreviewMaxChars)) && Number(config?.cellPreviewMaxChars) >= 0
          ? Number(config.cellPreviewMaxChars)
          : 0,
      columnMinWidth,
      columnFormats,
      pagination: true,
      paginationMode: staticMode ? "client" : metricId ? "server" : "client",
      dataset: {
        shape: metricId ? "dataframe" : "table",
        __mei_runtime_ref: runtimeRef,
        ...(staticMode
          ? {
              columns,
              rows: staticRows,
              __mei_data_origin: "static_skeleton",
            }
          : {}),
      },
      _mei: {
        runtime_capabilities: {
          rows_query: {
            enabled: !staticMode,
            api: `/api/datasets/query/${appPath}`,
            scene_qualified: true,
          },
          metric_query: {
            enabled: !staticMode,
            api: `/api/datasets/metrics/${appPath}`,
            scene_qualified: true,
          },
          metric_batch_query: {
            enabled: !staticMode,
            api: `/api/datasets/metrics/${appPath}`,
            scene_qualified: true,
          },
          ...(staticMode
            ? {
                static_display: {
                  enabled: true,
                  origin: "static_skeleton",
                },
              }
            : {}),
        },
        active_scene_id: resolvedSceneId,
        active_target_file: resolvedScenePath,
        entry_target: resolvedScenePath,
        preview_scope: previewScope,
      },
      query_state: queryStateId || undefined,
    };
    return tableProps;
  }

