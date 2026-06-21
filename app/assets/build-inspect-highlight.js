/**
 * Build view: preview inspect — highlight, click-to-select node/focus, inspect bar, suppress drilldown.
 */
(function (global) {
  "use strict";

  const PANEL_SELECTOR = "[data-build-node^='scene-panel:']";
  const BLOCK_SELECTOR = "[data-build-focus^='scene-block:']";
  const SELECTOR = `${PANEL_SELECTOR}, ${BLOCK_SELECTOR}`;

  function isBuildRoute() {
    return /^\/apps\/(?:build|manage)\//.test(String(global.location.pathname || ""));
  }

  function activeShell() {
    return document.querySelector(".shell[data-build-node]");
  }

  function activeBuildNode() {
    try {
      const fromUrl = String(new URL(global.location.href).searchParams.get("node") || "").trim();
      if (fromUrl) return fromUrl;
    } catch (_) {}
    return String(activeShell()?.getAttribute("data-build-node") || "").trim();
  }

  function activeBuildFocus() {
    try {
      const fromUrl = String(new URL(global.location.href).searchParams.get("focus") || "").trim();
      if (fromUrl) return fromUrl;
    } catch (_) {}
    return String(activeShell()?.getAttribute("data-build-focus") || "").trim();
  }

  function previewRoot() {
    return (
      document.querySelector("[data-manage-tab-panel='preview']") ||
      document.querySelector(".preview-surface") ||
      document.querySelector(".preview-pane-scroll")
    );
  }

  function inspectBarLabel() {
    return document.getElementById("build-inspect-bar-label");
  }

  function clearHighlights(root) {
    root.querySelectorAll(".build-inspect-selected, .build-inspect-focus-selected").forEach((el) => {
      el.classList.remove("build-inspect-selected", "build-inspect-focus-selected");
    });
  }

  function updateInspectBar(node, focus, el, message) {
    const bar = inspectBarLabel();
    if (!bar) return;
    if (message) {
      bar.textContent = message;
      return;
    }
    if (!node && !focus) {
      bar.textContent = "在左侧体验树选择 Panel/Block，或在预览中点击组件以指认上下文。";
      return;
    }
    const blockId = el?.getAttribute("data-mei-block-id") || "";
    const useKey = el?.getAttribute("data-mei-use-key") || "";
    const panelId = el?.getAttribute("data-mei-panel-id") || "";
    const bits = [];
    if (node) bits.push(`node=${node}`);
    if (focus) bits.push(`focus=${focus}`);
    if (panelId) bits.push(`panel=${panelId}`);
    if (blockId) bits.push(`block=${blockId}`);
    if (useKey) bits.push(`use=${useKey}`);
    bar.textContent = bits.join(" · ");
  }

  function syncBuildPreviewScopedChrome(root) {
    const scopedActive =
      root instanceof HTMLElement &&
      (root.querySelector("[data-preview-scope].build-preview-scoped-dim") != null ||
        root.querySelector("[data-mei-panel-id].build-preview-scoped-dim") != null ||
        root.querySelector("[data-chart-slot-index].build-preview-scoped-dim") != null ||
        root.querySelector("[data-build-board-slot].build-preview-scoped-dim") != null);
    document.body.classList.toggle("build-preview-scoped-active", scopedActive);
  }

  function templateKeyFromNode(node) {
    const raw = String(node || "").trim();
    if (!raw.startsWith("template:")) return "";
    return raw.replace(/^template:/i, "");
  }

  function isTemplateAuthoringPreview() {
    const node = activeBuildNode();
    if (!node.startsWith("template:")) return false;
    const target = String(activeShell()?.getAttribute("data-compile-target") || "")
      .trim()
      .toLowerCase();
    if (!target) return false;
    return (
      target.includes("templates/") ||
      target.includes(".stock/templates") ||
      target.includes("/authoring/examples/")
    );
  }

  function normalizeTemplateFileKey(value) {
    let raw = String(value || "").trim().replace(/\\/g, "/");
    while (raw.startsWith("./")) raw = raw.slice(2);
    while (raw.startsWith("/")) raw = raw.slice(1);
    if (raw.startsWith(".stock/templates/")) raw = raw.slice(".stock/templates/".length);
    else if (raw.startsWith("templates/")) raw = raw.slice("templates/".length);
    return raw;
  }

  function uniqueStrings(values) {
    const out = [];
    const seen = new Set();
    for (const value of values || []) {
      const item = String(value || "").trim();
      if (!item || seen.has(item)) continue;
      seen.add(item);
      out.push(item);
    }
    return out;
  }

  function templateUseKeysFromTree(templateNode) {
    const key = templateKeyFromNode(templateNode);
    if (!key) return [];
    const useKeys = [];
    if (!key.includes("/") && !/\.mei$/i.test(key)) {
      useKeys.push(key);
    }
    const normalizedFile = normalizeTemplateFileKey(key);
    if (!normalizedFile || (!key.includes("/") && !/\.mei$/i.test(key))) {
      return uniqueStrings(useKeys);
    }
    const roots = readReachabilityTreeRoots();
    const walk = (nodes) => {
      for (const node of nodes || []) {
        const kind = String(node?.kind || "").trim();
        const nodeId = String(node?.node_id || "").trim();
        if (kind === "template" && nodeId.startsWith("template:")) {
          const useKey = nodeId.slice("template:".length).trim();
          const badges = Array.isArray(node?.badges) ? node.badges : [];
          const matched = badges.some(
            (badge) => normalizeTemplateFileKey(badge) === normalizedFile,
          );
          if (matched && useKey) {
            useKeys.push(useKey);
          }
        }
        walk(node?.children);
      }
    };
    for (const root of roots) {
      walk(root?.children);
    }
    return uniqueStrings(useKeys);
  }

  function applyTemplateScopedPreview(root, useKeys) {
    if (!Array.isArray(useKeys) || useKeys.length === 0) return;
    const keySet = new Set(useKeys);
    root.querySelectorAll("[data-mei-use-key]").forEach((el) => {
      const key = String(el.getAttribute("data-mei-use-key") || "").trim();
      if (key && !keySet.has(key)) {
        el.classList.add("build-preview-scoped-dim");
      }
    });
    syncBuildPreviewScopedChrome(root);
  }

  function boardSlotIdFromNode(node) {
    const raw = String(node || "").trim();
    if (!raw.startsWith("board-slot:")) return "";
    const payload = raw.replace(/^board-slot:/i, "");
    const slash = payload.lastIndexOf("/");
    return slash >= 0 ? payload.slice(slash + 1) : "";
  }

  function readReachabilityTreeRoots() {
    const el = document.getElementById("mei-build-reachability-tree");
    if (!el) return [];
    try {
      const parsed = JSON.parse(el.textContent || "[]");
      return Array.isArray(parsed) ? parsed : [];
    } catch (_) {
      return [];
    }
  }

  function findReachabilityNodeEntry(nodeId) {
    const target = String(nodeId || "").trim();
    if (!target) return null;
    let found = null;
    const walk = (nodes, parent) => {
      for (const node of nodes || []) {
        if (String(node?.node_id || "").trim() === target) {
          found = { node, parent };
          return true;
        }
        if (walk(node?.children, node)) return true;
      }
      return false;
    };
    for (const root of readReachabilityTreeRoots()) {
      if (walk(root?.children, root)) break;
    }
    return found;
  }

  function boardSlotMetaFromReachabilityTree(node) {
    const entry = findReachabilityNodeEntry(node);
    if (!entry) return null;
    const treeNode = entry.node;
    const parent = entry.parent;
    const layoutZone = String(treeNode?.board_layout_zone || "").trim();
    const component = String(treeNode?.badges?.[0] || "").trim();
    let chartIndex = -1;
    if (component === "chart" && layoutZone && parent && Array.isArray(parent.children)) {
      const chartSiblings = parent.children.filter((child) => {
        return (
          String(child?.board_layout_zone || "").trim() === layoutZone &&
          String(child?.badges?.[0] || "").trim() === "chart"
        );
      });
      chartIndex = chartSiblings.findIndex(
        (child) => String(child?.node_id || "").trim() === String(node || "").trim(),
      );
    }
    return { layoutZone, component, chartIndex };
  }

  function readSceneDrilldownContext() {
    const el = document.getElementById("mei-scene-drilldown-context");
    if (!el) return null;
    try {
      const parsed = JSON.parse(el.textContent || "{}");
      return parsed && typeof parsed === "object" && !Array.isArray(parsed) ? parsed : null;
    } catch (_) {
      return null;
    }
  }

  function boardLayoutZoneFromTreeLink(node) {
    const raw = String(node || "").trim();
    if (!raw) return "";
    const link = document.querySelector(
      `.build-reachability-tree a[data-build-node="${CSS.escape(raw)}"]`,
    );
    return String(link?.getAttribute("data-board-layout-zone") || "").trim();
  }

  function projectionSlotsForBoardScene(sceneId) {
    const ctx = readSceneDrilldownContext();
    const assembly = ctx?.scene_projection_assembly_by_id?.[sceneId];
    const raw = assembly?.projection_slots;
    return Array.isArray(raw) ? raw : [];
  }

  function resolveBoardSlotMeta(node, slotId) {
    const fromTree = boardSlotMetaFromReachabilityTree(node);
    const layoutZoneFromTree =
      String(fromTree?.layoutZone || "").trim() ||
      boardLayoutZoneFromTreeLink(node);
    const sceneId = sceneExportIdFromBoardNode(node);
    const slots = projectionSlotsForBoardScene(sceneId);
    const matched = slots.find((entry) => {
      const id = String(entry?.id || entry?.slot_id || "").trim();
      return id && id === slotId;
    });
    const component = String(
      fromTree?.component ||
        matched?.component ||
        matched?.as ||
        (slotId === "filter" ? "filter" : ""),
    ).trim();
    const layoutZone =
      layoutZoneFromTree ||
      String(matched?.layout_zone || matched?.layoutZone || "").trim() ||
      (slotId === "filter" ? "filter" : slotId);
    const chartIndex =
      typeof fromTree?.chartIndex === "number" && fromTree.chartIndex >= 0
        ? fromTree.chartIndex
        : chartSlotIndexForMeta({ slotId, layoutZone, component, sceneId });
    return { slotId, layoutZone, component, sceneId, chartIndex };
  }

  function chartSlotIndexForMeta(meta) {
    if (typeof meta?.chartIndex === "number" && meta.chartIndex >= 0) {
      return meta.chartIndex;
    }
    if (meta.component !== "chart") return -1;
    const slots = projectionSlotsForBoardScene(meta.sceneId).filter((entry) => {
      const zone = String(entry?.layout_zone || entry?.layoutZone || "").trim();
      const component = String(entry?.component || entry?.as || "").trim();
      return zone === meta.layoutZone && component === "chart";
    });
    return slots.findIndex((entry) => {
      const id = String(entry?.id || entry?.slot_id || "").trim();
      return id === meta.slotId;
    });
  }

  function resolveBoardSlotHighlightTargets(root, meta) {
    if (!meta?.slotId) return [];
    const tagged = root.querySelector(`[data-build-board-slot="${CSS.escape(meta.slotId)}"]`);
    if (tagged instanceof HTMLElement) return [tagged];

    const panel = root.querySelector(
      `[data-mei-panel-id="${CSS.escape(meta.layoutZone)}"]`,
    );
    if (!(panel instanceof HTMLElement)) return [];

    const isChart =
      meta.component === "chart" ||
      (!meta.component && meta.layoutZone === "chart");
    if (isChart) {
      const chartIndex = chartSlotIndexForMeta(meta);
      if (chartIndex >= 0) {
        const chartSlot = panel.querySelector(
          `[data-chart-slot-index="${chartIndex}"]`,
        );
        if (chartSlot instanceof HTMLElement) return [chartSlot];
      }
      const chartSlots = panel.querySelectorAll("[data-chart-slot-index]");
      if (chartSlots.length === 1) return [chartSlots[0]];
    }
    const panelBody = panel.querySelector(
      "[data-mei-panel-body='true'], .preview-panel-body, .panel-body-cell",
    );
    return [panelBody instanceof HTMLElement ? panelBody : panel];
  }

  const BOARD_SLOT_COMPANION_ZONES = new Set(["filter"]);

  function applyBoardSlotScopedPreview(root, meta) {
    if (!meta?.layoutZone) return;
    const keepZones = new Set([meta.layoutZone]);
    BOARD_SLOT_COMPANION_ZONES.forEach((zoneId) => keepZones.add(zoneId));

    root.querySelectorAll("[data-mei-panel-id]").forEach((el) => {
      const panelId = String(el.getAttribute("data-mei-panel-id") || "").trim();
      if (panelId && !keepZones.has(panelId)) {
        el.classList.add("build-preview-scoped-dim");
      }
    });

    if (meta.component === "chart" || meta.layoutZone === "chart") {
      const panel = root.querySelector(
        `[data-mei-panel-id="${CSS.escape(meta.layoutZone)}"]`,
      );
      const chartIndex = chartSlotIndexForMeta(meta);
      if (panel instanceof HTMLElement && chartIndex >= 0) {
        panel.querySelectorAll("[data-chart-slot-index]").forEach((el) => {
          if (String(el.getAttribute("data-chart-slot-index") || "") !== String(chartIndex)) {
            el.classList.add("build-preview-scoped-dim");
          }
        });
      }
    }

    syncBuildPreviewScopedChrome(root);
  }

  function applyScopedPreview(root) {
    const node = activeBuildNode();
    root.querySelectorAll("[data-preview-scope], [data-mei-panel-id], [data-chart-slot-index], [data-build-board-slot], [data-mei-use-key]").forEach((el) => {
      el.classList.remove("build-preview-scoped-dim");
    });
    const boardSlot = boardSlotIdFromNode(node);
    if (boardSlot) {
      applyBoardSlotScopedPreview(root, resolveBoardSlotMeta(node, boardSlot));
      return;
    }
    if (node.startsWith("board-file:")) {
      syncBuildPreviewScopedChrome(root);
      return;
    }
    const templateUseKeys = templateUseKeysFromTree(node);
    if (templateUseKeys.length > 0 && !isTemplateAuthoringPreview()) {
      applyTemplateScopedPreview(root, templateUseKeys);
      return;
    }
    if (templateKeyFromNode(node) && isTemplateAuthoringPreview()) {
      syncBuildPreviewScopedChrome(root);
      return;
    }
    if (
      !node.startsWith("scene-panel:") &&
      !node.startsWith("scene-block:")
    ) {
      syncBuildPreviewScopedChrome(root);
      return;
    }
    const encoded = node.replace(/^scene-panel:/, "").replace(/^scene-block:/, "");
    const slash = encoded.indexOf("/");
    const scopePath = slash >= 0 ? encoded.slice(slash + 1) : "";
    if (!scopePath) return;
    root.querySelectorAll("[data-preview-scope]").forEach((el) => {
      const elScope = String(el.getAttribute("data-preview-scope") || "");
      if (elScope === scopePath || elScope.startsWith(`${scopePath}/`)) {
        return;
      }
      if (scopePath.startsWith(`${elScope}/`)) {
        return;
      }
      el.classList.add("build-preview-scoped-dim");
    });
    syncBuildPreviewScopedChrome(root);
  }

  function scrollIntoViewIfOne(matches, scrollRoot) {
    if (matches.length !== 1) return;
    const el = matches[0];
    const container =
      scrollRoot instanceof HTMLElement
        ? scrollRoot
        : el?.closest?.(".preview-pane-scroll");
    if (!(container instanceof HTMLElement) || !(el instanceof HTMLElement)) return;
    const elRect = el.getBoundingClientRect();
    const boxRect = container.getBoundingClientRect();
    const margin = 8;
    if (elRect.top < boxRect.top + margin) {
      container.scrollTop -= boxRect.top + margin - elRect.top;
    } else if (elRect.bottom > boxRect.bottom - margin) {
      container.scrollTop += elRect.bottom - boxRect.bottom + margin;
    }
  }

  function applyHighlight(root) {
    const node = activeBuildNode();
    const focus = activeBuildFocus();
    clearHighlights(root);
    applyScopedPreview(root);

    let focusEl = null;
    if (focus && focus.startsWith("scene-block:")) {
      const focusMatches = root.querySelectorAll(`[data-build-focus="${CSS.escape(focus)}"]`);
      focusMatches.forEach((el) => el.classList.add("build-inspect-focus-selected"));
      scrollIntoViewIfOne(focusMatches, root);
      focusEl = focusMatches[0] || null;
    }

    if (node && (node.startsWith("scene-panel:") || node.startsWith("scene-block:"))) {
      const matches = root.querySelectorAll(`[data-build-node="${CSS.escape(node)}"]`);
      let selected = Array.from(matches);
      if (focus && focus.startsWith("scene-block:")) {
        const focusScoped = selected.filter(
          (el) => String(el.getAttribute("data-build-focus") || "").trim() === focus,
        );
        if (focusScoped.length > 0) {
          selected = focusScoped;
        }
      }
      if (selected.length > 1) {
        selected = [selected[0]];
      }
      selected.forEach((el) => el.classList.add("build-inspect-selected"));
      if (!focusEl) {
        scrollIntoViewIfOne(selected, root);
      }
      updateInspectBar(node, focus, focusEl || selected[0] || null);
      return;
    }

    if (node.startsWith("board-slot:")) {
      const slotId = boardSlotIdFromNode(node);
      const meta = resolveBoardSlotMeta(node, slotId);
      const selected = resolveBoardSlotHighlightTargets(root, meta);
      selected.forEach((el) => el.classList.add("build-inspect-selected"));
      scrollIntoViewIfOne(selected, root);
      updateInspectBar(node, focus, selected[0] || null);
      return;
    }

    if (node.startsWith("board-file:")) {
      updateInspectBar(node, focus, null);
      return;
    }

    if (node.startsWith("template:")) {
      if (isTemplateAuthoringPreview()) {
        updateInspectBar(
          node,
          focus,
          null,
          "模板独立预览：展示内置示例场景与 props，非应用内使用处高亮。",
        );
        return;
      }
      const selected = [];
      for (const useKey of templateUseKeysFromTree(node)) {
        if (!useKey) continue;
        root
          .querySelectorAll(`[data-mei-use-key="${CSS.escape(useKey)}"]`)
          .forEach((el) => selected.push(el));
      }
      const uniqueSelected = Array.from(new Set(selected));
      uniqueSelected.forEach((el) => el.classList.add("build-inspect-selected"));
      scrollIntoViewIfOne(uniqueSelected, root);
      updateInspectBar(node, focus, uniqueSelected[0] || null);
      return;
    }

    updateInspectBar(node, focus, focusEl);
  }

  function currentManageTab() {
    try {
      return String(new URL(global.location.href).searchParams.get("tab") || "").trim().toLowerCase();
    } catch (_) {
      return "";
    }
  }

  function pushBuildUrl(mutator) {
    if (!isBuildRoute()) return;
    const shell = activeShell();
    const appPath = shell?.getAttribute("data-app-path") || "";
    if (!appPath) return;
    const url = new URL(global.location.href);
    mutator(url);
    const tab = currentManageTab() || String(shell?.getAttribute("data-build-tab") || "").trim().toLowerCase();
    if (tab) {
      url.searchParams.set("tab", tab);
    } else if (url.searchParams.get("tab") === "" || !url.searchParams.get("tab")) {
      url.searchParams.set("tab", "preview");
    }
    if (url.href === global.location.href) {
      applyHighlight(previewRoot() || document);
      return;
    }
    global.history.pushState({}, "", url.href);
    global.dispatchEvent(new PopStateEvent("popstate"));
  }

  function readFocusFromUrl() {
    try {
      return String(new URL(global.location.href).searchParams.get("focus") || "").trim();
    } catch (_) {
      return "";
    }
  }

  function syncShellFocus(focus) {
    const shell = activeShell();
    if (!shell) return;
    shell.setAttribute("data-build-focus", focus || "");
  }

  function navigateToBuildNode(node) {
    if (!node) return;
    pushBuildUrl((url) => {
      url.searchParams.set("node", node);
      url.searchParams.delete("focus");
    });
    syncShellFocus("");
  }

  function navigateToBuildFocus(focus) {
    if (!focus) return;
    pushBuildUrl((url) => {
      url.searchParams.set("focus", focus);
      url.searchParams.set("tab", "preview");
    });
    syncShellFocus(focus);
  }

  function bindPreviewInspect(root) {
    if (!root || root.__buildInspectBound) return;
    root.__buildInspectBound = true;

    root.addEventListener(
      "click",
      (event) => {
        if (!isBuildRoute()) return;
        if (event.target.closest("[data-preview-zoom-bar]")) {
          return;
        }
        const blockTarget = event.target.closest(BLOCK_SELECTOR);
        if (blockTarget) {
          const focus = String(blockTarget.getAttribute("data-build-focus") || "").trim();
          if (focus) {
            event.preventDefault();
            event.stopPropagation();
            navigateToBuildFocus(focus);
          }
          return;
        }
        const panelTarget = event.target.closest(PANEL_SELECTOR);
        if (panelTarget) {
          const node = String(panelTarget.getAttribute("data-build-node") || "").trim();
          if (node) {
            event.preventDefault();
            event.stopPropagation();
            navigateToBuildNode(node);
          }
          return;
        }
        if (
          event.target.closest("a[href^='/apps/']") ||
          event.target.closest("[data-popup]") ||
          event.target.closest(".metric-card")?.closest("[role='button']")
        ) {
          event.preventDefault();
          event.stopPropagation();
        }
      },
      true,
    );
  }

  function boardMountKeyForSurface(surface) {
    if (!(surface instanceof HTMLElement)) return "";
    try {
      const url = new URL(global.location.href);
      const fromNode = String(url.searchParams.get("node") || "");
      const sceneId =
        sceneExportIdFromBoardNode(fromNode) ||
        String(surface.dataset.sceneId || "").trim() ||
        String(url.searchParams.get("scene") || "").trim();
      const target = String(
        surface.dataset.targetFile || surface.dataset.sourcePath || "",
      ).trim();
      return sceneId && target ? `${sceneId}::${target}` : "";
    } catch (_) {
      return "";
    }
  }

  function sceneExportIdFromBoardNode(nodeParam) {
    const node = String(nodeParam || "").trim();
    if (!/^board-(?:file|slot):/i.test(node)) return "";
    const payload = node.replace(/^board-(?:file|slot):/i, "");
    const slash = payload.indexOf("/");
    const boardKey = slash >= 0 ? payload.slice(0, slash) : payload;
    const hashAt = boardKey.indexOf("#");
    return hashAt >= 0 ? boardKey.slice(hashAt + 1).trim() : "";
  }

  function clearScopedPreviewDim(root) {
    if (!(root instanceof HTMLElement)) return;
    root.querySelectorAll(
      "[data-preview-scope].build-preview-scoped-dim, [data-mei-panel-id].build-preview-scoped-dim, [data-chart-slot-index].build-preview-scoped-dim, [data-build-board-slot].build-preview-scoped-dim",
    ).forEach((el) => {
      el.classList.remove("build-preview-scoped-dim");
    });
    document.body.classList.remove("build-preview-scoped-active");
  }

  function clearStaleBoardMount(root) {
    if (!(root instanceof HTMLElement)) return false;
    let cleared = false;
    root.querySelectorAll(".preview-surface, .preview-stage").forEach((surface) => {
      if (!(surface instanceof HTMLElement)) return;
      const mounted = surface.dataset.meiPreviewBoardMounted;
      if (!mounted) return;
      const expected = boardMountKeyForSurface(surface);
      if (expected && mounted === expected) return;
      delete surface.dataset.meiPreviewBoardMounted;
      surface.classList.remove("preview-board-mounted");
      cleared = true;
    });
    return cleared;
  }
  function refresh(event) {
    if (!isBuildRoute()) return;
    document.body.classList.remove("access-drilldown-open", "access-scene-board-open");
    const root = previewRoot();
    if (!root) return;
    clearScopedPreviewDim(root);
    const scope = String(event?.detail?.scope || "").trim();
    if (scope !== "manage-board-preview") {
      clearStaleBoardMount(root);
    }
    syncShellFocus(readFocusFromUrl());
    bindPreviewInspect(root);
    applyHighlight(root);
  }

  function bind() {
    if (!isBuildRoute()) return;
    refresh(null);
    global.addEventListener("mei:manage-tab-change", refresh);
    global.addEventListener("meilang:preview-updated", refresh);
    global.addEventListener("popstate", refresh);
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", bind);
  } else {
    bind();
  }

  global.MeiBuildInspectHighlight = { refresh, navigateToBuildNode, navigateToBuildFocus };
})(window);
