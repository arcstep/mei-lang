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
    const dedicatedDatasetId = config?.structuredBoard
      ? nonEmptyString(
          tableProps.dataset?.__mei_runtime_ref?.dataset_id,
          runtimeRefConfig.datasetId,
          runtimeRefConfig.dataset_id,
        )
      : nonEmptyString(
          runtimeRefConfig.datasetId,
          runtimeRefConfig.dataset_id,
          tableProps.dataset?.__mei_runtime_ref?.dataset_id,
        );
    const chartDataset =
      dedicatedChartMetric && chartMetricId
        ? {
            __mei_runtime_ref: {
              kind: "metric",
              metric_id: chartMetricId,
              dataset_id: dedicatedDatasetId,
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
      // 仅当 query_state 上已有有效筛选时，才基于明细 rowset 客户端重聚合；
      // 默认无筛选时应走服务端 explain 指标（全量聚合），避免误用分页 rowset 样本。
      const needsFilterAwareReaggregate = hasActiveDrilldownQueryFilters(sharedQueryStateId);
      if (!needsFilterAwareReaggregate && dedicatedChartMetric) {
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
