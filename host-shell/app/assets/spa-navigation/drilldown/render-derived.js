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
        root,
      });
      return false;
    }
    // 构成/趋势派生必须用看板分析指标（popup / tableMetricId），勿用入口 KPI count，
    // 否则有 default_filters 时会走 useDetailRowset → count::__scalar_rowset__ → 500。
    const cardMetricId = nonEmptyString(
      config?.tableMetricId,
      resolvePopupPassedMetricId(detail, config),
      detail?.metric_id,
      detail?.__mei_runtime_ref?.metric_id,
    );
    const fetchConfig = { ...config, datasetId };
    const sharedQueryStateId = nonEmptyString(
      config?.queryStateId,
      detail?.query_state_id,
      detail?.queryStateId,
    );
    const isCompositionTab =
      explainMetricKind(config, tabId) === "composition" ||
      nonEmptyString(config?.supportRole).toLowerCase() === "composition" ||
      isVerifiedShareCompositionTab(tabId, config);
    const isTrendTab =
      explainMetricKind(config, tabId) === "trend" ||
      nonEmptyString(config?.supportRole).toLowerCase() === "trend";
    const verifiedShare = isVerifiedShareCompositionTab(tabId, config);
    const useDetailRowset = Boolean(
      cardMetricId &&
        (isCompositionTab || isTrendTab) &&
        (verifiedShare ||
          (sharedQueryStateId && hasActiveDrilldownQueryFilters(sharedQueryStateId))),
    );
    if (useDetailRowset) {
      // 查实占比与有筛选的构成图：一律拉当前明细 scalar rowset 再聚合（单一真源）。
      fetchConfig.tableMetricId = resolveCardMetricRowsetId(cardMetricId);
      fetchConfig.supportRole = "";
      fetchConfig.clientAggregate = true;
    } else if (cardMetricId && isCompositionTab) {
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
    if (explainMetricKind(config, tabId) === "composition" || isVerifiedShareCompositionTab(tabId, config)) {
      const columns = Array.isArray(dataset?.columns) ? dataset.columns : [];
      const verifiedShare = isVerifiedShareCompositionTab(tabId, config);
      const dimension = verifiedShare ? "查实占比" : compositionFieldForTab(config, tabId);
      if (!dimension && !verifiedShare) {
        recordPopupDebugIssue({
          level: "error",
          message: `构成 tab 未解析到分组字段（tab=${normalizeTabId(tabId)}）`,
          phase: "derived_composition_dimension_missing",
          detail,
          config,
          datasetId,
          root,
        });
        return false;
      }
      const grouped = limitCompositionRows(
        verifiedShare
          ? groupRowsForVerifiedShare(rows, columns)
          : groupRowsForComposition(rows, dimension, columns, config, detail),
        config,
      );
      if (!grouped.length) return false;
      const chartTag = drilldownChartTag(config?.chartKind, tabId) || "mei-chart-bar";
      const registered = await ensureDrilldownChartRegistered(chartTag);
      if (!registered) return false;
      resetDrilldownChartSlotHost(
        host,
        resolveDrilldownChartSlotCaption(config) || (verifiedShare ? "查实占比" : `${dimension}构成`),
      );
      const node = document.createElement(chartTag);
      node.dataset.props = JSON.stringify(
        buildStaticChartModel(
          // Empty: slot caption already shows the title.
          "",
          tabId,
          grouped,
          {
            x: [{ field: "label", name: verifiedShare ? "查实占比" : dimension || "label" }],
            y: [{ field: "value", name: resolveCompositionYDisplayName(config, detail, "value") }],
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
          root,
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
          // Empty: slot caption already shows the title.
          "",
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

