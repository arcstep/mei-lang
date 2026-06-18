  function nonEmptyString(...values) {
    for (const value of values) {
      const text = String(value || "").trim();
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

  function metricRefId(value) {
    if (!value || typeof value !== "object" || Array.isArray(value)) return "";
    if (value.__ref === "metric") return nonEmptyString(value.id);
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
    return nonEmptyString(
      detail?.metric_id,
      detail?.__mei_runtime_ref?.metric_id,
      metricRefId(popupParams?.metric),
      metricRefId(configParams?.metric),
      metricRefId(configPopupParams?.metric),
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

