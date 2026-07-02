/**
 * Build view: preview inspect — highlight, click-to-select node/focus, inspect bar, suppress drilldown.
 */
(function (global) {
  "use strict";

  const PANEL_SELECTOR = "[data-build-node^='scene-panel:'], [data-build-node^='ui-scope:']";
  const BLOCK_SELECTOR = "[data-build-focus^='scene-block:']";
  const UI_SCOPE_SELECTOR = "[data-mei-ui-scope]";
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

  function catalogKeyFromNode(node) {
    const raw = String(node || "").trim();
    if (raw.startsWith("component:")) return raw.slice("component:".length).trim();
    if (raw.startsWith("template:")) return raw.slice("template:".length).trim();
    return "";
  }

  function templateKeyFromNode(node) {
    return catalogKeyFromNode(node);
  }

  function isTemplateAuthoringPreview() {
    const node = activeBuildNode();
    if (node.startsWith("component:")) {
      const target = String(activeShell()?.getAttribute("data-compile-target") || "")
        .trim()
        .toLowerCase();
      return target.includes("/previews/") || target.includes("stock/components/");
    }
    if (!node.startsWith("template:")) return false;
    const target = String(activeShell()?.getAttribute("data-compile-target") || "")
      .trim()
      .toLowerCase();
    if (!target) return false;
    return (
      target.includes("templates/") ||
      target.includes(".stock/templates") ||
      target.includes("stock/templates") ||
      target.includes("/previews/")
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
        if (
          (kind === "component" && nodeId.startsWith("component:")) ||
          (kind === "template" && nodeId.startsWith("template:"))
        ) {
          const useKey = nodeId.includes(":")
            ? nodeId.slice(nodeId.indexOf(":") + 1).trim()
            : "";
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
    let anyMatch = false;
    root.querySelectorAll("[data-mei-use-key]").forEach((el) => {
      const key = String(el.getAttribute("data-mei-use-key") || "").trim();
      if (key && keySet.has(key)) {
        anyMatch = true;
      }
    });
    if (!anyMatch) {
      syncBuildPreviewScopedChrome(root);
      return;
    }
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
