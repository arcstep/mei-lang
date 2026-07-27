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
    // 构成/趋势派生：父级看板 metric 优先；筛选重聚合时绝不能用 composition_by_* 子 metric。
    const boardParentMetricId = resolveBoardParentMetricId(detail, config);
    const cardMetricId = nonEmptyString(
      boardParentMetricId,
      config?.tableMetricId,
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
      fetchConfig.tableMetricId = resolveCardMetricRowsetId(boardParentMetricId || cardMetricId);
      fetchConfig.boardParentMetricId = boardParentMetricId || cardMetricId;
      fetchConfig.supportRole = "";
      fetchConfig.clientAggregate = true;
    } else if (cardMetricId && isCompositionTab) {
      const slotMetricId = nonEmptyString(config?.tableMetricId);
      const compositionMetricId = resolveCompositionScopedMetricId(
        boardParentMetricId || cardMetricId,
        tabId,
      );
      if (isDedicatedExplainMetricId(slotMetricId, { supportRole: config?.supportRole })) {
        fetchConfig.tableMetricId = slotMetricId;
      } else if (compositionMetricId) {
        fetchConfig.tableMetricId = compositionMetricId;
        fetchConfig.supportRole = "composition";
      }
    } else if (cardMetricId && isTrendTab) {
      fetchConfig.tableMetricId = resolveCardMetricRowsetId(boardParentMetricId || cardMetricId);
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
      const compositionCaption =
        resolveDrilldownChartSlotCaption(config) ||
        (verifiedShare ? "查实占比" : `${dimension}构成`);
      if (!grouped.length) {
        renderDrilldownChartEmptyState(host, compositionCaption);
        dispatchPreviewUpdated("drilldown");
        return true;
      }
      const chartTag = drilldownChartTag(config?.chartKind, tabId) || "mei-chart-bar";
      const registered = await ensureDrilldownChartRegistered(chartTag);
      if (!registered) return false;
      resetDrilldownChartSlotHost(host, compositionCaption);
      const dimTitle = verifiedShare ? "查实占比" : dimension || "label";
      const node = document.createElement(chartTag);
      node.dataset.props = JSON.stringify(
        buildStaticChartModel(
          // Empty: slot caption already shows the title.
          "",
          tabId,
          grouped,
          {
            // field 必须是聚合结果列 label；name 保留真实维度名供预警色板识别
            x: [{ field: "label", name: dimTitle }],
            label: [{ field: "label", name: dimTitle }],
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
      // 024008：与服务端 trend_year_compare 同形（month/year/value + group=year）
      const grouped = groupRowsYearMonthCompare(rows, trendField, columns, {
        window: "calendar",
        maxYears: 5,
      });
      const trendCaption = resolveDrilldownChartSlotCaption(config) || "趋势";
      if (!grouped.length) {
        renderDrilldownChartEmptyState(host, trendCaption);
        dispatchPreviewUpdated("drilldown");
        return true;
      }
      const registered = await ensureDrilldownChartRegistered("mei-chart-line");
      if (!registered) return false;
      resetDrilldownChartSlotHost(host, trendCaption);
      const node = document.createElement("mei-chart-line");
      const trendMapping =
        config?.mapping && typeof config.mapping === "object"
          ? config.mapping
          : {
              x: [{ field: "month", name: "月份" }],
              y: [{ field: "value", name: "value" }],
              group: [{ field: "year", name: "年度" }],
            };
      node.dataset.props = JSON.stringify(
        buildStaticChartModel(
          // Empty: slot caption already shows the title.
          "",
          tabId,
          grouped,
          trendMapping,
          config,
        ),
      );
      host.appendChild(node);
      dispatchPreviewUpdated("drilldown");
      return true;
    }
    return false;
  }

