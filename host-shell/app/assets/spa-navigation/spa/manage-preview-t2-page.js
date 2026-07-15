  const MANAGE_PREVIEW_UPDATED_EVENT = "meilang:preview-updated";
  const MAX_BOARD_MOUNT_POOL = 6;
  const boardMountPool = new Map();

  function boardMountPoolKey(doc, sceneId, targetFile) {
    return `${sceneId}::${targetFile}`;
  }

  function stashBoardMount(mountKey, surface) {
    if (!(surface instanceof HTMLElement)) return;
    if (boardMountPool.size >= MAX_BOARD_MOUNT_POOL && !boardMountPool.has(mountKey)) {
      const firstKey = boardMountPool.keys().next().value;
      if (firstKey) boardMountPool.delete(firstKey);
    }
    boardMountPool.set(mountKey, surface.innerHTML);
  }

  function restoreBoardMount(mountKey, surface) {
    if (!(surface instanceof HTMLElement) || !boardMountPool.has(mountKey)) return false;
    surface.innerHTML = boardMountPool.get(mountKey);
    surface.dataset.meiPreviewBoardMounted = mountKey;
    surface.classList.add("preview-board-mounted");
    refreshManagePreviewBoardCharts(surface);
    return true;
  }

  function activateManagePreviewBoardPool(doc = document) {
    if (!shouldMountManagePreviewBoard(doc)) return;
    const sceneId = resolveManagePreviewSceneId(doc);
    const target = boardTargetFromUrl(new URL(window.location.href)) ||
      nonEmptyString(doc.querySelector("[data-target-file]")?.dataset?.targetFile);
    if (!sceneId || !target) return;
    const surface = resolveManagePreviewSurface(doc);
    if (!surface) return;
    const mountKey = boardMountPoolKey(doc, sceneId, target);
    if (surface.dataset.meiPreviewBoardMounted === mountKey) return;
    if (restoreBoardMount(mountKey, surface)) {
      dispatchPreviewUpdated("manage-board-preview");
    }
  }

  function readSceneDrilldownContext(doc = document) {
    const el = doc.getElementById("mei-scene-drilldown-context");
    if (!el) return null;
    try {
      const parsed = JSON.parse(el.textContent || "{}");
      return parsed && typeof parsed === "object" && !Array.isArray(parsed) ? parsed : null;
    } catch (_) {
      return null;
    }
  }

  function metricRefId(value) {
    if (!value || typeof value !== "object" || Array.isArray(value)) return "";
    if (value.__ref === "metric") return nonEmptyString(value.id);
    if (value.__ref === "metric_ref") {
      return nonEmptyString(
        value.__args?.arg0,
        value.__args?.[0],
        value.id,
        value.metric_id,
        value.metricId,
      );
    }
    return nonEmptyString(value.metric_id, value.metricId);
  }

  function pickExampleParams(assembly, context, sceneId) {
    const rawExamples =
      assembly?.examples ??
      (sceneId && context?.scene_examples_by_id
        ? context.scene_examples_by_id[sceneId]
        : null);
    const example = Array.isArray(rawExamples)
      ? rawExamples.find((entry) => entry && typeof entry === "object" && !Array.isArray(entry))
      : rawExamples && typeof rawExamples === "object"
        ? rawExamples
        : null;
    return example && typeof example === "object" ? normalizeSceneParams(example.params) : {};
  }

  function boardTargetFromUrl(url) {
    const fromFile = nonEmptyString(url.searchParams.get("file"));
    if (fromFile && /\.board\.mei$/i.test(fromFile)) return fromFile;
    const node = nonEmptyString(url.searchParams.get("node"));
    if (!/^board-(?:file|slot):/i.test(node)) return "";
    const payload = node.replace(/^board-(?:file|slot):/i, "");
    const hashAt = payload.indexOf("#");
    return hashAt >= 0 ? payload.slice(0, hashAt) : payload;
  }

  function sceneExportIdFromBoardNode(nodeParam) {
    const node = nonEmptyString(nodeParam);
    if (!/^board-(?:file|slot):/i.test(node)) return "";
    const payload = node.replace(/^board-(?:file|slot):/i, "");
    const slash = payload.indexOf("/");
    const boardKey = slash >= 0 ? payload.slice(0, slash) : payload;
    const hashAt = boardKey.indexOf("#");
    return hashAt >= 0 ? nonEmptyString(boardKey.slice(hashAt + 1)) : "";
  }

  function resolveManagePreviewSceneId(doc = document) {
    const url = new URL(window.location.href);
    const fromQuery = nonEmptyString(url.searchParams.get("scene"));
    if (fromQuery) return fromQuery;
    const fromNode = sceneExportIdFromBoardNode(url.searchParams.get("node"));
    if (fromNode) return fromNode;
    const anchor = doc.querySelector("[data-mei-frame-viewport][data-scene-id], [data-scene-id]");
    return nonEmptyString(anchor?.dataset?.sceneId);
  }

  function resolveManagePreviewSurface(doc = document) {
    const viewport = doc.querySelector("[data-mei-frame-viewport][data-scene-id]");
    const surface =
      (viewport instanceof HTMLElement
        ? viewport.querySelector(".preview-surface.preview-stage, .preview-surface")
        : null) ||
      doc.querySelector(".preview-surface[data-scene-id]") ||
      doc.querySelector(".preview-surface.preview-stage, .preview-surface");
    return surface instanceof HTMLElement ? surface : null;
  }

  function scopedManagePreviewPanelId(zone) {
    if (!zone || typeof zone !== "object" || Array.isArray(zone)) return "";
    const id = nonEmptyString(zone.id);
    if (!id) return "";
    const parent = nonEmptyString(zone.parent);
    return parent ? `${parent}/${id}` : id;
  }

  function managePreviewPanelIdCandidates(zoneOrPanelId, sceneShell = null) {
    const zone =
      zoneOrPanelId && typeof zoneOrPanelId === "object" && !Array.isArray(zoneOrPanelId)
        ? zoneOrPanelId
        : null;
    const candidates = [];
    if (zone) {
      const scoped = scopedManagePreviewPanelId(zone);
      if (scoped) candidates.push(scoped);
      const id = nonEmptyString(zone.id);
      if (id) candidates.push(id);
      return [...new Set(candidates)];
    }
    const panelId = nonEmptyString(zoneOrPanelId);
    if (!panelId) return [];
    const shellZone = (Array.isArray(sceneShell?.zones) ? sceneShell.zones : []).find(
      (entry) => nonEmptyString(entry?.id) === panelId,
    );
    const scoped = scopedManagePreviewPanelId(shellZone || { id: panelId });
    if (scoped) candidates.push(scoped);
    candidates.push(panelId);
    return [...new Set(candidates)];
  }

  function resolveManagePreviewPanelHost(surface, zoneOrPanelId, sceneShell = null) {
    if (!(surface instanceof HTMLElement)) return null;
    for (const panelId of managePreviewPanelIdCandidates(zoneOrPanelId, sceneShell)) {
      const panel = surface.querySelector(`[data-mei-panel-id="${panelId}"]`);
      if (!(panel instanceof HTMLElement)) continue;
      const body = panel.querySelector(
        "[data-mei-panel-body='true'], .preview-panel-body, .panel-body-cell",
      );
      return body instanceof HTMLElement ? body : panel;
    }
    return null;
  }

  function buildManagePreviewZoneHosts(surface, sceneShell) {
    const zoneHosts = {};
    (Array.isArray(sceneShell?.zones) ? sceneShell.zones : []).forEach((zone) => {
      const host = resolveManagePreviewPanelHost(surface, zone, sceneShell);
      if (host instanceof HTMLElement && nonEmptyString(zone?.id)) {
        zoneHosts[zone.id] = host;
      }
    });
    return zoneHosts;
  }

  function shouldMountManagePreviewBoard(doc = document) {
    if (!isBuildRoute()) return false;
    const url = new URL(window.location.href);
    const layoutProto =
      typeof isWorkspaceSurfaceRoute === "function" && isWorkspaceSurfaceRoute(url.pathname);
    if (!layoutProto && nonEmptyString(url.searchParams.get("tab"), "preview") !== "preview") {
      return false;
    }
    const viewport = doc.querySelector("[data-mei-frame-viewport][data-target-file], [data-target-file]");
    const surfaceTarget = nonEmptyString(viewport?.dataset?.targetFile);
    const target = boardTargetFromUrl(url) || surfaceTarget;
    if (!target || !/\.board\.mei$/i.test(target)) return false;
    return Boolean(resolveManagePreviewSceneId(doc) && resolveManagePreviewSurface(doc));
  }

  function buildManagePreviewDetail(context, sceneId) {
    const assembly = context?.scene_projection_assembly_by_id?.[sceneId];
    if (!assembly || typeof assembly !== "object" || Array.isArray(assembly)) return null;
    const params = normalizeSceneParams(assembly.preview_params || pickExampleParams(assembly, context, sceneId));
    const metricId = metricRefId(params.metric);
    if (!metricId) return null;
    const projectionSlots = normalizeProjectionSlots(assembly.projection_slots);
    if (!projectionSlots.length) return null;
    const filterSchema =
      assembly.filter_schema && typeof assembly.filter_schema === "object" && !Array.isArray(assembly.filter_schema)
        ? assembly.filter_schema
        : null;
    const hostSceneFile = nonEmptyString(assembly.target_file);
    return {
      board_scene_id: sceneId,
      board_scene_file: hostSceneFile,
      scene_id: sceneId,
      host_scene_id: sceneId,
      host_scene_file: hostSceneFile,
      metric_id: metricId,
      scene_projection_assembly_by_id: context.scene_projection_assembly_by_id,
      scene_bindings_by_id: context.scene_bindings_by_id,
      scene_examples_by_id: context.scene_examples_by_id,
      popup: {
        mode: "board_link",
        scene_id: sceneId,
        scene_file: nonEmptyString(assembly.target_file),
        projection_slots: projectionSlots,
        filter_schema: filterSchema,
        params,
        local_nav: assembly.local_nav || assembly.localNav,
      },
    };
  }

  function repairManagePreviewBoardGrid(surface, sceneShell) {
    if (!(surface instanceof HTMLElement)) return;
    const layout = sceneShell?.layout;
    const rawRows = Array.isArray(layout?.rows) ? layout.rows : [];
    const rows =
      rawRows.length >= 2
        ? rawRows.map((row, index) => {
            const normalized = String(row || "").trim().toLowerCase();
            if (index === 0 && (normalized === "auto" || normalized === "max-content")) {
              return "minmax(240px, auto)";
            }
            return String(row || "auto");
          })
        : ["minmax(240px, auto)", "minmax(0, 1fr)"];
    surface.style.gridTemplateRows = rows.join(" ");
    const chartZone =
      (Array.isArray(sceneShell?.zones) ? sceneShell.zones : []).find(
        (zone) => nonEmptyString(zone?.role) === "slots" && zone?.accepts?.includes?.("chart"),
      ) || { id: "chart" };
    const chartHost = resolveManagePreviewPanelHost(surface, chartZone, sceneShell);
    if (chartHost instanceof HTMLElement) {
      chartHost.style.display = "grid";
      chartHost.style.minHeight = "220px";
      chartHost.style.height = "100%";
    }
  }

  function refreshManagePreviewBoardCharts(surface) {
    if (!(surface instanceof HTMLElement)) return;
    surface.querySelectorAll("mei-chart-column, mei-chart-rose, mei-chart-bar, mei-chart-pie").forEach((node) => {
      if (node && typeof node.refresh === "function") {
        node.refresh();
      }
    });
  }

  function showManagePreviewBoardError(surface, message, detail = null) {
    if (!(surface instanceof HTMLElement)) return;
    let banner = surface.querySelector("[data-manage-preview-t2-page-error]");
    if (!(banner instanceof HTMLElement)) {
      banner = document.createElement("div");
      banner.className = "access-drilldown-overlay-status manage-preview-t2-page-error";
      banner.dataset.managePreviewBoardError = "true";
      banner.style.margin = "12px";
      surface.prepend(banner);
    }
    banner.hidden = false;
    const userMessage = String(message || "看板预览加载失败，请稍后重试。");
    let traceId = "";
    if (typeof recordPopupDebugIssue === "function") {
      traceId = String(
        recordPopupDebugIssue({
          level: "error",
          phase: "manage_preview_board_mount",
          message: userMessage,
          detail: detail || {},
          config: detail?.popup || {},
          metricId: nonEmptyString(detail?.metric_id),
          root: surface,
        }) || "",
      ).trim();
    }
    banner.textContent = traceId
      ? `${userMessage}（追踪编号：${traceId}）`
      : userMessage;
  }

  async function mountManagePreviewBoard(doc = document) {
    if (!shouldMountManagePreviewBoard(doc)) return false;
    const sceneId = resolveManagePreviewSceneId(doc);
    const context = readSceneDrilldownContext(doc);
    const detail = context ? buildManagePreviewDetail(context, sceneId) : null;
    const surface = resolveManagePreviewSurface(doc);
    if (!detail) {
      if (surface instanceof HTMLElement) {
        showManagePreviewBoardError(
          surface,
          "看板预览配置不完整：缺少 projection_slots 或示例 metric 参数。",
          { scene_id: sceneId, board_scene_id: sceneId },
        );
      }
      return false;
    }

    const resolved = resolveSceneOpenRequest(detail);
    if (!resolved.enabled || !resolved.structuredBoard || !resolved.sceneShell) {
      if (surface instanceof HTMLElement) {
        showManagePreviewBoardError(surface, "看板预览无法解析结构化壳配置。", detail);
      }
      return false;
    }
    if (!nonEmptyString(resolved.queryStateId)) {
      resolved.queryStateId = `manage-preview::${sceneId}`;
    }
    if (!nonEmptyString(resolved.hostSceneId, resolved.sceneId)) {
      resolved.hostSceneId = sceneId;
      resolved.sceneId = sceneId;
    }
    if (!nonEmptyString(resolved.hostSceneFile)) {
      resolved.hostSceneFile = nonEmptyString(detail.host_scene_file, detail.board_scene_file);
    }

    if (!surface) return false;
    const previewTargetFile =
      boardTargetFromUrl(new URL(window.location.href)) ||
      nonEmptyString(
        doc.querySelector("[data-mei-frame-viewport]")?.dataset?.targetFile,
        surface.dataset.targetFile,
        surface.dataset.sourcePath,
      );
    const mountKey = `${sceneId}::${previewTargetFile}`;
    if (surface.dataset.meiPreviewBoardMounted === mountKey) {
      return true;
    }
    if (restoreBoardMount(mountKey, surface)) {
      return true;
    }
    delete surface.dataset.meiPreviewBoardMounted;
    surface.classList.remove("preview-board-mounted");

    const zoneHosts = buildManagePreviewZoneHosts(surface, resolved.sceneShell);
    const filterZone = sceneShellZonesByRole(resolved.sceneShell, "filter")[0] || null;
    const slotZones = sceneShellZonesByRole(resolved.sceneShell, "slots");
    const projectionSlots = Array.isArray(resolved?.popup?.projection_slots)
      ? resolved.popup.projection_slots
      : Array.isArray(resolved?.projectionSlots)
        ? resolved.projectionSlots
        : [];
    const expectsSlotZones = projectionSlots.some(
      (slot) =>
        String(slot?.component || "").trim() === "chart" ||
        String(slot?.component || "").trim() === "data_table",
    );
    if (expectsSlotZones && !slotZones.length) {
      if (surface instanceof HTMLElement) {
        showManagePreviewBoardError(
          surface,
          "看板预览 shell 缺少 chart/detail 挂载区（嵌套 zone 解析不完整）。",
          detail,
        );
      }
      return false;
    }
    const hostsReady =
      (!filterZone || zoneHosts[filterZone.id] instanceof HTMLElement) &&
      slotZones.every((zone) => zoneHosts[zone.id] instanceof HTMLElement);
    if (!hostsReady) {
      if (surface instanceof HTMLElement) {
        showManagePreviewBoardError(surface, "看板预览区缺少 filter/chart/detail 挂载点。", detail);
      }
      return false;
    }

    if (filterZone) {
      const host = zoneHosts[filterZone.id];
      if (host instanceof HTMLElement) {
        host.dataset.buildBoardSlot = "filter";
        host.replaceChildren();
        await mountAnalyticsFilterBar(surface, detail, resolved, host);
      }
    }

    let mountOk = true;
    for (const zone of slotZones) {
      const host = zoneHosts[zone.id];
      if (!(host instanceof HTMLElement)) continue;
      host.replaceChildren();
      const zoneSlots = Array.isArray(resolved?.slotsByZone?.[zone.id]) ? resolved.slotsByZone[zone.id] : [];
      if (!zoneSlots.length) continue;
      if (zoneSlots.every((slot) => slot.component === "chart")) {
        zoneSlots.forEach((slot, index) => {
          const slotEl = document.createElement("div");
          slotEl.className = "access-drilldown-shell-slot access-drilldown-shell-slot--chart";
          slotEl.dataset.chartSlotIndex = String(index);
          if (slot?.id) slotEl.dataset.buildBoardSlot = String(slot.id);
          slotEl.style.height = "100%";
          slotEl.style.minHeight = "180px";
          host.appendChild(slotEl);
        });
        host.style.display = "grid";
        host.style.gridTemplateColumns =
          zoneSlots.length > 1 ? `repeat(${zoneSlots.length}, minmax(0, 1fr))` : "1fr";
        host.style.gridTemplateRows = "1fr";
        host.style.height = "100%";
        host.style.minHeight = "220px";
        const chartsOk = await mountAnalyticsChartSlots(surface, detail, resolved, zoneSlots, host);
        mountOk = mountOk && chartsOk;
        continue;
      }
      host.style.minHeight = "240px";
      const zoneOk = await mountStructuredSlotZone(surface, detail, resolved, zone, host);
      mountOk = mountOk && zoneOk;
    }

    if (!mountOk) {
      delete surface.dataset.meiPreviewBoardMounted;
      surface.classList.remove("preview-board-mounted");
      showManagePreviewBoardError(surface, "看板图表或明细区挂载失败，请查看控制台 [mei][popup-panel] 日志。", detail);
      return false;
    }

    if (resolved.hasRowPreviewZone) {
      mountStructuredRowPreviewZone(surface, zoneHosts, resolved);
    }

    surface.querySelector("[data-manage-preview-t2-page-error]")?.remove();

    repairManagePreviewBoardGrid(surface, resolved.sceneShell);
    refreshManagePreviewBoardCharts(surface);

    bindAnalyticsChartsQueryStateRefresh(surface, detail, resolved, (zoneId) => zoneHosts?.[zoneId]);

    surface.dataset.meiPreviewBoardMounted = mountKey;
    surface.classList.add("preview-board-mounted");
    stashBoardMount(mountKey, surface);
    dispatchPreviewUpdated("manage-board-preview");
    if (global.MeiBuildInspectHighlight && typeof global.MeiBuildInspectHighlight.refresh === "function") {
      global.MeiBuildInspectHighlight.refresh({ detail: { scope: "manage-board-preview" } });
    }
    return true;
  }

  function scheduleManagePreviewBoardMount(doc = document) {
    requestAnimationFrame(() => {
      requestAnimationFrame(() => {
        void mountManagePreviewBoard(doc);
      });
    });
  }

  function installManagePreviewBoard() {
    if (boot.managePreviewBoardInstalled) return;
    boot.managePreviewBoardInstalled = true;
    boot.mountManagePreviewBoard = mountManagePreviewBoard;
    boot.activateManagePreviewBoardPool = activateManagePreviewBoardPool;
    window.addEventListener(MANAGE_PREVIEW_UPDATED_EVENT, (event) => {
      const scope = String(event?.detail?.scope || "").trim();
      if (scope === "manage-board-preview") return;
      scheduleManagePreviewBoardMount(document);
    });
    window.addEventListener("popstate", () => {
      scheduleManagePreviewBoardMount(document);
    });
    document.addEventListener("mei:manage-tab-change", (event) => {
      if (String(event?.detail?.tab || "").trim().toLowerCase() === "preview") {
        scheduleManagePreviewBoardMount(document);
      }
    });
    if (isBuildRoute()) {
      scheduleManagePreviewBoardMount(document);
    }
  }

  installManagePreviewBoard();
