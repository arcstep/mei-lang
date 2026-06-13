  function renderDrilldownContent(root, detail, config, tabId, hostOverride = null) {
    const activeConfig = resolveDrilldownTabConfig(config, tabId);
    applyDrilldownOverlayMeta(root, activeConfig);
    const host =
      hostOverride instanceof HTMLElement
        ? hostOverride
        : root.querySelector('[data-drilldown-table-host="true"]');
    if (!(host instanceof HTMLElement)) {
      return false;
    }
    const normalizedTab = normalizeTabId(tabId);
    if (config?.genericDrilldown) {
      const slot = config?.slotByTab?.[normalizedTab];
      if (slot?.component === "metric_card") {
        host.replaceChildren(createDrilldownSummaryNode(activeConfig, normalizedTab));
        setDrilldownOverlayStatus(root, "ready");
        return true;
      }
      if (slot?.component === "summary") {
        host.replaceChildren(createDrilldownSummaryNode(activeConfig, normalizedTab));
        setDrilldownOverlayStatus(root, "ready");
        return true;
      }
    }
    const kindTab = explainMetricKind(config, tabId);
    const tabOverride = config?.tabMetrics?.[normalizedTab] || config?.tabMetrics?.[kindTab];
    const hasCustomMetricSource = hasTabMetricDataSource(tabOverride);
    if (
      isDrilldownSummaryTab(tabId, config) ||
      (isDrilldownAnalysisTab(tabId, config) && !hasCustomMetricSource)
    ) {
      if (isDrilldownAnalysisTab(tabId, config) && !hasCustomMetricSource) {
        const summaryConfig = {
          ...activeConfig,
          note: nonEmptyString(activeConfig.note, unconfiguredTabNote(tabId)),
        };
        host.replaceChildren(createDrilldownSummaryNode(summaryConfig, tabId));
        setDrilldownOverlayStatus(root, "ready");
        return true;
      }
      host.replaceChildren(createDrilldownSummaryNode(activeConfig, tabId));
      setDrilldownOverlayStatus(root, "ready");
      return true;
    }
    if (isDrilldownAnalysisTab(tabId, config) || config?.genericDrilldown) {
      host.replaceChildren();
      setDrilldownOverlayStatus(root, "loading");
      const preferTableFirst =
        (config?.genericDrilldown && config?.slotByTab?.[normalizedTab]?.component === "data_table") ||
        (typeof window.__meiDatasetRuntime?.isYearMonthMatrixMetricConfig === "function" &&
          window.__meiDatasetRuntime.isYearMonthMatrixMetricConfig(activeConfig));
      const mountAnalysisContent = async () => {
        if (config?.genericDrilldown && config?.slotByTab?.[normalizedTab]?.supportRole === "composition") {
          if (await mountDerivedDrilldownContent(root, detail, activeConfig, tabId, host)) {
            return true;
          }
        }
        if (preferTableFirst && (await mountDrilldownTable(root, detail, activeConfig, host))) {
          return true;
        }
        if (await mountDrilldownChart(root, detail, activeConfig, tabId, host)) {
          return true;
        }
        if (!preferTableFirst && (await mountDrilldownTable(root, detail, activeConfig, host))) {
          return true;
        }
        if (config?.genericDrilldown && (await mountDerivedDrilldownContent(root, detail, activeConfig, tabId, host))) {
          return true;
        }
        return false;
      };
      mountAnalysisContent()
        .then(async (mounted) => {
          if (mounted) {
            setDrilldownOverlayStatus(root, "ready");
            dispatchPreviewUpdated("drilldown");
            return;
          }
          recordPopupDebugIssue({
            level: "error",
            message: `popup panel 表格挂载失败：${normalizedTab || tabId}`,
            phase: "table_mount_failed",
            detail,
            config: activeConfig,
            datasetId: activeConfig?.datasetId,
            metricId: activeConfig?.tableMetricId,
          });
          setDrilldownOverlayStatus(root, "error");
        })
        .catch(async (error) => {
          recordPopupDebugIssue({
            level: "error",
            message: String(error?.message || error || "图表 explain 块渲染失败"),
            phase: "chart_render_error",
            detail,
            config: activeConfig,
            datasetId: activeConfig?.datasetId,
            metricId: activeConfig?.tableMetricId,
          });
          if (await mountDrilldownTable(root, detail, activeConfig, host)) {
            setDrilldownOverlayStatus(root, "ready");
            dispatchPreviewUpdated("drilldown");
            return;
          }
          setDrilldownOverlayStatus(root, "error");
        });
      return true;
    }
    setDrilldownOverlayStatus(root, "loading");
    mountDrilldownTable(root, detail, activeConfig, host)
      .then((mounted) => {
        if (mounted) {
          setDrilldownOverlayStatus(root, "ready");
          dispatchPreviewUpdated("drilldown");
          return;
        }
        recordPopupDebugIssue({
          level: "error",
          message: `popup panel 表格挂载失败：${normalizedTab || tabId}`,
          phase: "table_mount_failed",
          detail,
          config: activeConfig,
          datasetId: activeConfig?.datasetId,
          metricId: activeConfig?.tableMetricId,
        });
        setDrilldownOverlayStatus(root, "error");
      })
      .catch((error) => {
        recordPopupDebugIssue({
          level: "error",
          message: String(error?.message || error || "明细表渲染失败"),
          phase: "table_render_error",
          detail,
          config: activeConfig,
          datasetId: activeConfig?.datasetId,
          metricId: activeConfig?.tableMetricId,
        });
        setDrilldownOverlayStatus(root, "error");
      });
    return true;
  }

  function renderDrilldownTabs(root, detail, config, hostOverride = null, contentHostOverride = null) {
    const tabsHost =
      hostOverride instanceof HTMLElement
        ? hostOverride
        : root.querySelector('[data-drilldown-tabs="true"]');
    if (!(tabsHost instanceof HTMLElement)) {
      return defaultActiveDrilldownTab(config?.tabs || []);
    }
    const normalizedTabs = Array.from(
      new Set(
        (Array.isArray(config?.tabs) ? config.tabs : [])
          .map((tab) => normalizeTabId(tab))
          .filter(Boolean),
      ),
    );
    const tabs = normalizedTabs.length
      ? normalizedTabs
      : [defaultActiveDrilldownTab(defaultDrilldownTabs(config?.explainKind, { hasDetail: true }))];
    const preferredTab = normalizeTabId(
      nonEmptyString(
        config?.popup?.entry,
        config?.popup?.entry_tab,
        config?.popup?.entryTab,
        config?.popup?.focus,
        config?.link?.entry,
        config?.link?.defaultFocus,
      ),
    );
    const defaultFromSlots = config?.genericDrilldown
      ? normalizeTabId(
          Object.values(config?.slotByTab || {}).find((slot) => slot?.default)?.id,
        )
      : "";
    const activeTab =
      (preferredTab && tabs.includes(preferredTab) ? preferredTab : "") ||
      (defaultFromSlots && tabs.includes(defaultFromSlots) ? defaultFromSlots : "") ||
      defaultActiveDrilldownTab(tabs);
    tabsHost.replaceChildren();
    tabsHost.toggleAttribute("hidden", tabs.length <= 1);
    tabs.forEach((tab) => {
      const explainMetric = explainMetricForTab(config, tab);
      const tabConfig = resolveDrilldownTabConfig(config, tab);
      const button = document.createElement("button");
      button.type = "button";
      button.className = "access-drilldown-tab-button";
      button.dataset.drilldownTab = tab;
      button.setAttribute("role", "tab");
      button.setAttribute("aria-selected", tab === activeTab ? "true" : "false");
      const slotLabel =
        config?.genericDrilldown && config?.slotByTab?.[tab]
          ? nonEmptyString(config.slotByTab[tab].label)
          : "";
      button.textContent = nonEmptyString(
        slotLabel,
        explainMetric?.label,
        tabConfig?.title,
        tabConfig?.label,
        drilldownTabLabel(explainMetric?.kind || tab, tabConfig),
      );
      button.addEventListener("click", () => {
        if (button.getAttribute("aria-selected") === "true") return;
        tabsHost
          .querySelectorAll(".access-drilldown-tab-button")
          .forEach((node) => node.setAttribute("aria-selected", node === button ? "true" : "false"));
        renderDrilldownContent(root, detail, config, tab, contentHostOverride);
      });
      tabsHost.appendChild(button);
    });
    return activeTab;
  }

