  function nonEmptyString(...values) {
    for (const value of values) {
      // Skip unresolved IR (param_ref / maps); String({}) === "[object Object]".
      if (value == null || typeof value === "object") continue;
      const text = String(value).trim();
      if (text) return text;
    }
    return "";
  }

  function isWorldMetricsOwnerDatasetId(datasetId) {
    const id = String(datasetId || "").trim();
    return id === "__world_metrics__" || id.startsWith("__world_metrics__::");
  }

  function importedCapsuleScenePathFromWorldMetricsDatasetId(datasetId) {
    const text = String(datasetId || "").trim();
    const prefix = "__world_metrics__::";
    const suffix = "::metrics";
    if (!text.startsWith(prefix) || !text.endsWith(suffix)) {
      return "";
    }
    return text.slice(prefix.length, text.length - suffix.length);
  }

  function importedCapsuleScenePathFromMetricId(metricId) {
    const text = String(metricId || "").trim();
    const marker = ".mei::";
    const idx = text.indexOf(marker);
    if (idx <= 0) {
      return "";
    }
    return text.slice(0, idx + 4);
  }

  function localDatasetIdFromSelector(datasetId) {
    const text = String(datasetId || "").trim();
    if (!text) return "";
    const parts = text.split("::").filter(Boolean);
    return parts.length > 0 ? parts[parts.length - 1] : text;
  }

  function qualifyDatasetIdForScene(datasetId, scenePath) {
    const id = String(datasetId || "").trim();
    if (!id || id.includes("::") || id.startsWith("__")) return id;
    const scene = String(scenePath || "")
      .trim()
      .replace(/\\/g, "/")
      .replace(/\.board\.mei$/i, ".mei");
    if (!scene.startsWith("scenes/") || !scene.endsWith(".mei")) return id;
    return `${scene}::${id}`;
  }

  function metricRefId(value) {
    if (!value || typeof value !== "object" || Array.isArray(value)) return "";
    if (value.__ref === "metric") return nonEmptyString(value.id);
    // page_instance local_nav 内嵌的未展开 metric_ref（__args.arg0 = 本地 metric id）
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

  /** 父级 popup / 行级下钻传入的 metric，优先于 board example 默认 metric。 */
  function resolvePopupPassedMetricId(detail, config = null) {
    const popupParams =
      detail?.popup && typeof detail.popup === "object" && !Array.isArray(detail.popup)
        ? detail.popup.params
        : null;
    const configParams =
      config?.params && typeof config.params === "object" && !Array.isArray(config.params)
        ? config.params
        : null;
    const configPopupParams =
      config?.popup && typeof config.popup === "object" && !Array.isArray(config.popup)
        ? config.popup.params
        : null;
    // link_decl / popup.params.metric 是作者显式指定的分析指标（如 issue_handling_analytics），
    // 必须优先于卡片自身的 count metric（warnings_pending_count 等），否则二级屏会误拉标量 rowset。
    return nonEmptyString(
      metricRefId(popupParams?.metric),
      metricRefId(configPopupParams?.metric),
      metricRefId(configParams?.metric),
      detail?.metric_id,
      detail?.__mei_runtime_ref?.metric_id,
    );
  }

  /** 下钻表/行级详情卡应使用的 metric：父级传入优先于 board slot / 示例默认。 */
  function resolveDrilldownTableMetricId(detail, config = null) {
    const metricId = String(detail?.metric_id || "").trim();
    return nonEmptyString(
      metricId,
      resolvePopupPassedMetricId(detail, config),
      config?.tableMetricId,
      config?.detailSlot?.metricId,
      config?.runtimeRef?.metricId,
      config?.runtimeRef?.metric_id,
    );
  }

  function resolveMetricOwnerScenePath(projectionSlots, detail) {
    if (Array.isArray(projectionSlots)) {
      for (const slot of projectionSlots) {
        const fromDataset = importedCapsuleScenePathFromWorldMetricsDatasetId(
          slot?.datasetId ?? slot?.dataset_id,
        );
        if (fromDataset) {
          return fromDataset;
        }
        const fromMetric = importedCapsuleScenePathFromMetricId(slot?.metricId ?? slot?.metric_id);
        if (fromMetric) {
          return fromMetric;
        }
      }
    }
    return nonEmptyString(
      importedCapsuleScenePathFromMetricId(detail?.metric_id),
      importedCapsuleScenePathFromWorldMetricsDatasetId(detail?.dataset_id),
      detail?.host_scene_file,
    );
  }

  function cloneArray(value) {
    return Array.isArray(value) ? value.slice() : [];
  }

  function positiveInt(...values) {
    for (const value of values) {
      const parsed = Number(value);
      if (Number.isFinite(parsed) && parsed > 0) {
        return Math.floor(parsed);
      }
    }
    return 0;
  }

  function boolValue(...values) {
    for (const value of values) {
      if (typeof value === "boolean") return value;
    }
    return undefined;
  }

  function isStaticDataMode(doc = document) {
    const body = doc?.body;
    if (!(body instanceof HTMLElement)) return false;
    const dataMode = String(body.getAttribute("data-data-mode") || "").trim().toLowerCase();
    if (dataMode === "static") return true;
    return String(body.getAttribute("data-surface") || "").trim().toLowerCase() === "prototype";
  }

  function buildStaticTableRows(columns, rowCount = 5) {
    const normalizedColumns = (Array.isArray(columns) ? columns : [])
      .map((column, index) => {
        const name = String(column?.name || column || "").trim();
        return name || `列${index + 1}`;
      })
      .filter(Boolean);
    const cols = normalizedColumns.length > 0 ? normalizedColumns : ["列1", "列2", "列3"];
    const count = Math.max(3, Math.min(8, Number(rowCount) || 5));
    return Array.from({ length: count }, (_entry, rowIndex) => {
      const row = {};
      cols.forEach((column, columnIndex) => {
        row[column] = columnIndex === 0 ? `值${rowIndex + 1}` : `列${columnIndex + 1}-值${rowIndex + 1}`;
      });
      return row;
    });
  }

