  function cleanupStructuredDrilldownWatcher(root) {
    if (!(root instanceof HTMLElement)) return;
    cleanupAnalyticsDrilldownWatcher(root);
    cleanupListPreviewDrilldownWatcher(root);
    const queryCleanup = root.__meiStructuredQueryStateCleanup;
    if (typeof queryCleanup === "function") {
      queryCleanup();
    }
    root.__meiStructuredQueryStateCleanup = null;
    const rowCleanup = root.__meiStructuredRowSelectionCleanup;
    if (typeof rowCleanup === "function") {
      rowCleanup();
    }
    root.__meiStructuredRowSelectionCleanup = null;
  }

  function boardLayoutAreasCss(areas = []) {
    if (!Array.isArray(areas) || areas.length === 0) return "";
    return areas
      .filter((row) => Array.isArray(row) && row.length > 0)
      .map((row) => `"${row.map((entry) => String(entry || "").trim() || ".").join(" ")}"`)
      .join(" ");
  }

  function applySceneShellLayout(node, layout) {
    if (!(node instanceof HTMLElement)) return;
    node.style.display = "grid";
    node.style.gridTemplateColumns =
      Array.isArray(layout?.columns) && layout.columns.length ? layout.columns.join(" ") : "";
    node.style.gridTemplateRows =
      Array.isArray(layout?.rows) && layout.rows.length ? layout.rows.join(" ") : "";
    node.style.gridTemplateAreas = boardLayoutAreasCss(layout?.areas);
    node.style.gap = nonEmptyString(layout?.gap);
    node.style.padding = nonEmptyString(layout?.padding);
  }

  function ensureStructuredDrilldownZoneHosts(root, sceneShell) {
    const layoutHost = root.querySelector('[data-drilldown-structured-layout="true"]');
    if (!(layoutHost instanceof HTMLElement) || !sceneShell) return null;
    layoutHost.replaceChildren();
    layoutHost.dataset.shellLayoutMode = String(sceneShell.layoutMode || "");
    applySceneShellLayout(layoutHost, sceneShell.layout);
    const zoneHosts = {};
    const zones = Array.isArray(sceneShell.zones) ? sceneShell.zones : [];
    const childrenByParent = new Map();
    zones.forEach((zone) => {
      const parent = nonEmptyString(zone?.parent);
      if (!childrenByParent.has(parent)) childrenByParent.set(parent, []);
      childrenByParent.get(parent).push(zone);
    });

    const createZoneNode = (zone) => {
      const wrapper = document.createElement(zone.role === "filter" || zone.role === "row_preview" ? "aside" : "div");
      wrapper.className = "access-drilldown-shell-zone";
      wrapper.dataset.shellZoneId = zone.id;
      wrapper.dataset.shellZoneRole = zone.role;
      if (zone.area) {
        wrapper.style.gridArea = zone.area;
      }
      if (zone.role === "container") {
        wrapper.classList.add("access-drilldown-shell-zone--container");
        applySceneShellLayout(wrapper, zone.layout);
      } else {
        const host =
          zone.role === "filter" || zone.role === "tab_bar"
            ? wrapper
            : (() => {
                const surface = document.createElement("div");
                surface.className = "access-drilldown-shell-surface";
                wrapper.appendChild(surface);
                return surface;
              })();
        host.classList.add("access-drilldown-shell-host");
        host.dataset.drilldownZoneHost = zone.id;
        zoneHosts[zone.id] = host;
      }
      const children = childrenByParent.get(zone.id) || [];
      children.forEach((child) => wrapper.appendChild(createZoneNode(child)));
      return wrapper;
    };

    const rootZones = childrenByParent.get("") || [];
    rootZones.forEach((zone) => layoutHost.appendChild(createZoneNode(zone)));
    return zoneHosts;
  }

  async function mountStructuredSlotZone(root, detail, config, zone, host) {
    if (!(host instanceof HTMLElement)) return false;
    host.replaceChildren();
    host.style.gridTemplateColumns = "";
    const zoneSlots = Array.isArray(config?.slotsByZone?.[zone.id]) ? config.slotsByZone[zone.id] : [];
    if (!zoneSlots.length) {
      return !zone.required;
    }
    if (zoneSlots.every((slot) => slot.component === "chart")) {
      zoneSlots.forEach((slot, index) => {
        const slotEl = document.createElement("div");
        slotEl.className = "access-drilldown-shell-slot access-drilldown-shell-slot--chart";
        slotEl.dataset.chartSlotIndex = String(index);
        host.appendChild(slotEl);
      });
      host.style.display = "grid";
      host.style.gridTemplateColumns =
        zoneSlots.length > 1 ? `repeat(${zoneSlots.length}, minmax(0, 1fr))` : "1fr";
      return mountAnalyticsChartSlots(root, detail, config, zoneSlots, host);
    }
    const primarySlot = zoneSlots[0];
    const baseConfig = resolveDrilldownTabConfig(config, primarySlot.id);
    const slotConfig = {
      ...baseConfig,
      queryStateId: config.queryStateId,
      hasChartZone: config.hasChartZone,
      hasRowPreviewZone: config.hasRowPreviewZone,
      tableMetricId: nonEmptyString(primarySlot.metricId, baseConfig.tableMetricId, config.tableMetricId),
      datasetId: nonEmptyString(primarySlot.datasetId, baseConfig.datasetId, config.datasetId),
      columns: cloneArray(primarySlot.fields).length
        ? cloneArray(primarySlot.fields)
        : cloneArray(baseConfig.columns),
      rowSelectionMode:
        config?.rowPreviewSourceZoneId && config.rowPreviewSourceZoneId === zone.id ? "single" : "",
    };
    if (primarySlot.component === "data_table") {
      return mountDrilldownTable(root, detail, slotConfig, host);
    }
    if (primarySlot.component === "summary" || primarySlot.component === "metric_card") {
      host.appendChild(createDrilldownSummaryNode(slotConfig, primarySlot.id));
      return true;
    }
    return false;
  }

  function mountStructuredRowPreviewZone(root, zoneHosts, config) {
    const previewZoneId = nonEmptyString(config?.rowPreviewZoneId);
    const sourceZoneId = nonEmptyString(config?.rowPreviewSourceZoneId);
    if (!previewZoneId || !sourceZoneId) return;
    const previewHost = zoneHosts?.[previewZoneId];
    const sourceHost = zoneHosts?.[sourceZoneId];
    if (!(previewHost instanceof HTMLElement) || !(sourceHost instanceof HTMLElement)) return;
    renderListPreviewItemPanel(previewHost, null, config);
    const onRowSelect = (event) => {
      if (event?.detail?.query_state_id && event.detail.query_state_id !== config?.queryStateId) {
        return;
      }
      renderListPreviewItemPanel(previewHost, event?.detail?.row || null, config);
    };
    sourceHost.addEventListener(LIST_PREVIEW_ROW_SELECT_EVENT, onRowSelect);
    root.__meiStructuredRowSelectionCleanup = () => {
      sourceHost.removeEventListener(LIST_PREVIEW_ROW_SELECT_EVENT, onRowSelect);
    };
  }

  function renderStructuredTabZones(root, detail, config, zoneHosts) {
    const tabsHost = zoneHosts?.[config?.tabBarZoneId];
    const contentHost = zoneHosts?.[config?.tabContentZoneId];
    if (!(tabsHost instanceof HTMLElement) || !(contentHost instanceof HTMLElement)) {
      return false;
    }
    const activeTab = renderDrilldownTabs(root, detail, config, tabsHost, contentHost);
    return renderDrilldownContent(root, detail, config, activeTab, contentHost);
  }

  async function renderStructuredDrilldownContent(root, detail, config) {
    applyDrilldownOverlayMeta(root, config);
    setDrilldownOverlayStatus(root, "loading");
    cleanupStructuredDrilldownWatcher(root);
    const zoneHosts = ensureStructuredDrilldownZoneHosts(root, config?.sceneShell);
    if (!zoneHosts) {
      setDrilldownOverlayStatus(root, "error");
      return false;
    }
    try {
      if (config?.sceneShell?.layoutMode === "generic_tabs") {
        const ok = renderStructuredTabZones(root, detail, config, zoneHosts);
        if (!ok) {
          setDrilldownOverlayStatus(root, "error");
          return false;
        }
        return true;
      }
      const filterZone = sceneShellZonesByRole(config?.sceneShell, "filter")[0] || null;
      if (filterZone && zoneHosts[filterZone.id] instanceof HTMLElement) {
        await mountAnalyticsFilterBar(root, detail, config, zoneHosts[filterZone.id]);
      }
      const slotZones = sceneShellZonesByRole(config?.sceneShell, "slots");
      for (const zone of slotZones) {
        const ok = await mountStructuredSlotZone(root, detail, config, zone, zoneHosts[zone.id]);
        if (!ok) {
          setDrilldownOverlayStatus(root, "error");
          return false;
        }
      }
      mountStructuredRowPreviewZone(root, zoneHosts, config);
      const queryStateId = nonEmptyString(config?.queryStateId, detail?.query_state_id, detail?.queryStateId);
      if (queryStateId) {
        const onQueryStateChange = (event) => {
          if (event?.detail?.id !== queryStateId) return;
          if (!(root instanceof HTMLElement) || root.hasAttribute("hidden")) return;
          renderStructuredDrilldownContent(root, detail, config)
            .then((ok) => {
              if (ok) dispatchPreviewUpdated("drilldown");
            })
            .catch((error) => {
              recordPopupDebugIssue({
                level: "error",
                message: String(error?.message || error || "通用下钻壳刷新失败"),
                phase: "structured_shell_refresh_error",
                detail,
                config,
              });
            });
        };
        window.addEventListener("mei:query-state-change", onQueryStateChange);
        root.__meiStructuredQueryStateCleanup = () => {
          window.removeEventListener("mei:query-state-change", onQueryStateChange);
        };
      }
      setDrilldownOverlayStatus(root, "ready");
      dispatchPreviewUpdated("drilldown");
      return true;
    } catch (error) {
      recordPopupDebugIssue({
        level: "error",
        message: String(error?.message || error || "通用下钻壳渲染失败"),
        phase: "structured_shell_render_error",
        detail,
        config,
      });
      setDrilldownOverlayStatus(root, "error");
      return false;
    }
  }

