  const drilldownRowFetchInflight = new Map();

  function drilldownFetchCacheKey(detail, config, metricId, popupFetchFilters) {
    const sceneId = nonEmptyString(config?.hostSceneId, config?.sceneId, detail?.host_scene_id);
    const datasetId = nonEmptyString(
      config?.datasetId,
      config?.rowsetDatasetId,
      resolveDrilldownDatasetId(detail, config),
    );
    const queryStateId = nonEmptyString(config?.queryStateId, detail?.query_state_id, detail?.queryStateId);
    const filterKey = JSON.stringify(popupFetchFilters || {});
    return [sceneId, datasetId, metricId, queryStateId, filterKey].join("|");
  }

  function popupDatasetFetchOptions(config, { metricId = "", previewRow = false } = {}) {
    const pageSize = resolveDrilldownFetchPageSize(config, { previewRow, clientAggregate: false });
    const dedicated = isDedicatedExplainMetricId(metricId, { supportRole: config?.supportRole });
    return {
      page: 1,
      pageSize,
      full: false,
      summary: !dedicated,
    };
  }

  async function fetchPopupDatasetRows(detail, config, datasetId) {
    const appPath = resolvePreviewAppId();
    const runtimeRefConfig = config?.runtimeRef && typeof config.runtimeRef === "object" ? config.runtimeRef : {};
    const sceneId = nonEmptyString(
      runtimeRefConfig.sceneId,
      config?.hostSceneId,
      config?.sceneId,
      detail?.host_scene_id,
      detail?.scene_id,
    );
    const target = nonEmptyString(
      runtimeRefConfig.scenePath,
      config?.hostSceneFile,
      resolveMetricOwnerScenePath(
        config?.slotByTab ? Object.values(config.slotByTab) : [],
        detail,
      ),
      detail?.host_scene_file,
      detail?.scene_path,
    );
    if (!appPath || !sceneId || !datasetId) {
      recordPopupDebugIssue({
        level: "error",
        message: "缺少 popup panel 数据查询所需的 app / scene / dataset 参数",
        phase: "dataset_fetch_setup",
        detail,
        config,
        datasetId,
      });
      return null;
    }
    const runtimeQuery = window.__meiDatasetRuntime;
    const queryStateId = nonEmptyString(config?.queryStateId, detail?.query_state_id, detail?.queryStateId);
    const sharedFilters =
      runtimeQuery &&
      typeof runtimeQuery.sharedFiltersForQueryStateId === "function" &&
      queryStateId &&
      !config?.popupFetchFilters
        ? runtimeQuery.sharedFiltersForQueryStateId(queryStateId)
        : {};
    const mergedFilters = {};
    if (sharedFilters && typeof sharedFilters === "object" && !Array.isArray(sharedFilters)) {
      Object.entries(sharedFilters).forEach(([key, value]) => {
        const normalizedKey = String(key || "").trim();
        const normalizedValue = String(value ?? "").trim();
        if (!normalizedKey || !normalizedValue) return;
        mergedFilters[normalizedKey] = normalizedValue;
      });
    }
    if (config?.popupFetchFilters && typeof config.popupFetchFilters === "object" && !Array.isArray(config.popupFetchFilters)) {
      Object.entries(config.popupFetchFilters).forEach(([key, value]) => {
        const normalizedKey = String(key || "").trim();
        const normalizedValue = String(value ?? "").trim();
        if (!normalizedKey || !normalizedValue) return;
        mergedFilters[normalizedKey] = normalizedValue;
      });
    }
    if (runtimeQuery && typeof runtimeQuery.fetchDatasetRows === "function") {
      try {
        const result = await runtimeQuery.fetchDatasetRows(
          {
            data: {
              id: String(datasetId || "").trim(),
              __mei_runtime_ref: {
                kind: "data",
                dataset_id: String(datasetId || "").trim(),
                scene_id: sceneId,
                scene_path: target,
              },
            },
            _mei: {
              runtime_capabilities: {
                rows_query: {
                  enabled: true,
                  api: `/api/datasets/query/${appPath}`,
                  scene_qualified: true,
                },
              },
              active_scene_id: sceneId,
              active_target_file: target,
              entry_target: target,
            },
          },
          {
            ...popupDatasetFetchOptions(config),
            queryStateId,
            filters: mergedFilters,
            meta: {
              component: "mei-popup-panel",
              panel_id: String(config?.panelId || "drilldown"),
              scene_id: sceneId,
              target,
              query_state_id: queryStateId || undefined,
              filter_intent_source: "drilldown",
            },
          }
        );
        if (result) {
          return {
            rows: Array.isArray(result?.rows) ? result.rows : [],
            columns: Array.isArray(result?.columns) ? result.columns : [],
            column_meta: Array.isArray(result?.column_meta) ? result.column_meta : [],
            summary: result?.summary || null,
            query_state_echo: result?.query_state_echo || null,
          };
        }
      } catch (error) {
        recordPopupDebugIssue({
          level: "error",
          message: String(error?.message || error || "popup panel runtime-query fetch failed"),
          phase: "dataset_fetch_runtime_query",
          detail,
          config,
          datasetId,
        });
        throw error;
      }
    }
    const message =
      "popup panel dataset fetch requires shared runtime-query.js; raw dataset fetch fallback has been removed";
    recordPopupDebugIssue({
      level: "error",
      message,
      phase: "dataset_fetch_runtime_missing",
      detail,
      config,
      datasetId,
    });
    throw new Error(message);
  }

  function mergePopupFetchFilters(detail, config, tableProps) {
    const merged = {};
    const rowSpecific =
      detail?.drilldown_filters &&
      typeof detail.drilldown_filters === "object" &&
      !Array.isArray(detail.drilldown_filters) &&
      Object.keys(detail.drilldown_filters).length > 0;
    const queryStateId = nonEmptyString(config?.queryStateId, detail?.query_state_id, detail?.queryStateId);
    if (!rowSpecific && queryStateId) {
      const runtimeQuery = window.__meiDatasetRuntime;
      const sharedFilters =
        runtimeQuery &&
        typeof runtimeQuery.sharedFiltersForQueryStateId === "function"
          ? runtimeQuery.sharedFiltersForQueryStateId(queryStateId)
          : {};
      if (sharedFilters && typeof sharedFilters === "object" && !Array.isArray(sharedFilters)) {
        Object.entries(sharedFilters).forEach(([key, value]) => {
          const normalizedKey = String(key || "").trim();
          const normalizedValue = String(value ?? "").trim();
          if (!normalizedKey || !normalizedValue) return;
          merged[normalizedKey] = normalizedValue;
        });
      }
    }
    [tableProps?.default_filters, detail?.default_filters, detail?.drilldown_filters].forEach((source) => {
      if (!source || typeof source !== "object" || Array.isArray(source)) return;
      Object.entries(source).forEach(([key, value]) => {
        const normalizedKey = String(key || "").trim();
        const normalizedValue = String(value ?? "").trim();
        if (!normalizedKey || !normalizedValue) return;
        merged[normalizedKey] = normalizedValue;
      });
    });
    return merged;
  }

  function hasRowDrilldownFilters(detail) {
    return Boolean(
      detail?.drilldown_filters &&
        typeof detail.drilldown_filters === "object" &&
        !Array.isArray(detail.drilldown_filters) &&
        Object.keys(detail.drilldown_filters).length > 0,
    );
  }

  async function fetchPopupDrilldownRows(detail, config) {
    const rowsetDatasetId = nonEmptyString(
      config?.rowsetDatasetId,
      config?.filterSchema?.rowsetDatasetId,
    );
    const cardMetricId = nonEmptyString(detail?.metric_id, detail?.__mei_runtime_ref?.metric_id);
    const detailSlotMetricId = nonEmptyString(
      config?.detailSlot?.metricId,
      config?.tableMetricId,
    );
    const tableMetricId = hasRowDrilldownFilters(detail)
      ? nonEmptyString(
          cardMetricId,
          detail?.__mei_runtime_ref?.metric_id,
          detailSlotMetricId,
          resolveCompositionMetricId(config, detail),
        )
      : nonEmptyString(
          detailSlotMetricId,
          config?.structuredBoard && cardMetricId ? cardMetricId : "",
          resolveCompositionMetricId(config, detail),
        );
    const detailRowsetMetricId =
      tableMetricId && !isScalarRowsetMetricId(tableMetricId)
        ? isDedicatedExplainMetricId(tableMetricId)
          ? tableMetricId
          : resolveCardMetricRowsetId(tableMetricId)
        : tableMetricId;
    const scopedConfig = detailRowsetMetricId ? { ...config, tableMetricId: detailRowsetMetricId } : config;
    const tableProps = buildDrilldownTableProps(detail, scopedConfig);
    const popupFetchFilters = mergePopupFetchFilters(detail, scopedConfig, tableProps);
    const runtimeQuery = window.__meiDatasetRuntime;
    const queryStateId = nonEmptyString(config?.queryStateId, detail?.query_state_id, detail?.queryStateId);
    if (detailRowsetMetricId && tableProps && runtimeQuery && typeof runtimeQuery.fetchDatasetRows === "function") {
      const fetchOptions = popupDatasetFetchOptions(scopedConfig, {
        metricId: detailRowsetMetricId,
        previewRow: hasRowDrilldownFilters(detail),
      });
      const inflightKey = drilldownFetchCacheKey(detail, scopedConfig, detailRowsetMetricId, popupFetchFilters);
      if (drilldownRowFetchInflight.has(inflightKey)) {
        return drilldownRowFetchInflight.get(inflightKey);
      }
      const fetchPromise = (async () => {
        try {
          const result = await runtimeQuery.fetchDatasetRows(
            {
              dataset: tableProps.dataset,
              _mei: tableProps._mei,
            },
            {
              ...fetchOptions,
              queryStateId: Object.keys(popupFetchFilters).length ? undefined : queryStateId,
              filters: popupFetchFilters,
              meta: {
                component: "mei-popup-panel",
                phase: "derived_metric_rowset",
                query_state_id: queryStateId || undefined,
                filter_intent_source: "drilldown",
              },
            },
          );
          if (result && Array.isArray(result.rows) && result.rows.length > 0) {
            return {
              rows: Array.isArray(result.rows) ? result.rows : [],
              columns: Array.isArray(result.columns) ? result.columns : [],
              column_meta: Array.isArray(result.column_meta) ? result.column_meta : [],
              summary: result?.summary || null,
              query_state_echo: result?.query_state_echo || null,
            };
          }
          return null;
        } finally {
          drilldownRowFetchInflight.delete(inflightKey);
        }
      })();
      drilldownRowFetchInflight.set(inflightKey, fetchPromise);
      try {
        const fetched = await fetchPromise;
        if (fetched) {
          return fetched;
        }
      } catch (error) {
        recordPopupDebugIssue({
          level: "error",
          message: String(error?.message || error || "popup panel metric rowset fetch failed"),
          phase: "derived_metric_rowset_fetch",
          detail,
          config,
          metricId: tableMetricId,
        });
        throw error;
      }
    }
    const datasetId = resolveDrilldownDatasetId(detail, scopedConfig);
    if (datasetId) {
      return fetchPopupDatasetRows(
        detail,
        { ...scopedConfig, datasetId, popupFetchFilters },
        datasetId,
      );
    }
    if (rowsetDatasetId) {
      return fetchPopupDatasetRows(
        detail,
        { ...scopedConfig, datasetId: rowsetDatasetId, popupFetchFilters },
        rowsetDatasetId,
      );
    }
    return { rows: [], columns: [], column_meta: [], summary: null, query_state_echo: null };
  }

