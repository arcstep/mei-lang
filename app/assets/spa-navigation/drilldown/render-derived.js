  async function mountDerivedDrilldownContent(root, detail, config, tabId, hostOverride = null) {
    const host =
      hostOverride instanceof HTMLElement
        ? hostOverride
        : root.querySelector('[data-drilldown-table-host="true"]');
    if (!(host instanceof HTMLElement)) {
      return false;
    }
    const datasetId = resolveDrilldownDatasetId(detail, config);
    if (!datasetId) {
      recordPopupDebugIssue({
        level: "error",
        message: "未解析到 explain 派生块需要的数据集 id",
        phase: "derived_dataset_missing",
        detail,
        config,
      });
      return false;
    }
    const cardMetricId = nonEmptyString(detail?.metric_id, detail?.__mei_runtime_ref?.metric_id);
    const fetchConfig = { ...config, datasetId };
    const isCompositionTab =
      explainMetricKind(config, tabId) === "composition" ||
      nonEmptyString(config?.supportRole).toLowerCase() === "composition";
    const isTrendTab =
      explainMetricKind(config, tabId) === "trend" ||
      nonEmptyString(config?.supportRole).toLowerCase() === "trend";
    if (cardMetricId && isCompositionTab) {
      const slotMetricId = nonEmptyString(config?.tableMetricId);
      const compositionMetricId = resolveCompositionScopedMetricId(cardMetricId, tabId);
      if (isDedicatedExplainMetricId(slotMetricId, { supportRole: config?.supportRole })) {
        fetchConfig.tableMetricId = slotMetricId;
      } else if (compositionMetricId) {
        fetchConfig.tableMetricId = compositionMetricId;
        fetchConfig.supportRole = "composition";
      }
    } else if (cardMetricId && isTrendTab) {
      fetchConfig.tableMetricId = resolveCardMetricRowsetId(cardMetricId);
    }
    const dataset = await fetchPopupDrilldownRows(detail, fetchConfig);
    const rows = Array.isArray(dataset?.rows) ? dataset.rows : [];
    if (!rows.length) {
      recordPopupDebugIssue({
        level: "warn",
        message: "popup panel 派生查询返回 0 行，已回退到摘要说明",
        phase: "derived_dataset_empty",
        detail,
        config,
        datasetId,
      });
    }
    if (explainMetricKind(config, tabId) === "composition") {
      const columns = Array.isArray(dataset?.columns) ? dataset.columns : [];
      const dimension = compositionFieldForTab(config, tabId);
      if (!dimension) {
        recordPopupDebugIssue({
          level: "error",
          message: `构成 tab 未解析到分组字段（tab=${normalizeTabId(tabId)}）`,
          phase: "derived_composition_dimension_missing",
          detail,
          config,
          datasetId,
        });
        return false;
      }
      const grouped = limitCompositionRows(
        groupRowsForComposition(rows, dimension, columns, config, detail),
        config,
      );
      if (!grouped.length) return false;
      const chartTag = drilldownChartTag(config?.chartKind, tabId) || "mei-chart-bar";
      const registered = await ensureDrilldownChartRegistered(chartTag);
      if (!registered) return false;
      resetDrilldownChartSlotHost(
        host,
        resolveDrilldownChartSlotCaption(config) || `${dimension}构成`,
      );
      const node = document.createElement(chartTag);
      node.dataset.props = JSON.stringify(
        buildStaticChartModel(
          config?.title || `${dimension}构成`,
          tabId,
          grouped,
          {
            x: "label",
            y: "value",
          },
          config,
        ),
      );
      host.appendChild(node);
      dispatchPreviewUpdated("drilldown");
      return true;
    }
    if (explainMetricKind(config, tabId) === "trend") {
      const columns = Array.isArray(dataset?.columns) ? dataset.columns : [];
      const trendField = nonEmptyString(config?.trendField);
      if (!trendField) {
        recordPopupDebugIssue({
          level: "error",
          message: `趋势 tab 未解析到日期字段（tab=${normalizeTabId(tabId)}）`,
          phase: "derived_trend_field_missing",
          detail,
          config,
          datasetId,
        });
        return false;
      }
      const grouped = groupRowsByMonth(rows, trendField, columns);
      if (!grouped.length) return false;
      const registered = await ensureDrilldownChartRegistered("mei-chart-line");
      if (!registered) return false;
      resetDrilldownChartSlotHost(host, resolveDrilldownChartSlotCaption(config) || "趋势");
      const node = document.createElement("mei-chart-line");
      node.dataset.props = JSON.stringify(
        buildStaticChartModel(
          config?.title || "趋势",
          tabId,
          grouped,
          {
            x: "month",
            y: "value",
          },
          config,
        ),
      );
      host.appendChild(node);
      dispatchPreviewUpdated("drilldown");
      return true;
    }
    return false;
  }

