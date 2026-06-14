  function drilldownChartTag(chartKind, tabId) {
    const explicit = String(chartKind || "").trim().toLowerCase();
    const fallback = normalizeTabId(tabId) === "trend" ? "line" : "bar";
    const kind = explicit || fallback;
    const supported = new Set([
      "line",
      "area",
      "trend",
      "column",
      "bar",
      "scatter",
      "pie",
      "donut",
      "rose",
      "radar",
      "ranking",
      "boxplot",
    ]);
    if (!supported.has(kind)) return "";
    return `mei-chart-${kind}`;
  }

  const DRILLDOWN_CHART_SCRIPT_BY_TAG = {
    "mei-chart-line": "/workspace-components/chart/echarts/line.js",
    "mei-chart-area": "/workspace-components/chart/echarts/area.js",
    "mei-chart-trend": "/workspace-components/chart/echarts/trend.js",
    "mei-chart-column": "/workspace-components/chart/echarts/column.js",
    "mei-chart-bar": "/workspace-components/chart/echarts/bar.js",
    "mei-chart-scatter": "/workspace-components/chart/echarts/scatter.js",
    "mei-chart-pie": "/workspace-components/chart/echarts/pie.js",
    "mei-chart-donut": "/workspace-components/chart/echarts/donut.js",
    "mei-chart-rose": "/workspace-components/chart/echarts/rose.js",
    "mei-chart-radar": "/workspace-components/chart/echarts/radar.js",
    "mei-chart-ranking": "/workspace-components/chart/echarts/ranking.js",
    "mei-chart-boxplot": "/workspace-components/chart/echarts/boxplot.js",
  };

  const DRILLDOWN_TABLE_SCRIPT = "/workspace-components/cockpit/data-table.js";
  const DRILLDOWN_FILTER_BAR_SCRIPT = "/workspace-components/dataset/filter-bar.js";
  const DRILLDOWN_CUSTOM_ELEMENT_WAIT_MS = 8000;

  async function waitForCustomElementTag(tagName) {
    const tag = String(tagName || "").trim().toLowerCase();
    if (!tag) return false;
    if (customElements.get(tag)) return true;
    try {
      await Promise.race([
        customElements.whenDefined(tag),
        new Promise((_, reject) => {
          setTimeout(
            () => reject(new Error("custom element define timeout: " + tag)),
            DRILLDOWN_CUSTOM_ELEMENT_WAIT_MS,
          );
        }),
      ]);
    } catch (_) {
      /* fall through */
    }
    return Boolean(customElements.get(tag));
  }

  async function ensureDrilldownChartRegistered(tagName) {
    const tag = String(tagName || "").trim().toLowerCase();
    if (!tag) return false;
    if (customElements.get(tag)) return true;
    const scriptPath = DRILLDOWN_CHART_SCRIPT_BY_TAG[tag];
    if (!scriptPath) return false;
    await loadScript(scriptPath, {
      module: true,
      persistentKey: scriptPath,
      softFail: false,
    });
    return waitForCustomElementTag(tag);
  }

  async function ensureDrilldownTableRegistered() {
    const tag = "mei-cockpit-data-table";
    if (customElements.get(tag)) return true;
    await loadScript(DRILLDOWN_TABLE_SCRIPT, {
      module: true,
      persistentKey: DRILLDOWN_TABLE_SCRIPT,
      softFail: false,
    });
    return waitForCustomElementTag(tag);
  }

  async function ensureDrilldownFilterBarRegistered() {
    const tag = "mei-dataset-filter-bar";
    if (customElements.get(tag)) return true;
    await loadScript(DRILLDOWN_FILTER_BAR_SCRIPT, {
      module: true,
      persistentKey: DRILLDOWN_FILTER_BAR_SCRIPT,
      softFail: false,
    });
    return waitForCustomElementTag(tag);
  }

  async function prefetchStructuredDrilldownWidgets(config) {
    const tasks = [ensureDrilldownTableRegistered(), ensureDrilldownFilterBarRegistered()];
    const chartTags = new Set();
    const slotsByZone =
      config?.slotsByZone && typeof config.slotsByZone === "object" && !Array.isArray(config.slotsByZone)
        ? config.slotsByZone
        : {};
    Object.values(slotsByZone).forEach((zoneSlots) => {
      if (!Array.isArray(zoneSlots)) return;
      zoneSlots.forEach((slot) => {
        if (slot?.component !== "chart") return;
        const tag = drilldownChartTag(slot.chartKind, slot.id);
        if (tag) chartTags.add(tag);
      });
    });
    if (!chartTags.size && Array.isArray(config?.chartSlots)) {
      config.chartSlots.forEach((slot) => {
        const tag = drilldownChartTag(slot?.chartKind, slot?.id);
        if (tag) chartTags.add(tag);
      });
    }
    chartTags.forEach((tag) => tasks.push(ensureDrilldownChartRegistered(tag)));
    await Promise.all(tasks);
  }

  function buildDrilldownChartProps(detail, config, tabId) {
    const tableProps = buildDrilldownTableProps(detail, config);
    if (!tableProps) return null;
    const chartTag = drilldownChartTag(config?.chartKind, tabId);
    if (!chartTag) return null;
    const columns = Array.isArray(config?.columns) ? config.columns : [];
    const normalizedKind = explainMetricKind(config, tabId);
    const compositionField = nonEmptyString(
      Array.isArray(config?.compositionBy) ? config.compositionBy[0] : "",
      columns[0],
      "label",
    );
    const xField =
      normalizedKind === "trend" ? "month" : normalizedKind === "composition" ? compositionField : columns[0] || "label";
    const yField = "value";
    const mapping =
      config?.mapping && typeof config.mapping === "object"
        ? config.mapping
        : {
            x: xField,
            y: yField,
          };
    return {
      chartTag,
      props: {
        title: String(config?.title || ""),
        data: tableProps.dataset,
        _mei: tableProps._mei,
        query_state: tableProps.query_state,
        mapping,
        ...buildAnalyticsChartPresentationProps(config),
      },
    };
  }

  async function mountAnalyticsChartSlot(root, detail, config, tabId, hostOverride = null) {
    const kind = explainMetricKind(config, tabId);
    const supportRole = nonEmptyString(config?.supportRole, config?.slotByTab?.[normalizeTabId(tabId)]?.supportRole);
    // 构成图需要明细行在前端按维度聚合；metric KPI 查询只返回标量，不能直接驱动图表。
    if (kind === "composition" || supportRole === "composition") {
      if (await mountDerivedDrilldownContent(root, detail, config, tabId, hostOverride)) {
        return true;
      }
    }
    return mountDrilldownChart(root, detail, config, tabId, hostOverride);
  }

  async function mountDrilldownChart(root, detail, config, tabId, hostOverride = null) {
    const host =
      hostOverride instanceof HTMLElement
        ? hostOverride
        : root.querySelector('[data-drilldown-table-host="true"]');
    if (!(host instanceof HTMLElement)) {
      return false;
    }
    const chart = buildDrilldownChartProps(detail, config, tabId);
    if (!chart) return false;
    const registered = await ensureDrilldownChartRegistered(chart.chartTag);
    if (!registered) return false;
    host.replaceChildren();
    const node = document.createElement(chart.chartTag);
    node.dataset.props = JSON.stringify(chart.props);
    host.appendChild(node);
    return true;
  }

  async function mountDrilldownTable(root, detail, config, hostOverride = null) {
    const host =
      hostOverride instanceof HTMLElement
        ? hostOverride
        : root.querySelector('[data-drilldown-table-host="true"]');
    if (!(host instanceof HTMLElement)) {
      return false;
    }
    const props = buildDrilldownTableProps(detail, config);
    if (!props) {
      recordPopupDebugIssue({
        level: "error",
        message: "未解析到下钻明细表所需 scene_id 或 dataset_id",
        phase: "table_mount_setup",
        detail,
        config,
        datasetId: resolveDrilldownDatasetId(detail, config),
        metricId: nonEmptyString(detail?.metric_id, detail?.__mei_runtime_ref?.metric_id),
      });
      return false;
    }
    const registered = await ensureDrilldownTableRegistered();
    if (!registered) {
      recordPopupDebugIssue({
        level: "error",
        message: "未注册 mei-cockpit-data-table（可能是组件脚本加载失败）",
        phase: "table_mount_register",
        detail,
        config,
        datasetId: resolveDrilldownDatasetId(detail, config),
        metricId: nonEmptyString(detail?.metric_id, detail?.__mei_runtime_ref?.metric_id),
      });
      return false;
    }
    host.replaceChildren();
    const table = document.createElement("mei-cockpit-data-table");
    table.dataset.props = JSON.stringify(props);
    host.appendChild(table);
    return true;
  }

  function buildAnalyticsFilterBarProps(config, detail) {
    const fields = Array.isArray(config?.filterSchema?.fields) ? config.filterSchema.fields : [];
    const tableProps = buildDrilldownTableProps(detail, config) || {};
    const rowsetDatasetId = nonEmptyString(
      config?.filterSchema?.rowsetDatasetId,
      tableProps?.dataset?.__mei_runtime_ref?.dataset_id,
      tableProps?.dataset?.id,
    );
    const listPreview = Boolean(config?.hasRowPreviewZone);
    return {
      title: "筛选条件",
      description: listPreview
        ? "调整条件后清单与预览将同步刷新。"
        : "调整条件后图表与明细表将同步刷新。",
      live: true,
      query_state: config?.queryStateId || undefined,
      rowset_dataset_id: rowsetDatasetId || undefined,
      dataset: rowsetDatasetId
        ? {
            id: rowsetDatasetId,
            shape: "table",
            __mei_runtime_ref: {
              dataset_id: rowsetDatasetId,
              scene_id: nonEmptyString(config?.hostSceneId, config?.sceneId),
            },
          }
        : tableProps.dataset,
      data: rowsetDatasetId ? { id: rowsetDatasetId } : tableProps.dataset,
      _mei: tableProps._mei,
      fields: fields.map((field) => {
        const column = nonEmptyString(field.column, field.key);
        const control = nonEmptyString(field.control, "text");
        const needsRowsetOptions = control === "multi_select" || control === "month_multi_select";
        return {
          key: column,
          label: field.label || field.key || column,
          column,
          control,
          options_from: needsRowsetOptions ? "rowset" : "",
          options_field: column,
        };
      }),
    };
  }

  async function mountAnalyticsFilterBar(root, detail, config, hostOverride = null) {
    const host =
      hostOverride instanceof HTMLElement
        ? hostOverride
        : root.querySelector('[data-drilldown-filter-host="true"]');
    if (!(host instanceof HTMLElement)) return false;
    const filterProps = buildAnalyticsFilterBarProps(config, detail);
    const fieldCount = Array.isArray(filterProps?.fields) ? filterProps.fields.length : 0;
    host.toggleAttribute("hidden", fieldCount === 0);
    if (fieldCount === 0) {
      host.replaceChildren();
      return false;
    }
    const registered = await ensureDrilldownFilterBarRegistered();
    if (!registered) return false;
    host.replaceChildren();
    const node = document.createElement("mei-dataset-filter-bar");
    node.dataset.props = JSON.stringify(filterProps);
    host.appendChild(node);
    return true;
  }

  function cleanupAnalyticsDrilldownWatcher(root) {
    if (!(root instanceof HTMLElement)) return;
    const cleanup = root.__meiAnalyticsQueryStateCleanup;
    if (typeof cleanup === "function") {
      cleanup();
    }
    root.__meiAnalyticsQueryStateCleanup = null;
  }

  async function remountStructuredAnalyticsChartZones(root, detail, config, resolveZoneHost) {
    const slotZones = sceneShellZonesByRole(config?.sceneShell, "slots");
    let ok = true;
    for (const zone of slotZones) {
      const zoneSlots = Array.isArray(config?.slotsByZone?.[zone.id]) ? config.slotsByZone[zone.id] : [];
      if (!zoneSlots.length || !zoneSlots.every((slot) => slot.component === "chart")) {
        continue;
      }
      const host =
        typeof resolveZoneHost === "function"
          ? resolveZoneHost(zone.id)
          : root.__meiStructuredZoneHosts?.[zone.id];
      if (!(host instanceof HTMLElement)) {
        ok = false;
        continue;
      }
      const zoneOk = await mountAnalyticsChartSlots(root, detail, config, zoneSlots, host);
      ok = ok && zoneOk;
    }
    return ok;
  }

  function bindAnalyticsChartsQueryStateRefresh(root, detail, config, resolveZoneHost) {
    cleanupAnalyticsDrilldownWatcher(root);
    const queryStateId = nonEmptyString(config?.queryStateId, detail?.query_state_id, detail?.queryStateId);
    if (!queryStateId) return;
    let refreshSeq = 0;
    const onQueryStateChange = (event) => {
      if (event?.detail?.id !== queryStateId) return;
      if (!(root instanceof HTMLElement) || root.hasAttribute("hidden")) return;
      const currentSeq = ++refreshSeq;
      remountStructuredAnalyticsChartZones(root, detail, config, resolveZoneHost)
        .then((ok) => {
          if (!ok || currentSeq !== refreshSeq) return;
          dispatchPreviewUpdated("drilldown");
        })
        .catch((error) => {
          recordPopupDebugIssue({
            level: "error",
            message: String(error?.message || error || "分析型看板图表刷新失败"),
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

