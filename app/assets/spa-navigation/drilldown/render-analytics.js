  async function mountAnalyticsChartSlots(root, detail, config, chartSlots, chartsHost) {
    const chartMounts = chartSlots.map(async (slot, index) => {
      const slotHost = chartsHost.querySelector(`[data-chart-slot-index="${index}"]`);
      const slotConfig = resolveDrilldownTabConfig(config, slot.id);
      const boardMetricId = nonEmptyString(config.tableMetricId);
      const chartMapping =
        slot.mapping && typeof slot.mapping === "object"
          ? slot.mapping
          : slotConfig.mapping && typeof slotConfig.mapping === "object"
            ? slotConfig.mapping
            : null;
      const cardMetricId = nonEmptyString(detail?.metric_id, detail?.__mei_runtime_ref?.metric_id, boardMetricId);
      const compositionMetricId = resolveCompositionScopedMetricId(cardMetricId, slot.id);
      const resolvedChartMetricId = nonEmptyString(
        isDedicatedExplainMetricId(slot.metricId, { supportRole: slot.supportRole })
          ? slot.metricId
          : "",
        compositionMetricId,
        slot.metricId,
        slotConfig.tableMetricId,
        boardMetricId,
      );
      const mergedConfig = {
        ...slotConfig,
        hasChartZone: config.hasChartZone,
        rowsetDatasetId: config.rowsetDatasetId,
        hostSceneId: config.hostSceneId,
        hostSceneFile: config.hostSceneFile,
        queryStateId: config.queryStateId,
        supportRole: nonEmptyString(slot.supportRole, slotConfig.supportRole, "composition"),
        tableMetricId: resolvedChartMetricId,
        chartKind: nonEmptyString(slot.chartKind, slotConfig.chartKind),
        topN: positiveInt(slot.topN, slot.top_n, slotConfig.topN, slotConfig.top_n),
        mapping: chartMapping,
        compositionBy:
          Array.isArray(slot.by) && slot.by.length > 0
            ? slot.by
            : Array.isArray(slotConfig.compositionBy)
              ? slotConfig.compositionBy
              : [],
        runtimeRef: {
          ...(slotConfig.runtimeRef && typeof slotConfig.runtimeRef === "object" ? slotConfig.runtimeRef : {}),
          kind: "metric",
          metricId: resolvedChartMetricId,
          metric_id: resolvedChartMetricId,
          datasetId: nonEmptyString(slot.datasetId, slotConfig?.runtimeRef?.datasetId),
          dataset_id: nonEmptyString(slot.datasetId, slotConfig?.runtimeRef?.dataset_id),
          sceneId: nonEmptyString(config.hostSceneId, config.sceneId),
          scene_id: nonEmptyString(config.hostSceneId, config.sceneId),
          scenePath: nonEmptyString(config.hostSceneFile, detail?.host_scene_file, detail?.scene_path),
          scene_path: nonEmptyString(config.hostSceneFile, detail?.host_scene_file, detail?.scene_path),
        },
      };
      if (await mountAnalyticsChartSlot(root, detail, mergedConfig, slot.id, slotHost)) {
        return true;
      }
      recordPopupDebugIssue({
        level: "warn",
        phase: "analytics_chart_mount_failed",
        message: `chart slot ${String(slot.id || "").trim() || "unknown"} mount returned false`,
        detail,
        config: mergedConfig,
        datasetId: mergedConfig.datasetId,
        metricId: mergedConfig.tableMetricId,
      });
      return false;
    });
    const results = await Promise.all(chartMounts);
    return chartSlots.length === 0 || results.every(Boolean);
  }

  async function renderAnalyticsDrilldownContent(root, detail, config) {
    applyDrilldownOverlayMeta(root, config);
    setDrilldownOverlayStatus(root, "loading");
    cleanupAnalyticsDrilldownWatcher(root);
    const chartsHost = root.querySelector('[data-drilldown-charts-host="true"]');
    const tableHost = root.querySelector('[data-drilldown-analytics-table-host="true"]');
    if (!(chartsHost instanceof HTMLElement) || !(tableHost instanceof HTMLElement)) {
      setDrilldownOverlayStatus(root, "error");
      return false;
    }
    chartsHost.replaceChildren();
    tableHost.replaceChildren();
    const chartSlots = Array.isArray(config?.chartSlots) ? config.chartSlots : [];
    chartSlots.forEach((slot, index) => {
      const slotEl = document.createElement("div");
      slotEl.className = "access-drilldown-analytics-chart-slot";
      slotEl.dataset.chartSlotIndex = String(index);
      chartsHost.appendChild(slotEl);
    });
    chartsHost.style.gridTemplateColumns =
      chartSlots.length > 1 ? `repeat(${chartSlots.length}, minmax(0, 1fr))` : "1fr";
    chartsHost.toggleAttribute("hidden", chartSlots.length === 0);

    try {
      const filterHost = root.querySelector(
        '[data-drilldown-body-mode="analytics"] [data-drilldown-filter-host="true"]',
      );
      await mountAnalyticsFilterBar(root, detail, config, filterHost);
      const detailSlot = config?.detailSlot;
      const detailTabConfig = detailSlot ? resolveDrilldownTabConfig(config, detailSlot.id) : config;
      const detailConfig = detailSlot
        ? {
            ...detailTabConfig,
            queryStateId: config.queryStateId,
            pageSize: positiveInt(
              detailSlot.pageSize,
              detailSlot.page_size,
              detailTabConfig.pageSize,
              detailTabConfig.page_size,
              10,
            ),
            pagination: true,
            paginationMode: "server",
            columns: cloneArray(detailSlot.fields).length
              ? cloneArray(detailSlot.fields)
              : cloneArray(detailTabConfig.columns),
          }
        : config;
      const [chartsOk, tableOk] = await Promise.all([
        mountAnalyticsChartSlots(root, detail, config, chartSlots, chartsHost),
        mountDrilldownTable(root, detail, detailConfig, tableHost),
      ]);
      if (!tableOk || !chartsOk) {
        setDrilldownOverlayStatus(root, "error");
        return false;
      }
      const queryStateId = nonEmptyString(config?.queryStateId, detail?.query_state_id, detail?.queryStateId);
      if (queryStateId) {
        let refreshSeq = 0;
        const onQueryStateChange = (event) => {
          if (event?.detail?.id !== queryStateId) return;
          if (!(root instanceof HTMLElement) || root.hasAttribute("hidden")) return;
          const currentSeq = ++refreshSeq;
          mountAnalyticsChartSlots(root, detail, config, chartSlots, chartsHost)
            .then((ok) => {
              if (!ok || currentSeq !== refreshSeq) return;
              dispatchPreviewUpdated("drilldown");
            })
            .catch((error) => {
              recordPopupDebugIssue({
                level: "error",
                message: String(error?.message || error || "分析型下钻图表刷新失败"),
                phase: "analytics_chart_refresh_error",
                detail,
                config,
              });
            });
        };
        window.addEventListener("mei:query-state-change", onQueryStateChange);
        root.__meiAnalyticsQueryStateCleanup = () => {
          window.removeEventListener("mei:query-state-change", onQueryStateChange);
        };
      }
      setDrilldownOverlayStatus(root, "ready");
      dispatchPreviewUpdated("drilldown");
      return true;
    } catch (error) {
      recordPopupDebugIssue({
        level: "error",
        message: String(error?.message || error || "分析型下钻看板渲染失败"),
        phase: "analytics_render_error",
        detail,
        config,
      });
      setDrilldownOverlayStatus(root, "error");
      return false;
    }
  }

