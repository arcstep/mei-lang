  const drilldownRowFetchInflight = new Map();

  function resolveDrilldownSharedFilters(queryStateId) {
    const id = nonEmptyString(queryStateId);
    if (!id) return {};
    const runtimeQuery = window.__meiDatasetRuntime;
    if (runtimeQuery && typeof runtimeQuery.sharedFiltersForQueryStateId === "function") {
      const shared = runtimeQuery.sharedFiltersForQueryStateId(id);
      return shared && typeof shared === "object" && !Array.isArray(shared) ? shared : {};
    }
    return {};
  }

  /** query_state 已绑定但尚未写入任何筛选时，构成/趋势图应走服务端 explain 指标而非分页 rowset 重聚合。 */
  function hasActiveDrilldownQueryFilters(queryStateId) {
    const id = nonEmptyString(queryStateId);
    if (!id) return false;
    const filters = resolveDrilldownSharedFilters(id);
    if (
      Object.values(filters).some((value) => String(value ?? "").trim())
    ) {
      return true;
    }
    const runtimeQuery = window.__meiDatasetRuntime;
    if (runtimeQuery && typeof runtimeQuery.sharedFilterIntentsForQueryStateId === "function") {
      const intents = runtimeQuery.sharedFilterIntentsForQueryStateId(id);
      if (
        Array.isArray(intents) &&
        intents.some((entry) => {
          if (!entry || typeof entry !== "object") return false;
          const dimension = String(entry.dimension || entry.field || "").trim();
          const value = String(entry.value ?? "").trim();
          return Boolean(dimension && value);
        })
      ) {
        return true;
      }
    }
    if (runtimeQuery && typeof runtimeQuery.sharedSearchForQueryStateId === "function") {
      if (String(runtimeQuery.sharedSearchForQueryStateId(id) || "").trim()) {
        return true;
      }
    }
    return false;
  }

  function drilldownFetchCacheKey(detail, config, metricId, popupFetchFilters) {
    const sceneId = nonEmptyString(
      config?.runtimeSceneId,
      config?.hostSceneId,
      config?.sceneId,
      detail?.host_scene_id,
    );
    const datasetId = nonEmptyString(
      config?.datasetId,
      config?.rowsetDatasetId,
      resolveDrilldownDatasetId(detail, config),
    );
    const queryStateId = nonEmptyString(config?.queryStateId, detail?.query_state_id, detail?.queryStateId);
    const filterKey = JSON.stringify(popupFetchFilters || {});
    return [sceneId, datasetId, metricId, queryStateId, filterKey].join("|");
  }

  function popupDatasetFetchOptions(config, { metricId = "", previewRow = false, clientAggregate = false } = {}) {
    const pageSize = resolveDrilldownFetchPageSize(config, {
      previewRow,
      clientAggregate: clientAggregate || config?.clientAggregate === true,
    });
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
      config?.runtimeSceneId,
      config?.hostSceneId,
      config?.sceneId,
      detail?.host_scene_id,
      detail?.scene_id,
    );
    const target = nonEmptyString(
      runtimeRefConfig.scenePath,
      config?.runtimeSceneFile,
      config?.hostSceneFile,
      resolveMetricOwnerScenePath(
        config?.slotByTab ? Object.values(config.slotByTab) : [],
        detail,
      ),
      detail?.host_scene_file,
      detail?.scene_path,
    );
    const previewScope = nonEmptyString(
      config?.previewScope,
      config?.preview_scope,
      detail?.preview_scope,
      detail?._mei?.preview_scope,
      config?.pageSceneId,
      config?.boardSceneId,
      sceneId,
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
              preview_scope: previewScope,
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

  function applyFilterMap(target, source) {
    if (!source || typeof source !== "object" || Array.isArray(source)) return;
    Object.entries(source).forEach(([key, value]) => {
      const normalizedKey = String(key || "").trim();
      const normalizedValue = String(value ?? "").trim();
      if (!normalizedKey || !normalizedValue) return;
      target[normalizedKey] = normalizedValue;
    });
  }

  /**
   * 合并 popup / drilldown 拉取用 filters。优先级（后写覆盖）：
   * 1. 入口 default_filters（仅 query_state 尚未写入时作种子）
   * 2. query_state（过滤面板用户覆盖；一旦有值则不再回填 default）
   * 3. drilldown_filters（行级对象打开的身份锁，不可被面板清掉）
   * 与 024005：default_filters 不是第二套真值，只作初始化。
   */
  function mergePopupFetchFilters(detail, config, tableProps) {
    const merged = {};
    const popupParams =
      config?.popup && typeof config.popup === "object" && !Array.isArray(config.popup)
        ? config.popup.params
        : null;
    const queryStateId = nonEmptyString(config?.queryStateId, detail?.query_state_id, detail?.queryStateId);
    let sharedFilters = {};
    if (queryStateId) {
      const runtimeQuery = window.__meiDatasetRuntime;
      const shared =
        runtimeQuery &&
        typeof runtimeQuery.sharedFiltersForQueryStateId === "function"
          ? runtimeQuery.sharedFiltersForQueryStateId(queryStateId)
          : {};
      if (shared && typeof shared === "object" && !Array.isArray(shared)) {
        sharedFilters = shared;
      }
    }
    const hasSharedFilters = Object.keys(sharedFilters).some(
      (key) => String(sharedFilters[key] ?? "").trim(),
    );
    if (!hasSharedFilters) {
      applyFilterMap(merged, popupParams?.default_filters);
      applyFilterMap(merged, config?.params?.default_filters);
      applyFilterMap(merged, detail?.default_filters);
      applyFilterMap(merged, tableProps?.default_filters);
    }
    applyFilterMap(merged, sharedFilters);

    applyFilterMap(merged, detail?.drilldown_filters);

    // Also emit column-name aliases so dataset bindings that only expose 预警ID / 处理结果ID / 序号 / 模型ID resolve.
    if (merged.warningId && !merged["预警ID"]) merged["预警ID"] = merged.warningId;
    if (merged["预警ID"] && !merged.warningId) merged.warningId = merged["预警ID"];
    if (merged.resultId && !merged["处理结果ID"]) merged["处理结果ID"] = merged.resultId;
    if (merged["处理结果ID"] && !merged.resultId) merged.resultId = merged["处理结果ID"];
    if (merged.matterId && !merged["序号"]) merged["序号"] = merged.matterId;
    if (merged["序号"] && !merged.matterId) merged.matterId = merged["序号"];
    if (merged.modelId && !merged["模型ID"]) merged["模型ID"] = merged.modelId;
    if (merged["模型ID"] && !merged.modelId) merged.modelId = merged["模型ID"];
    return merged;
  }

  /** Excel/Parquet 常把整型 ID 存成 2025001 / "2025001.0"；比较与展示时归一成无小数文本。 */
  function normalizeIdentityText(value) {
    if (value == null) return "";
    if (typeof value === "number" && Number.isFinite(value)) {
      if (Math.abs(value % 1) < Number.EPSILON) return String(Math.trunc(value));
      return String(value);
    }
    const text = String(value).trim();
    if (!text) return "";
    if (/^-?\d+\.0+$/.test(text)) return text.replace(/\.0+$/, "");
    return text;
  }

  function identityTextEquals(left, right) {
    const a = normalizeIdentityText(left);
    const b = normalizeIdentityText(right);
    return Boolean(a) && a === b;
  }

  function pickRowMatchingDrilldownFilters(rows, detail) {
    const list = Array.isArray(rows) ? rows : [];
    if (!list.length) return null;
    const filters =
      detail?.drilldown_filters && typeof detail.drilldown_filters === "object"
        ? detail.drilldown_filters
        : detail?.default_filters && typeof detail.default_filters === "object"
          ? detail.default_filters
          : {};
    const warningId = normalizeIdentityText(filters.warningId ?? filters["预警ID"]);
    const resultId = normalizeIdentityText(filters.resultId ?? filters["处理结果ID"]);
    const matterId = normalizeIdentityText(filters.matterId ?? filters["序号"]);
    const modelId = normalizeIdentityText(filters.modelId ?? filters["模型ID"]);
    const matterName = String(filters.matter ?? filters["风险事项"] ?? filters["监督事项"] ?? "").trim();
    if (warningId) {
      const hit = list.find((row) =>
        identityTextEquals(row?.["预警ID"] ?? row?.warning_id ?? row?.warningId, warningId),
      );
      if (hit) return hit;
    }
    if (resultId) {
      const hit = list.find((row) =>
        identityTextEquals(row?.["处理结果ID"] ?? row?.result_id ?? row?.resultId, resultId),
      );
      if (hit) return hit;
    }
    if (modelId) {
      const hit = list.find((row) =>
        identityTextEquals(row?.["模型ID"] ?? row?.model_id ?? row?.modelId, modelId),
      );
      if (hit) return hit;
    }
    if (matterId) {
      const hit = list.find((row) =>
        identityTextEquals(row?.["序号"] ?? row?.matterId ?? row?.seq, matterId),
      );
      if (hit) return hit;
    }
    if (matterName) {
      const hit = list.find((row) => {
        const name = String(row?.["风险事项"] ?? row?.["监督事项"] ?? row?.matter ?? "").trim();
        return name === matterName;
      });
      if (hit) return hit;
    }
    return list[0] || null;
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
    const previewOnlyFetch =
      isPreviewOnlyMapping(config) ||
      isSheetDetailCardPreview(config) ||
      isTypicalCaseCardPreview(config);
    // Row drilldown from L1/L2 host tables sets page_scene_* to the caller scene while
    // board_scene_* / popup.scene_* point at the detail page (e.g. warning_detail_page).
    // Preview-only cards must compile against the board scene or row filters are dropped.
    const previewCompileSceneId = nonEmptyString(
      detail?.board_scene_id,
      detail?.boardSceneId,
      detail?.popup?.scene_id,
      detail?.popup?.sceneId,
      config?.boardSceneId,
      config?.previewCompileAnchor?.sceneId,
      detail?.page_scene_id,
      detail?.pageSceneId,
      config?.pageSceneId,
    );
    const previewCompileScenePath = nonEmptyString(
      detail?.board_scene_file,
      detail?.boardSceneFile,
      detail?.popup?.scene_file,
      detail?.popup?.sceneFile,
      config?.boardSceneFile,
      config?.previewCompileAnchor?.scenePath,
      detail?.page_scene_file,
      detail?.pageSceneFile,
      config?.pageSceneFile,
    );
    const metricFetchConfig =
      previewOnlyFetch && previewCompileScenePath && previewCompileSceneId
        ? {
            ...config,
            structuredBoard: false,
            previewCompileAnchor: {
              sceneId: previewCompileSceneId,
              scenePath: previewCompileScenePath,
            },
          }
        : config;
    const passedMetricId = resolvePopupPassedMetricId(detail, config);
    const cardMetricId = nonEmptyString(
      passedMetricId,
      resolveDrilldownTableMetricId(detail, config),
      detail?.metric_id,
      detail?.__mei_runtime_ref?.metric_id,
    );
    const detailSlotMetricId = nonEmptyString(config?.detailSlot?.metricId);
    const tableMetricId = hasRowDrilldownFilters(detail)
      ? nonEmptyString(
          cardMetricId,
          detailSlotMetricId,
          resolveCompositionMetricId(config, detail),
        )
      : nonEmptyString(
          cardMetricId,
          resolveCompositionMetricId(config, detail),
          detailSlotMetricId,
        );
    const detailRowsetMetricId =
      tableMetricId && !isScalarRowsetMetricId(tableMetricId)
        ? isDedicatedExplainMetricId(tableMetricId)
          ? tableMetricId
          : resolveCardMetricRowsetId(tableMetricId)
        : tableMetricId;
    const scopedConfig = detailRowsetMetricId ? { ...metricFetchConfig, tableMetricId: detailRowsetMetricId } : metricFetchConfig;
    const tableProps = buildDrilldownTableProps(detail, scopedConfig);
    const popupFetchFilters = mergePopupFetchFilters(detail, scopedConfig, tableProps);
    const runtimeQuery = window.__meiDatasetRuntime;
    const queryStateId = nonEmptyString(config?.queryStateId, detail?.query_state_id, detail?.queryStateId);
    if (detailRowsetMetricId && tableProps && runtimeQuery && typeof runtimeQuery.fetchDatasetRows === "function") {
      const fetchOptions = popupDatasetFetchOptions(scopedConfig, {
        metricId: detailRowsetMetricId,
        previewRow: hasRowDrilldownFilters(detail),
        clientAggregate: scopedConfig.clientAggregate === true,
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

