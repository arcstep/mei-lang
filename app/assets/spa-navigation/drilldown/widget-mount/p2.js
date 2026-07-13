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
    const props = applyAnalyticsTableRowDrilldown(buildDrilldownTableProps(detail, config), config, detail);
    if (!props) {
      recordPopupDebugIssue({
        level: "error",
        message: "未解析到下钻明细表所需 scene_id 或 dataset_id",
        phase: "table_mount_setup",
        detail,
        config,
        datasetId: resolveDrilldownDatasetId(detail, config),
        metricId: nonEmptyString(detail?.metric_id, detail?.__mei_runtime_ref?.metric_id),
        root,
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
        root,
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
      query_state: nonEmptyString(
        config?.queryStateId,
        detail?.query_state_id,
        detail?.queryStateId,
        config?.tableMetricId ? `drilldown::${config.tableMetricId}` : "",
        config?.metricId ? `drilldown::${config.metricId}` : "",
      ) || undefined,
      default_filters: tableProps?.default_filters || undefined,
      rowset_dataset_id: rowsetDatasetId || undefined,
      dataset: rowsetDatasetId
        ? {
            id: rowsetDatasetId,
            shape: "table",
            __mei_runtime_ref: {
              dataset_id: rowsetDatasetId,
              scene_id: nonEmptyString(config?.runtimeSceneId, config?.hostSceneId, config?.sceneId),
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

  async function remountStructuredAnalyticsDetailZones(root, detail, config, resolveZoneHost) {
    const slotZones = sceneShellZonesByRole(config?.sceneShell, "slots");
    let ok = true;
    for (const zone of slotZones) {
      const zoneSlots = Array.isArray(config?.slotsByZone?.[zone.id]) ? config.slotsByZone[zone.id] : [];
      if (!zoneSlots.length || zoneSlots.every((slot) => slot.component === "chart")) {
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
      const zoneOk = await mountStructuredSlotZone(root, detail, config, zone, host);
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
      Promise.all([
        remountStructuredAnalyticsChartZones(root, detail, config, resolveZoneHost),
        remountStructuredAnalyticsDetailZones(root, detail, config, resolveZoneHost),
      ])
        .then(([chartsOk, detailOk]) => {
          if ((!chartsOk && !detailOk) || currentSeq !== refreshSeq) return;
          dispatchPreviewUpdated("drilldown");
        })
        .catch((error) => {
          recordPopupDebugIssue({
            level: "error",
            message: String(error?.message || error || "分析型看板刷新失败"),
            phase: "analytics_chart_refresh_error",
            detail,
            config,
            root,
            stack: error?.stack || "",
          });
        });
    };
    window.addEventListener("mei:query-state-change", onQueryStateChange);
    root.__meiAnalyticsQueryStateCleanup = () => {
      window.removeEventListener("mei:query-state-change", onQueryStateChange);
    };
  }

