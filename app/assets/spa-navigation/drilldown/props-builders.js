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

  const SPBJW_WARNING_DETAIL_BOARD_FILE = "scenes/_shared/warning-detail.card.board.mei";
  const SPBJW_TYPICAL_CASES_BOARD_FILE = "scenes/09-监督典型案例.board.mei";
  const SPBJW_ISSUE_CLUE_DETAIL_BOARD_FILE = "scenes/_shared/issue-clue-detail.card.board.mei";
  const SPBJW_ISSUE_HANDLING_DETAIL_BOARD_FILE = "scenes/_shared/issue-clue-detail.card.board.mei";
  const SPBJW_ISSUE_RESULT_DETAIL_BOARD_FILE = "scenes/_shared/issue-result-detail.card.board.mei";
  const SPBJW_WARNING_ROWSET_IDS = new Set(["warning_list", "warning_detail"]);
  const SPBJW_ISSUE_HANDLING_METRIC_IDS = new Set([
    "warnings_pending_count",
    "effectiveness_in_progress_count",
    "effectiveness_completed_count",
  ]);
  const SPBJW_ISSUE_CLUE_METRIC_IDS = new Set([
    "effectiveness_transfer_clue_count",
    "effectiveness_filing_count",
  ]);

  function resolveDetailCardOwnerSceneFile(boardSceneId) {
    const id = String(boardSceneId || "").trim();
    if (id === "issue_handling_detail_card_board") {
      return "scenes/07-问题办理.mei";
    }
    if (id === "issue_clue_detail_card_board") {
      return "scenes/08-监督成效.mei";
    }
    return "";
  }

  function resolveDetailCardScopedMetricId(metricId, boardSceneId) {
    const local = normalizeMetricLocalId(metricId);
    if (!local) return "";
    const ownerScene = resolveDetailCardOwnerSceneFile(boardSceneId);
    if (ownerScene) {
      return `${ownerScene}::${local}`;
    }
    return nonEmptyString(metricId, local);
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

  function resolveCaseDetailBoardTarget(rowsetId, detail = null, config = null) {
    const id = localDatasetIdFromSelector(rowsetId);
    if (!id) return null;
    if (id === "issue_result_list") {
      return {
        sceneId: "issue_result_detail_card_board",
        sceneFile: SPBJW_ISSUE_RESULT_DETAIL_BOARD_FILE,
      };
    }
    if (id === "typical_cases") {
      return {
        sceneId: "typical_cases_detail_board",
        sceneFile: SPBJW_TYPICAL_CASES_BOARD_FILE,
      };
    }
    if (SPBJW_WARNING_ROWSET_IDS.has(id)) {
      const metricId = normalizeMetricLocalId(
        resolveDrilldownTableMetricId(detail, config),
      );
      if (SPBJW_ISSUE_HANDLING_METRIC_IDS.has(metricId)) {
        return {
          sceneId: "issue_handling_detail_card_board",
          sceneFile: SPBJW_ISSUE_HANDLING_DETAIL_BOARD_FILE,
        };
      }
      if (SPBJW_ISSUE_CLUE_METRIC_IDS.has(metricId)) {
        return {
          sceneId: "issue_clue_detail_card_board",
          sceneFile: SPBJW_ISSUE_CLUE_DETAIL_BOARD_FILE,
        };
      }
      return {
        sceneId: "warning_detail_card_board",
        sceneFile: SPBJW_WARNING_DETAIL_BOARD_FILE,
      };
    }
    return null;
  }

  function resolveAnalyticsTableRowDrilldown(config = null, detail = null) {
    if (!isAnalyticsDetailTableConfig(config)) {
      return null;
    }
    const rowsetId = resolveAnalyticsRowsetDatasetId(config);
    const localRowsetId = localDatasetIdFromSelector(rowsetId);
    const boardTarget = resolveCaseDetailBoardTarget(rowsetId, detail, config);
    if (!boardTarget?.sceneId) {
      return null;
    }
    const metricId = resolveDrilldownTableMetricId(detail, config);
    const boardSceneId = boardTarget.sceneId;
    const boardSceneFile = nonEmptyString(boardTarget.sceneFile);
    const ownerScenePath = nonEmptyString(
      resolveDetailCardOwnerSceneFile(boardSceneId),
      importedCapsuleScenePathFromMetricId(metricId),
      resolveMetricOwnerScenePath(
        config?.detailSlot ? [config.detailSlot] : [],
        { metric_id: metricId, dataset_id: localRowsetId, host_scene_file: config?.hostSceneFile },
      ),
      String(config?.hostSceneFile || "").replace(/\.board\.mei$/i, ".mei"),
      config?.hostSceneFile,
    );
    const scopedMetricId = resolveDetailCardScopedMetricId(metricId, boardSceneId);
    if (!scopedMetricId || !boardSceneId || !boardSceneFile) {
      return null;
    }
    const runtimeRef = {
      kind: "metric",
      metric_id: scopedMetricId,
      dataset_id: localRowsetId,
      scene_id: boardSceneId,
      scene_path: boardSceneFile,
    };
    return {
      popup: {
        mode: "popup",
        type: "popup",
        projection: "overlay",
        overlay_size: "fullscreen",
        scene_id: boardSceneId,
        scene_file: boardSceneFile,
        params: {
          metric: { __mei_runtime_ref: runtimeRef },
          rowset_dataset_id: localRowsetId,
        },
      },
      drilldownMetric: { __mei_runtime_ref: runtimeRef },
      previewCompileAnchor: {
        sceneId: boardSceneId,
        scenePath: boardSceneFile,
        ownerScenePath,
      },
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
    };
  }

  function buildDrilldownTableProps(detail, config) {
    const runtimeRefConfig = config?.runtimeRef && typeof config.runtimeRef === "object" ? config.runtimeRef : {};
    const queryStateId = nonEmptyString(config?.queryStateId, detail?.query_state_id, detail?.queryStateId);
    const preferredSceneId = config?.structuredBoard
      ? nonEmptyString(config?.runtimeSceneId, runtimeRefConfig.sceneId)
      : runtimeRefConfig.sceneId;
    const sceneId = nonEmptyString(
      preferredSceneId,
      config?.runtimeSceneId,
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
    const metricId = resolveDrilldownDetailTableMetricId(config, detail);
    const scenePathMetricId = nonEmptyString(
      detail?.metric_id,
      detail?.__mei_runtime_ref?.metric_id,
      normalizeMetricLocalId(metricId),
      metricId,
    );
    const preferredScenePath = config?.structuredBoard
      ? nonEmptyString(config?.runtimeSceneFile, runtimeRefConfig.scenePath)
      : runtimeRefConfig.scenePath;
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
    const resolvedSceneId = nonEmptyString(
      previewAnchor?.sceneId,
      config?.boardSceneId,
      sceneId,
    );
    const resolvedScenePath = nonEmptyString(
      previewAnchor?.scenePath,
      config?.boardSceneFile,
      ownerScenePath,
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
        Number.isFinite(Number(config?.cellPreviewMaxChars)) && Number(config?.cellPreviewMaxChars) >= 0
          ? Number(config.cellPreviewMaxChars)
          : 0,
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
        active_scene_id: resolvedSceneId,
        active_target_file: resolvedScenePath,
        entry_target: resolvedScenePath,
      },
      query_state: queryStateId || undefined,
    };
  }

