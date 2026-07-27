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
    const allowExtra =
      config?.filterSchema?.allowExtra === true || config?.filterSchema?.allow_extra === true;
    const byColumn = new Map();
    // 作者声明字段优先（含 control / contains_any 等），并占 catalog 前部 → 默认预置取前 N 个
    for (const field of schemaFields) {
      const mapped = mapAnalyticsFilterField(field);
      const column = nonEmptyString(mapped.column, mapped.key);
      if (!column) continue;
      byColumn.set(column, mapped);
    }
    // 作者声明 allow_extra=false 时只保留 fields，禁止表列（旧「监督类别」/行权类别）抢预置位。
    // 完全没有 filter_schema 时仍回退表列（兼容未声明过滤的看板）。
    const authorSchemaPresent = Boolean(
      config?.filterSchema &&
        (schemaFields.length > 0 ||
          config.filterSchema.allowExtra === false ||
          config.filterSchema.allow_extra === false ||
          config.filterSchema.presetFilterCount != null ||
          config.filterSchema.preset_filter_count != null ||
          config.filterSchema.defaultCollapsed === false ||
          config.filterSchema.default_collapsed === false),
    );
    if (allowExtra || (schemaFields.length === 0 && !authorSchemaPresent)) {
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
    const authoredOptionsFrom = nonEmptyString(field.options_from, field.optionsFrom);
    // 已声明静态枚举时勿默认成 rowset，否则 additive 会走空 facet 并盖掉静态项。
    const optionsFrom =
      authoredOptionsFrom || (declaredOptions.length > 0 ? "static" : "rowset");
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
    // issue_handling_list 等是 metric 派生 rowset：无 metric_id 时 rows/facets 皆空。
    // filter-bar 选项请求必须复用明细表的 metric runtime ref（含 ::__scalar_rowset__）。
    const tableRuntimeRef =
      tableProps?.dataset?.__mei_runtime_ref && typeof tableProps.dataset.__mei_runtime_ref === "object"
        ? tableProps.dataset.__mei_runtime_ref
        : {};
    const filterMetricId = nonEmptyString(
      tableRuntimeRef.metric_id,
      tableRuntimeRef.metricId,
      typeof resolveCardMetricRowsetId === "function"
        ? resolveCardMetricRowsetId(
            nonEmptyString(config?.tableMetricId, config?.metricId, detail?.metric_id),
          )
        : "",
    );
    const filterRuntimeRef = rowsetDatasetId
      ? {
          ...tableRuntimeRef,
          kind: filterMetricId ? "metric" : nonEmptyString(tableRuntimeRef.kind, "data") || "data",
          dataset_id: rowsetDatasetId,
          ...(filterMetricId ? { metric_id: filterMetricId } : {}),
          scene_id: nonEmptyString(
            tableRuntimeRef.scene_id,
            config?.runtimeSceneId,
            config?.hostSceneId,
            config?.sceneId,
          ),
          scene_path: nonEmptyString(tableRuntimeRef.scene_path, config?.runtimeSceneFile),
        }
      : null;
    // Seed only — 不要把 tableProps 里误混的 identity 当种子（table 已拆分）。
    const seedFilters = (() => {
      const seed = tableProps?.default_filters;
      if (seed && typeof seed === "object" && !Array.isArray(seed)) return seed;
      const fromDetail = detail?.default_filters;
      if (fromDetail && typeof fromDetail === "object" && !Array.isArray(fromDetail)) return fromDetail;
      const fromParams = config?.params?.default_filters;
      if (fromParams && typeof fromParams === "object" && !Array.isArray(fromParams)) return fromParams;
      return null;
    })();
    const scopeFilters = (() => {
      const fromTable = tableProps?.scope_filters;
      if (fromTable && typeof fromTable === "object" && !Array.isArray(fromTable)) return fromTable;
      const fromConfig = config?.scopeFilters || config?.scope_filters;
      if (fromConfig && typeof fromConfig === "object" && !Array.isArray(fromConfig)) return fromConfig;
      const fromDetail = detail?.scope_filters || detail?.scopeFilters;
      if (fromDetail && typeof fromDetail === "object" && !Array.isArray(fromDetail)) return fromDetail;
      return null;
    })();
    const identityFilters = (() => {
      const fromTable = tableProps?.drilldown_filters;
      if (fromTable && typeof fromTable === "object" && !Array.isArray(fromTable)) return fromTable;
      const fromDetail = detail?.drilldown_filters;
      if (fromDetail && typeof fromDetail === "object" && !Array.isArray(fromDetail)) return fromDetail;
      return null;
    })();
    return {
      mode: "additive",
      live: false,
      title: nonEmptyString(filterSchema.title) || "筛选条件",
      default_collapsed: Boolean(filterSchema.defaultCollapsed),
      allow_extra: filterSchema.allowExtra === true,
      preset_filter_count: presetFilterCount,
      query_state: nonEmptyString(
        config?.queryStateId,
        detail?.query_state_id,
        detail?.queryStateId,
        config?.tableMetricId ? `drilldown::${config.tableMetricId}` : "",
        config?.metricId ? `drilldown::${config.metricId}` : "",
      ) || undefined,
      default_filters: seedFilters || undefined,
      scope_filters: scopeFilters || undefined,
      drilldown_filters: identityFilters || undefined,
      rowset_dataset_id: rowsetDatasetId || undefined,
      dataset: filterRuntimeRef
        ? {
            id: rowsetDatasetId,
            shape: filterMetricId ? "dataframe" : "table",
            __mei_runtime_ref: filterRuntimeRef,
          }
        : tableProps.dataset,
      data: filterRuntimeRef
        ? {
            id: rowsetDatasetId,
            __mei_runtime_ref: filterRuntimeRef,
          }
        : tableProps.dataset,
      _mei: {
        ...(tableProps._mei && typeof tableProps._mei === "object" ? tableProps._mei : {}),
        filter_layers: {
          seed: seedFilters || {},
          scope: scopeFilters || {},
          identity: identityFilters || {},
        },
      },
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

