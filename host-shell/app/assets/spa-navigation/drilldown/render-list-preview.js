  const LIST_PREVIEW_ROW_SELECT_EVENT = "mei:table-row-select";

  function resolveListPreviewFields(config) {
    const previewFields = cloneArray(config?.rowPreviewSlot?.fields || config?.previewSlot?.fields);
    if (previewFields.length) return previewFields;
    const listFields = cloneArray(config?.detailSlot?.fields || config?.listSlot?.fields);
    if (listFields.length) return listFields;
    return cloneArray(config?.columns);
  }

  function resolveListPreviewRowTitle(row, config) {
    if (!row || typeof row !== "object") return "条目详情";
    for (const key of ["label", "title", "name", "名称", "标题"]) {
      const value = row[key];
      if (value != null && String(value).trim()) {
        return String(value).trim();
      }
    }
    for (const field of resolveListPreviewFields(config)) {
      const key = String(field?.column || field?.key || field || "").trim();
      if (!key) continue;
      const value = row[key];
      if (value != null && String(value).trim()) {
        return String(value).trim();
      }
    }
    for (const value of Object.values(row)) {
      if (value != null && String(value).trim() && typeof value !== "object") {
        return String(value).trim();
      }
    }
    return "条目详情";
  }

  function renderListPreviewItemPanel(host, row, config) {
    if (!(host instanceof HTMLElement)) return;
    if (isTypicalCaseCardPreview(config)) {
      renderTypicalCaseCardPanel(host, row, config, config?.drilldownDetail);
      return;
    }
    if (isCaseDetailCardPreview(config)) {
      renderCaseDetailCardPanel(host, row, config, config?.drilldownDetail);
      return;
    }
    if (isSwimlanePreview(config)) {
      renderSwimlanePreviewPanel(host, row, config);
      return;
    }
    if (isDocumentPreview(config)) {
      renderDocumentPreviewPanel(host, row, config);
      return;
    }
    if (isVideoSubtitleCockpitPreview(config)) {
      renderVideoSubtitleCockpitPanel(host, row, config);
      return;
    }
    host.replaceChildren();
    if (!row || typeof row !== "object") {
      const empty = document.createElement("div");
      empty.className = "access-drilldown-list-preview-empty";
      empty.textContent = "点击清单中的条目查看详情";
      host.appendChild(empty);
      return;
    }
    const panel = document.createElement("div");
    panel.className = "access-drilldown-list-preview-panel";
    const title = document.createElement("div");
    title.className = "access-drilldown-list-preview-title";
    title.textContent = resolveListPreviewRowTitle(row, config);
    panel.appendChild(title);
    const fields = resolveListPreviewFields(config);
    const entries =
      fields.length > 0
        ? fields.map((field) => {
            const key = String(field?.column || field?.key || field || "").trim();
            if (!key) return null;
            const label = String(field?.label || key).trim();
            const value = row?.[key];
            return { label, value };
          })
        : Object.entries(row).map(([key, value]) => ({ label: key, value }));
    const list = document.createElement("dl");
    list.className = "access-drilldown-list-preview-fields";
    entries
      .filter(Boolean)
      .forEach((entry) => {
        const dt = document.createElement("dt");
        dt.textContent = entry.label;
        const dd = document.createElement("dd");
        dd.textContent =
          entry.value === null || entry.value === undefined ? "" : String(entry.value);
        list.appendChild(dt);
        list.appendChild(dd);
      });
    panel.appendChild(list);
    host.appendChild(panel);
  }

  function cleanupListPreviewDrilldownWatcher(root) {
    if (!(root instanceof HTMLElement)) return;
    const cleanup = root.__meiListPreviewQueryStateCleanup;
    if (typeof cleanup === "function") {
      cleanup();
    }
    root.__meiListPreviewQueryStateCleanup = null;
    const rowSelectCleanup = root.__meiListPreviewRowSelectCleanup;
    if (typeof rowSelectCleanup === "function") {
      rowSelectCleanup();
    }
    root.__meiListPreviewRowSelectCleanup = null;
  }

  async function renderListPreviewDrilldownContent(root, detail, config) {
    applyDrilldownOverlayMeta(root, config);
    setDrilldownOverlayStatus(root, "loading");
    cleanupListPreviewDrilldownWatcher(root);
    const listHost = root.querySelector('[data-drilldown-list-host="true"]');
    const previewHost = root.querySelector('[data-drilldown-preview-host="true"]');
    if (!(listHost instanceof HTMLElement) || !(previewHost instanceof HTMLElement)) {
      setDrilldownOverlayStatus(root, "error", {
        message: "清单预览看板缺少列表或预览挂载节点",
        phase: "list_preview_host_missing",
        detail,
        config,
      });
      return false;
    }
    listHost.replaceChildren();
    renderListPreviewItemPanel(previewHost, null, config);
    const listSlot = config?.listSlot || config?.detailSlot;
    const listConfig = listSlot
      ? {
          ...resolveDrilldownTabConfig(config, listSlot.id),
          listPreviewDrilldown: true,
          queryStateId: config.queryStateId,
          columns: cloneArray(listSlot.fields).length
            ? cloneArray(listSlot.fields)
            : cloneArray(resolveDrilldownTabConfig(config, listSlot.id).columns),
        }
      : { ...config, listPreviewDrilldown: true };
    try {
      const filterHost = root.querySelector(
        '[data-drilldown-body-mode="list-preview"] [data-drilldown-filter-host="true"]',
      );
      await mountAnalyticsFilterBar(root, detail, config, filterHost);
      const tableOk = await mountDrilldownTable(root, detail, listConfig, listHost);
      if (!tableOk) {
        setDrilldownOverlayStatus(root, "error", {
          message: "清单预览明细表挂载失败",
          phase: "list_preview_table_mount_failed",
          detail,
          config: listConfig,
        });
        return false;
      }
      const onRowSelect = (event) => {
        if (event?.detail?.query_state_id && event.detail.query_state_id !== config?.queryStateId) {
          return;
        }
        // 右侧预览重挂可能带动左侧壳滚动复位；先记下再钉回。
        const scrollSnapshots = [];
        let node = listHost;
        while (node instanceof HTMLElement) {
          if (node.scrollHeight > node.clientHeight + 1 || node.scrollWidth > node.clientWidth + 1) {
            scrollSnapshots.push({ el: node, top: node.scrollTop, left: node.scrollLeft });
          }
          node = node.parentElement;
          if (node === root) break;
        }
        const table = listHost.querySelector("mei-cockpit-data-table");
        const tableScroll = table?.shadowRoot?.querySelector?.(".table-scroll");
        if (tableScroll instanceof HTMLElement) {
          scrollSnapshots.push({
            el: tableScroll,
            top: tableScroll.scrollTop,
            left: tableScroll.scrollLeft,
          });
        }
        renderListPreviewItemPanel(previewHost, event?.detail?.row || null, config);
        const restore = () => {
          for (const entry of scrollSnapshots) {
            if (!(entry.el instanceof HTMLElement)) continue;
            if (entry.el.scrollTop !== entry.top) entry.el.scrollTop = entry.top;
            if (entry.el.scrollLeft !== entry.left) entry.el.scrollLeft = entry.left;
          }
        };
        restore();
        if (typeof requestAnimationFrame === "function") {
          requestAnimationFrame(() => {
            restore();
            requestAnimationFrame(restore);
          });
        }
      };
      listHost.addEventListener(LIST_PREVIEW_ROW_SELECT_EVENT, onRowSelect);
      root.__meiListPreviewRowSelectCleanup = () => {
        listHost.removeEventListener(LIST_PREVIEW_ROW_SELECT_EVENT, onRowSelect);
      };
      const queryStateId = nonEmptyString(config?.queryStateId, detail?.query_state_id, detail?.queryStateId);
      if (queryStateId) {
        const onQueryStateChange = (event) => {
          if (event?.detail?.id !== queryStateId) return;
          if (!(root instanceof HTMLElement) || root.hasAttribute("hidden")) return;
          // 明细表已 subscribeQueryState；不要 remount，否则与进行中请求竞态盖掉筛选结果。
          renderListPreviewItemPanel(previewHost, null, config);
          dispatchPreviewUpdated("drilldown");
        };
        window.addEventListener("mei:query-state-change", onQueryStateChange);
        root.__meiListPreviewQueryStateCleanup = () => {
          window.removeEventListener("mei:query-state-change", onQueryStateChange);
        };
      }
      setDrilldownOverlayStatus(root, "ready");
      dispatchPreviewUpdated("drilldown");
      return true;
    } catch (error) {
      recordPopupDebugIssue({
        level: "error",
        message: String(error?.message || error || "清单预览看板渲染失败"),
        phase: "list_preview_render_error",
        detail,
        config,
        root,
        stack: error?.stack || "",
      });
      setDrilldownOverlayStatus(root, "error");
      return false;
    }
  }

