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

