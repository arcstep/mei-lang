  function resolveDrilldownChartSlotCaption(config) {
    const explicit = nonEmptyString(config?.title, config?.label);
    if (explicit) return explicit;
    const by = nonEmptyString(
      Array.isArray(config?.compositionBy) ? config.compositionBy[0] : "",
      config?.by,
    );
    return by ? `按${by}构成` : "";
  }

  function createDrilldownChartSlotCaption(title) {
    const text = nonEmptyString(title);
    if (!text) return null;
    const el = document.createElement("div");
    el.className = "access-drilldown-chart-slot-caption";
    el.textContent = text;
    return el;
  }

  function resetDrilldownChartSlotHost(host, title) {
    host.replaceChildren();
    const caption = createDrilldownChartSlotCaption(title);
    if (caption) host.appendChild(caption);
  }

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
    const cardMetricId = nonEmptyString(
      detail?.metric_id,
      detail?.__mei_runtime_ref?.metric_id,
      config?.tableMetricId,
    );
    const chartMetricId = nonEmptyString(
      config?.tableMetricId,
      resolveCompositionScopedMetricId(cardMetricId, tabId),
      config?.runtimeRef?.metricId,
      config?.runtimeRef?.metric_id,
    );
    const dedicatedChartMetric = isDedicatedExplainMetricId(chartMetricId, {
      supportRole: config?.supportRole,
    });
    const runtimeRefConfig =
      config?.runtimeRef && typeof config.runtimeRef === "object" ? config.runtimeRef : {};
    const chartDataset =
      dedicatedChartMetric && chartMetricId
        ? {
            __mei_runtime_ref: {
              kind: "metric",
              metric_id: chartMetricId,
              dataset_id: nonEmptyString(
                runtimeRefConfig.datasetId,
                runtimeRefConfig.dataset_id,
                tableProps.dataset?.__mei_runtime_ref?.dataset_id,
              ),
              scene_id: nonEmptyString(
                runtimeRefConfig.sceneId,
                runtimeRefConfig.scene_id,
                tableProps.dataset?.__mei_runtime_ref?.scene_id,
              ),
              scene_path: nonEmptyString(
                runtimeRefConfig.scenePath,
                runtimeRefConfig.scene_path,
                tableProps.dataset?.__mei_runtime_ref?.scene_path,
              ),
            },
          }
        : tableProps.dataset;
    const compositionField = nonEmptyString(
      compositionFieldForTab(config, tabId),
      Array.isArray(config?.compositionBy) ? config.compositionBy[0] : "",
      config?.by,
      columns[0],
    );
    const xField =
      normalizedKind === "trend" ? "month" : normalizedKind === "composition" ? compositionField : columns[0] || "label";
    const yField = "value";
    const mapping = buildDefaultCompositionMapping(config, detail, xField, yField);
    return {
      chartTag,
      props: {
        title: String(config?.title || ""),
        data: chartDataset,
        _mei: tableProps._mei,
        query_state: tableProps.query_state,
        supportRole: config?.supportRole,
        labelField: compositionField,
        topN: positiveInt(config?.top_n, config?.topN),
        mapping,
        ...buildAnalyticsChartPresentationProps(config, { mapping }),
      },
    };
  }

  async function mountAnalyticsChartSlot(root, detail, config, tabId, hostOverride = null) {
    const kind = explainMetricKind(config, tabId);
    const supportRole = nonEmptyString(config?.supportRole, config?.slotByTab?.[normalizeTabId(tabId)]?.supportRole);
    const cardMetricId = nonEmptyString(
      detail?.metric_id,
      detail?.__mei_runtime_ref?.metric_id,
      config?.tableMetricId,
    );
    const chartMetricId = nonEmptyString(
      config?.tableMetricId,
      resolveCompositionScopedMetricId(cardMetricId, tabId),
      config?.runtimeRef?.metricId,
      config?.runtimeRef?.metric_id,
    );
    const dedicatedChartMetric = isDedicatedExplainMetricId(chartMetricId, {
      supportRole: config?.supportRole ?? supportRole,
    });
    const sharedQueryStateId = nonEmptyString(
      config?.queryStateId,
      detail?.query_state_id,
      detail?.queryStateId,
    );
    if (kind === "composition" || supportRole === "composition" || kind === "trend" || supportRole === "trend") {
      // 过滤条与图表共享 query_state 时，composition/trend 需基于明细 rowset 重聚合，
      // 服务端已聚合的 explain dataframe 不含全部筛选维度。
      if (!sharedQueryStateId && dedicatedChartMetric) {
        if (await mountDrilldownChart(root, detail, config, tabId, hostOverride)) {
          return true;
        }
      }
      return mountDerivedDrilldownContent(root, detail, config, tabId, hostOverride);
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
    resetDrilldownChartSlotHost(host, resolveDrilldownChartSlotCaption(config));
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
    const props = applyAnalyticsTableRowDrilldown(buildDrilldownTableProps(detail, config), config);
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

  function buildFilterColumnCatalog(config, tableProps) {
    const schemaFields = Array.isArray(config?.filterSchema?.fields) ? config.filterSchema.fields : [];
    const detailFields = Array.isArray(config?.detailSlot?.fields) ? config.detailSlot.fields : [];
    const tableColumns = Array.isArray(tableProps?.columns) ? tableProps.columns : [];
    const fallbackColumns = Array.isArray(config?.columns) ? config.columns : [];
    const byColumn = new Map();
    for (const raw of [...detailFields, ...tableColumns, ...fallbackColumns]) {
      const column = String(raw || "").trim();
      if (!column || byColumn.has(column) || !isFilterableDetailColumn(column)) continue;
      byColumn.set(column, { key: column, label: column, column });
    }
    for (const field of schemaFields) {
      const column = nonEmptyString(field.column, field.key);
      if (!column) continue;
      byColumn.set(column, {
        key: nonEmptyString(field.key, column),
        label: field.label || field.key || column,
        column,
        control: nonEmptyString(field.control, field.type) || undefined,
        operator: nonEmptyString(field.operator, field.default_operator, field.defaultOperator),
        options_from: nonEmptyString(field.options_from, field.optionsFrom) || "rowset",
        options_field: nonEmptyString(field.options_field, field.optionsField, column),
        options: Array.isArray(field.options) ? field.options : undefined,
      });
    }
    return Array.from(byColumn.values());
  }

  function isFilterableDetailColumn(column) {
    const name = String(column || "").trim();
    if (!name) return false;
    if (/^序号$/.test(name)) return false;
    if (/条数$|金额$|人数$|^value$/i.test(name)) return false;
    if (/^\d{4}$/.test(name)) return false;
    if (/^month$/i.test(name)) return false;
    return true;
  }

  function buildAnalyticsFilterBarProps(config, detail) {
    const tableProps = buildDrilldownTableProps(detail, config) || {};
    const filterSchema = config?.filterSchema || {};
    const schemaFields = Array.isArray(filterSchema.fields) ? filterSchema.fields : [];
    const useSchemaCatalog = schemaFields.length > 0;
    const columnCatalog = useSchemaCatalog
      ? schemaFields.map((field) => ({
          key: nonEmptyString(field.key, field.column),
          label: field.label || field.key || field.column,
          column: nonEmptyString(field.column, field.key),
          control: nonEmptyString(field.control, field.type) || undefined,
          operator: nonEmptyString(field.operator, field.default_operator, field.defaultOperator),
          options_from: nonEmptyString(field.options_from, field.optionsFrom) || "rowset",
          options_field: nonEmptyString(field.options_field, field.optionsField, field.column),
          options: Array.isArray(field.options) ? field.options : undefined,
          placeholder: nonEmptyString(field.placeholder),
          visible: field.visible !== false,
        }))
      : buildFilterColumnCatalog(config, tableProps);
    const presetFilterCount = useSchemaCatalog
      ? Math.max(
          0,
          Number(
            filterSchema.presetFilterCount ??
              filterSchema.preset_filter_count ??
              filterSchema.defaultPresetCount ??
              3,
          ) || 0,
        )
      : 0;
    const rowsetDatasetId = nonEmptyString(
      filterSchema.rowsetDatasetId,
      config?.filterSchema?.rowsetDatasetId,
      tableProps?.dataset?.__mei_runtime_ref?.dataset_id,
      tableProps?.dataset?.id,
    );
    return {
      mode: "additive",
      live: false,
      title: nonEmptyString(filterSchema.title) || "筛选条件",
      default_collapsed: Boolean(filterSchema.defaultCollapsed),
      preset_filter_count: presetFilterCount,
      query_state: config?.queryStateId || undefined,
      default_filters: tableProps?.default_filters || undefined,
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
      column_catalog: columnCatalog,
      fields: columnCatalog,
    };
  }

  async function mountAnalyticsFilterBar(root, detail, config, hostOverride = null) {
    const host =
      hostOverride instanceof HTMLElement
        ? hostOverride
        : root.querySelector('[data-drilldown-filter-host="true"]');
    if (!(host instanceof HTMLElement)) return false;
    const filterProps = buildAnalyticsFilterBarProps(config, detail);
    const fieldCount = Array.isArray(filterProps?.column_catalog)
      ? filterProps.column_catalog.length
      : Array.isArray(filterProps?.fields)
        ? filterProps.fields.length
        : 0;
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

