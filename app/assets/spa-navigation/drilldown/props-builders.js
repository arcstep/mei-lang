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

  const SPBJW_CASE_DETAIL_BOARD_FILE = "scenes/_shared/case-detail.board.mei";
  const SPBJW_WARNING_ROWSET_IDS = new Set(["warning_list", "warning_detail"]);

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

  function resolveCaseDetailBoardSceneId(rowsetId) {
    const id = String(rowsetId || "").trim();
    if (!id) return "";
    if (id === "issue_result_list") return "issue_result_detail_card_board";
    if (SPBJW_WARNING_ROWSET_IDS.has(id)) return "warning_detail_card_board";
    return "";
  }

  function resolveAnalyticsTableRowDrilldown(config = null) {
    if (!isAnalyticsDetailTableConfig(config)) {
      return null;
    }
    const rowsetId = resolveAnalyticsRowsetDatasetId(config);
    const boardSceneId = resolveCaseDetailBoardSceneId(rowsetId);
    if (!boardSceneId) {
      return null;
    }
    const metricId = nonEmptyString(config?.tableMetricId);
    const sceneId = nonEmptyString(config?.hostSceneId, config?.sceneId);
    const scenePath = nonEmptyString(
      config?.detailSlot?.runtimeRef?.scenePath,
      config?.detailSlot?.runtimeRef?.scene_path,
      importedCapsuleScenePathFromMetricId(metricId),
      resolveMetricOwnerScenePath(
        config?.detailSlot ? [config.detailSlot] : [],
        { metric_id: metricId, dataset_id: rowsetId, host_scene_file: config?.hostSceneFile },
      ),
      String(config?.hostSceneFile || "").replace(/\.board\.mei$/i, ".mei"),
      config?.hostSceneFile,
    );
    if (!metricId || !sceneId || !scenePath) {
      return null;
    }
    const runtimeRef = {
      kind: "metric",
      metric_id: metricId,
      dataset_id: rowsetId,
      scene_id: sceneId,
      scene_path: scenePath,
    };
    return {
      popup: {
        mode: "popup",
        type: "popup",
        projection: "overlay",
        overlay_size: "fullscreen",
        scene_id: boardSceneId,
        scene_file: SPBJW_CASE_DETAIL_BOARD_FILE,
        params: {
          metric: { __mei_runtime_ref: runtimeRef },
          rowset_dataset_id: rowsetId,
        },
      },
      drilldownMetric: { __mei_runtime_ref: runtimeRef },
    };
  }

  function applyAnalyticsTableRowDrilldown(props, config) {
    if (!props) {
      return props;
    }
    const rowDrilldown = resolveAnalyticsTableRowDrilldown(config);
    if (!rowDrilldown) {
      return props;
    }
    return {
      ...props,
      popup: rowDrilldown.popup,
      drilldownMetric: rowDrilldown.drilldownMetric,
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
    const datasetId = resolveDrilldownDatasetId(detail, config);
    if (!datasetId) return null;
    const metricId = nonEmptyString(
      config?.tableMetricId,
      runtimeRefConfig.metricId,
      runtimeRefConfig.metric_id,
      detail?.metric_id,
      detail?.__mei_runtime_ref?.metric_id,
    );
    const scenePathMetricId = nonEmptyString(
      detail?.metric_id,
      detail?.__mei_runtime_ref?.metric_id,
      normalizeMetricLocalId(metricId),
      metricId,
    );
    const ownerScenePath = nonEmptyString(
      runtimeRefConfig.scenePath,
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
          ? 88
          : 56;
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
    return {
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
        Number(config?.cellPreviewMaxChars) > 0
          ? Number(config.cellPreviewMaxChars)
          : tableScrollX
            ? 20
            : 28,
      columnMinWidth,
      columnFormats,
      pagination: true,
      paginationMode: metricId ? "server" : "client",
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

