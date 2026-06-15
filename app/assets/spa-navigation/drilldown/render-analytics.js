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
      const mergedConfig = {
        ...slotConfig,
        hasChartZone: config.hasChartZone,
        rowsetDatasetId: config.rowsetDatasetId,
        hostSceneId: config.hostSceneId,
        hostSceneFile: config.hostSceneFile,
        queryStateId: config.queryStateId,
        tableMetricId: nonEmptyString(slot.metricId, slotConfig.tableMetricId, boardMetricId),
        chartKind: nonEmptyString(slot.chartKind, slotConfig.chartKind),
        topN: positiveInt(slot.topN, slot.top_n, slotConfig.topN, slotConfig.top_n),
        mapping: chartMapping,
      };
      if (await mountAnalyticsChartSlot(root, detail, mergedConfig, slot.id, slotHost)) {
        return true;
      }
      const fallbackRuntimeRef = {
        ...(mergedConfig.runtimeRef && typeof mergedConfig.runtimeRef === "object"
          ? mergedConfig.runtimeRef
          : {}),
        kind: "metric",
        metricId: nonEmptyString(slot.metricId, mergedConfig?.runtimeRef?.metricId, mergedConfig?.runtimeRef?.metric_id),
        datasetId: nonEmptyString(slot.datasetId, mergedConfig?.runtimeRef?.datasetId, mergedConfig?.runtimeRef?.dataset_id),
        sceneId: nonEmptyString(mergedConfig?.runtimeRef?.sceneId, config.hostSceneId, config.sceneId),
        scenePath: nonEmptyString(
          mergedConfig?.runtimeRef?.scenePath,
          config.hostSceneFile,
          detail?.host_scene_file,
          detail?.scene_path
        ),
      };
      const fallbackConfig = {
        ...mergedConfig,
        supportRole: nonEmptyString(slot.supportRole, mergedConfig.supportRole, "composition"),
        tableMetricId: nonEmptyString(slot.metricId, mergedConfig.tableMetricId),
        datasetId: nonEmptyString(slot.datasetId, mergedConfig.datasetId),
        runtimeRef: fallbackRuntimeRef,
        compositionBy:
          Array.isArray(slot.by) && slot.by.length > 0
            ? slot.by
            : Array.isArray(mergedConfig.compositionBy)
              ? mergedConfig.compositionBy
              : [],
      };
      const fallbackMounted = await mountDerivedDrilldownContent(
        root,
        detail,
        fallbackConfig,
        slot.id,
        slotHost
      );
      if (!fallbackMounted) {
        recordPopupDebugIssue({
          level: "warn",
          phase: "analytics_chart_mount_fallback_failed",
          message: `chart slot ${String(slot.id || "").trim() || "unknown"} mount returned false`,
          detail,
          config: fallbackConfig,
          datasetId: fallbackConfig.datasetId,
          metricId: fallbackConfig.tableMetricId,
        });
      }
      return fallbackMounted;
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

