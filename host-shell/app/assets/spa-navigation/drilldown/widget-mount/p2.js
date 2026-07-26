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
    // 作者声明字段优先（含 control / contains_any 等），并占 catalog 前部 → 默认预置取前 N 个
    for (const field of schemaFields) {
      const mapped = mapAnalyticsFilterField(field);
      const column = nonEmptyString(mapped.column, mapped.key);
      if (!column) continue;
      byColumn.set(column, mapped);
    }
    // 明细表全部可筛列并入候选；已在 schema 中的列保留作者配置
    for (const raw of [...detailFields, ...tableColumns, ...fallbackColumns]) {
      const column = String(raw || "").trim();
      if (!column || byColumn.has(column) || !isFilterableDetailColumn(column)) continue;
      const control = inferDefaultControlForColumn(column);
      byColumn.set(column, {
        key: column,
        label: column,
        column,
        control,
        options_from: control === "text" ? undefined : "rowset",
        options_field: column,
        visible: true,
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

  /** 自动并入的表列：给合理 control，否则无法拉 facet / 多选。 */
  function inferDefaultControlForColumn(column) {
    const name = String(column || "").trim();
    if (/时间$|日期$|年月/.test(name)) return "month_multi_select";
    if (/ID$|编号$|编码$/.test(name)) return "text";
    if (/描述$|说明$|内容$|意见$|表现形式$|存在的问题$/.test(name)) return "text";
    return "multi_select";
  }

  function mapAnalyticsFilterField(field) {
    const key = nonEmptyString(field.key, field.column);
    const column = nonEmptyString(field.column, field.key);
    const declaredOptions = Array.isArray(field.options) ? field.options : [];
    const optionsFrom = nonEmptyString(field.options_from, field.optionsFrom) || "rowset";
    // 风险等级等组合面值必须走 rowset facet，才能带计数并按计数排序；
    // 不要再注入无 count 的 static 组合列表。
    return {
      key,
      label: field.label || field.key || field.column,
      column,
      control: nonEmptyString(field.control, field.type) || undefined,
      operator: nonEmptyString(field.operator, field.default_operator, field.defaultOperator),
      options_from: optionsFrom,
      options_field: nonEmptyString(field.options_field, field.optionsField, field.column),
      options: declaredOptions.length > 0 ? declaredOptions : undefined,
      placeholder: nonEmptyString(field.placeholder),
      visible: field.visible !== false,
    };
  }

  function buildAnalyticsFilterBarProps(config, detail) {
    const tableProps = buildDrilldownTableProps(detail, config) || {};
    const filterSchema = config?.filterSchema || {};
    // 全列候选 = 明细表列 ∪ 作者 filter_schema.fields；作者字段排前，默认预置取前 ~3 个
    const columnCatalog = buildFilterColumnCatalog(config, tableProps);
    const presetFilterCount = Math.max(
      0,
      Number(
        filterSchema.presetFilterCount ??
          filterSchema.preset_filter_count ??
          filterSchema.defaultPresetCount ??
          3,
      ) || 0,
    );
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
    const node = document.createElement("mei-dataset-filter-bar-v2");
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
      // 明细表已 subscribeQueryState 自行刷新；此处若 remount 明细，会与进行中的
      // 过滤请求竞态（无过滤的旧请求后返回，盖掉筛选结果 → 看起来像过滤失效）。
      remountStructuredAnalyticsChartZones(root, detail, config, resolveZoneHost)
        .then((chartsOk) => {
          if (!chartsOk || currentSeq !== refreshSeq) return;
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

