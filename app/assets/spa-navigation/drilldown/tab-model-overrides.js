  function normalizeTabMetricOverrides(...values) {
    const raw = values.find(
      (value) =>
        value &&
        typeof value === "object" &&
        !Array.isArray(value) &&
        Object.keys(value).length > 0,
    );
    if (!raw) return {};
    const normalized = {};
    Object.entries(raw).forEach(([key, entry]) => {
      const tabId = normalizeTabId(key);
      if (!tabId) return;
      if (typeof entry === "string") {
        const metricId = String(entry || "").trim();
        if (!metricId) return;
        normalized[tabId] = { tableMetricId: metricId };
        return;
      }
      if (!entry || typeof entry !== "object" || Array.isArray(entry)) {
        return;
      }
      if (entry.__ref === "explain_metric") {
        const explainMetricId = nonEmptyString(entry.id);
        if (!explainMetricId) return;
        normalized[tabId] = {
          explainMetricId,
          compositionBy: cloneArray(entry.composition_by).length
            ? cloneArray(entry.composition_by)
            : entry.by
              ? [String(entry.by)]
              : [],
          topN: positiveInt(entry.top_n, entry.topN),
          trendField: nonEmptyString(entry.trend_field, entry.date_field, entry.dateField),
          trendGrain: nonEmptyString(entry.grain, entry.trend_grain, entry.trendGrain),
        };
        return;
      }
      if (entry.__ref === "metric") {
        const metricId = nonEmptyString(entry.id);
        if (!metricId) return;
        normalized[tabId] = {
          runtimeRef: {
            kind: "metric",
            metricId,
            datasetId: nonEmptyString(entry.from_dataset, entry.fromDataset),
            sceneId: nonEmptyString(entry.scene_id, entry.sceneId),
            scenePath: nonEmptyString(entry.scene_file, entry.sceneFile),
          },
        };
        return;
      }
      const runtimeRef =
        entry.__mei_runtime_ref && typeof entry.__mei_runtime_ref === "object"
          ? entry.__mei_runtime_ref
          : null;
      if (runtimeRef?.kind === "metric") {
        const metricId = nonEmptyString(runtimeRef.metric_id, runtimeRef.metricId);
        if (!metricId) return;
        normalized[tabId] = {
          runtimeRef: {
            kind: "metric",
            metricId,
            datasetId: nonEmptyString(runtimeRef.dataset_id, runtimeRef.datasetId),
            sceneId: nonEmptyString(runtimeRef.scene_id, runtimeRef.sceneId),
            scenePath: nonEmptyString(runtimeRef.scene_path, runtimeRef.scenePath),
          },
        };
        return;
      }
      if (entry.__ref === "dataset") {
        const datasetId = nonEmptyString(entry.id);
        if (!datasetId) return;
        normalized[tabId] = {
          runtimeRef: {
            kind: "data",
            datasetId,
            sceneId: nonEmptyString(entry.scene_id, entry.sceneId),
            scenePath: nonEmptyString(entry.scene_file, entry.sceneFile, entry.path),
          },
        };
        return;
      }
      if (entry.source && typeof entry.source === "object" && !Array.isArray(entry.source)) {
        const source = entry.source;
        const sourceKind = nonEmptyString(source.kind);
        if (sourceKind === "metric_ref") {
          normalized[tabId] = {
            runtimeRef: {
              kind: "metric",
              metricId: nonEmptyString(source.metric_id, source.metricId, entry.table_metric_id, entry.tableMetricId),
              datasetId: nonEmptyString(source.dataset_id, source.datasetId, entry.dataset_id, entry.datasetId),
              sceneId: nonEmptyString(source.scene_id, source.sceneId),
              scenePath: nonEmptyString(source.scene_file, source.sceneFile),
            },
            tableMetricId: nonEmptyString(source.metric_id, source.metricId, entry.table_metric_id, entry.tableMetricId),
            datasetId: nonEmptyString(source.dataset_id, source.datasetId, entry.dataset_id, entry.datasetId),
            topN: positiveInt(entry.top_n, entry.topN),
          };
          return;
        }
        if (sourceKind === "dataset_ref") {
          normalized[tabId] = {
            runtimeRef: {
              kind: "data",
              datasetId: nonEmptyString(source.dataset_id, source.datasetId, entry.dataset_id, entry.datasetId),
              sceneId: nonEmptyString(source.scene_id, source.sceneId),
              scenePath: nonEmptyString(source.scene_file, source.sceneFile),
            },
            datasetId: nonEmptyString(source.dataset_id, source.datasetId, entry.dataset_id, entry.datasetId),
          };
          return;
        }
      }
      if (entry.runtime_ref && typeof entry.runtime_ref === "object" && !Array.isArray(entry.runtime_ref)) {
        const runtimeRef = entry.runtime_ref;
        normalized[tabId] = {
          runtimeRef: {
            kind: nonEmptyString(runtimeRef.kind),
            metricId: nonEmptyString(runtimeRef.metric_id, runtimeRef.metricId, entry.metric_id, entry.metricId),
            datasetId: nonEmptyString(runtimeRef.dataset_id, runtimeRef.datasetId, entry.dataset_id, entry.datasetId),
            sceneId: nonEmptyString(runtimeRef.scene_id, runtimeRef.sceneId),
            scenePath: nonEmptyString(runtimeRef.scene_path, runtimeRef.scenePath),
          },
          tableMetricId: nonEmptyString(runtimeRef.metric_id, runtimeRef.metricId, entry.metric_id, entry.metricId),
          datasetId: nonEmptyString(runtimeRef.dataset_id, runtimeRef.datasetId, entry.dataset_id, entry.datasetId),
          columns: cloneArray(entry.fields),
          headers: cloneArray(entry.headers),
          mapping: entry.mapping && typeof entry.mapping === "object" ? entry.mapping : null,
          chartKind: nonEmptyString(entry.chart_kind, entry.chartKind),
          topN: positiveInt(entry.top_n, entry.topN),
          compositionBy: cloneArray(entry.composition_by).length
            ? cloneArray(entry.composition_by)
            : cloneArray(entry.compositionBy),
          trendField: nonEmptyString(entry.date_field, entry.dateField),
          trendGrain: nonEmptyString(entry.grain),
        };
        return;
      }
      let columns = cloneArray(entry.columns);
      if (!columns.length) columns = cloneArray(entry.detail_fields);
      if (!columns.length) columns = cloneArray(entry.detailFields);
      const override = {
        title: nonEmptyString(entry.title),
        note: nonEmptyString(entry.note),
        tableMetricId: nonEmptyString(
          entry.table_metric_id,
          entry.tableMetricId,
          entry.metric_id,
          entry.metricId,
        ),
        datasetId: nonEmptyString(entry.dataset_id, entry.datasetId),
        columns,
        headers: cloneArray(entry.headers),
        layoutPreset: nonEmptyString(entry.layout_preset, entry.layoutPreset),
        chartKind: nonEmptyString(entry.chart_kind, entry.chartKind, entry.chart),
        topN: positiveInt(entry.top_n, entry.topN),
        mapping: entry.mapping && typeof entry.mapping === "object" ? entry.mapping : null,
        compositionBy: cloneArray(entry.composition_by).length
          ? cloneArray(entry.composition_by)
          : cloneArray(entry.compositionBy),
        trendField: nonEmptyString(entry.trend_field, entry.trendField),
        trendGrain: nonEmptyString(entry.trend_grain, entry.trendGrain),
      };
      if (
        !override.title &&
        !override.note &&
        !override.tableMetricId &&
        !override.datasetId &&
        !override.columns.length &&
        !override.headers.length &&
        !override.layoutPreset &&
        !override.chartKind &&
        !override.topN &&
        !override.mapping &&
        !override.compositionBy.length &&
        !override.trendField
      ) {
        return;
      }
      normalized[tabId] = override;
    });
    return normalized;
  }

