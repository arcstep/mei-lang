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
    const path = String(global.location.pathname || "");
    return (
      /^\/apps\/[^/]+\/(?:layout|prototype)(?:\/|$)/.test(path) ||
      /^\/apps\/(?:build|manage)\//.test(path)
    );
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

  function inferPlaneTierFromMeta(meta, nodeId) {
    const fromMeta = normalizePreviewTier(meta?.plane_tier || "");
    if (fromMeta) return fromMeta;
    const scope = String(meta?.preview_scope || "").trim();
    const hay = `${scope}:${String(nodeId || "").trim()}`;
    const match = hay.match(/(?:^|\/)(T[0-2])(?:\/|$)/i);
    return match ? normalizePreviewTier(match[1]) : "";
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
    const sourceMeta = readSourceMetaFromReachabilityTree(node);
    const bits = [];
    if (node) bits.push(`node=${node}`);
    if (focus) bits.push(`focus=${focus}`);
    if (sourceMeta?.file) bits.push(`src=${sourceMeta.file}`);
    if (sourceMeta?.symbol) bits.push(`sym=${sourceMeta.symbol}`);
    if (panelId) bits.push(`panel=${panelId}`);
    if (blockId) bits.push(`block=${blockId}`);
    if (useKey) bits.push(`use=${useKey}`);
    bar.textContent = bits.join(" · ");
  }

  function readSourceMetaFromReachabilityTree(nodeId) {
    const id = String(nodeId || "").trim();
    if (!id) return null;
    const script = document.getElementById("mei-build-reachability-tree");
    if (!script) return null;
    try {
      const roots = JSON.parse(script.textContent || "[]");
      if (!Array.isArray(roots)) return null;
      const walk = (nodes) => {
        for (const node of nodes || []) {
          if (node?.node_id === id) {
            const file = String(node.source_file || "").trim();
            const symbol = String(node.source_symbol || "").trim();
            if (file || symbol) return { file, symbol };
            return null;
          }
          const nested = walk(node.children);
          if (nested) return nested;
        }
        return null;
      };
      for (const root of roots) {
        const found = walk(root.children);
        if (found) return found;
      }
    } catch (_) {}
    return null;
  }

  function previewInspectHost(root) {
    if (!(root instanceof HTMLElement)) return null;
    return (
      root.querySelector(".preview-pane-scroll") ||
      root.querySelector(".preview-surface") ||
      root
    );
  }

  function syncInspectModeAttributes(root, meta, nodeId) {
    const host = previewInspectHost(root);
    if (!(host instanceof HTMLElement)) return;
    host.setAttribute("data-build-inspect-active", "true");
    const role = String(meta?.ui_role || "").trim();
    const scope = String(meta?.preview_scope || "").trim();
    const tier =
      normalizePreviewTier(meta?.plane_tier || "") || inferPlaneTierFromMeta(meta, nodeId);
    const node = String(nodeId || "").trim();
    if (node) host.setAttribute("data-build-inspect-node", node);
    else host.removeAttribute("data-build-inspect-node");
    if (role) host.setAttribute("data-build-inspect-role", role);
    else host.removeAttribute("data-build-inspect-role");
    if (scope) host.setAttribute("data-build-inspect-scope", scope);
    else host.removeAttribute("data-build-inspect-scope");
    // plane 仅描边，不裁剪 tier（否则 DOM 默认 t1 时会把整屏藏空）
    if (role === "plane") {
      host.removeAttribute("data-build-inspect-tier");
    } else if (tier) {
      host.setAttribute("data-build-inspect-tier", tier);
    } else {
      host.removeAttribute("data-build-inspect-tier");
    }
  }

  function clearInspectModeAttributes(root) {
    const host = previewInspectHost(root);
    if (!(host instanceof HTMLElement)) return;
    host.removeAttribute("data-build-inspect-active");
    host.removeAttribute("data-build-inspect-node");
    host.removeAttribute("data-build-inspect-role");
    host.removeAttribute("data-build-inspect-scope");
    host.removeAttribute("data-build-inspect-tier");
  }

  /** 仅同步 inspect 宿主属性，供 CSS 切换 tier 可见性；不使用 opacity 蒙板（会连带压暗地图与子内容）。 */
  function applyInspectFocusChrome(root, meta, nodeId) {
    syncInspectModeAttributes(root, meta, nodeId);
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

  function normalizePreviewScopePath(scope) {
    const segments = String(scope || "")
      .split("/")
      .map((part) => part.trim())
      .filter(Boolean);
    const out = [];
    for (const segment of segments) {
      if (out[out.length - 1] !== segment) out.push(segment);
    }
    return out.join("/");
  }

  function scopeAlignScore(uiScope, previewScope) {
    const scope = normalizePreviewScopePath(uiScope);
    const target = normalizePreviewScopePath(previewScope);
    if (!scope || !target) return 0;
    if (scope === target) return 10_000 + target.length;
    if (scope.endsWith(`/${target}`)) return 9_000 + target.length;
    const scopeParts = scope.split("/").filter(Boolean);
    const targetParts = target.split("/").filter(Boolean);
    if (!scopeParts.length || !targetParts.length) return 0;
    let panelIndex = 0;
    let matched = 0;
    for (const part of targetParts) {
      let found = false;
      while (panelIndex < scopeParts.length) {
        if (scopeParts[panelIndex] === part) {
          matched += 1;
          panelIndex += 1;
          found = true;
          break;
        }
        panelIndex += 1;
      }
      if (!found) return 0;
    }
    return 6_000 + matched * 100 + target.length;
  }

  function isInspectTargetVisible(el) {
    if (!(el instanceof HTMLElement)) return false;
    let node = el;
    while (node && node !== document.body) {
      const style = getComputedStyle(node);
      if (style.display === "none" || style.visibility === "hidden") return false;
      node = node.parentElement;
    }
    return true;
  }

  function pickBestInspectTarget(candidates, previewScope) {
    const pool = (Array.isArray(candidates) ? candidates : []).filter(
      (el) => el instanceof HTMLElement,
    );
    if (!pool.length) return null;
    if (pool.length === 1) return pool[0];
    const target = normalizePreviewScopePath(previewScope);
    let best = pool[0];
    let bestScore = -1;
    for (const el of pool) {
      let score = scopeAlignScore(
        el.getAttribute("data-mei-ui-scope") || el.getAttribute("data-preview-scope") || "",
        target,
      );
      if (isInspectTargetVisible(el)) score += 50;
      if (score > bestScore) {
        bestScore = score;
        best = el;
      }
    }
    return best;
  }

  function scopePathLength(el) {
    if (!(el instanceof HTMLElement)) return 0;
    const path = String(
      el.getAttribute("data-mei-ui-scope") || el.getAttribute("data-preview-scope") || "",
    ).trim();
    return path.length;
  }

  function contentUseKeyFromMeta(meta) {
    const badges = Array.isArray(meta?.badges) ? meta.badges : [];
    for (let i = badges.length - 1; i >= 0; i -= 1) {
      const badge = String(badges[i] || "").trim();
      if (!badge || badge === "content") continue;
      if (/^plane:/i.test(badge)) continue;
      if (/^(?:region|section|plane|scene|micro|slot|budget)$/i.test(badge)) continue;
      return badge;
    }
    return "";
  }

  function blockKeyCandidates(scope) {
    const keys = [];
    const last = scope.split("/").filter(Boolean).pop() || "";
    if (last) keys.push(last);
    if (last.includes("~")) {
      const stem = last.split("~")[0];
      if (stem) keys.push(stem);
    }
    return keys;
  }

  function resolveUiScopeHighlightTargets(root, node, meta) {
    const role = String(meta?.ui_role || "").trim().toLowerCase();
    const scope = String(meta?.preview_scope || "").trim();
    const nodeId = String(node || "").trim();
    if (!nodeId) return [];

    const byBuildNode = Array.from(
      root.querySelectorAll(`[data-build-node="${CSS.escape(nodeId)}"]`),
    ).filter((el) => el instanceof HTMLElement);
    if (byBuildNode.length === 1) return byBuildNode;
    if (byBuildNode.length > 1) {
      const picked = pickBestInspectTarget(byBuildNode, scope);
      return picked ? [picked] : [byBuildNode[0]];
    }

    if (scope) {
      const exactScope = root.querySelector(`[data-mei-ui-scope="${CSS.escape(scope)}"]`);
      if (exactScope instanceof HTMLElement) return [exactScope];
      const normalizedScope = normalizePreviewScopePath(scope);
      if (normalizedScope && normalizedScope !== scope) {
        const normalizedExact = root.querySelector(
          `[data-mei-ui-scope="${CSS.escape(normalizedScope)}"]`,
        );
        if (normalizedExact instanceof HTMLElement) return [normalizedExact];
      }
      const exactPreview = root.querySelector(`[data-preview-scope="${CSS.escape(scope)}"]`);
      if (exactPreview instanceof HTMLElement) return [exactPreview];

      const nested = Array.from(
        root.querySelectorAll(`[data-mei-ui-scope^="${CSS.escape(scope)}/"]`),
      ).filter((el) => el instanceof HTMLElement);
      if (nested.length) {
        nested.sort((a, b) => scopePathLength(b) - scopePathLength(a));
        return [nested[0]];
      }
    }

    if (role === "content") {
      const areaHint = scope.split("/").filter(Boolean).slice(-2, -1)[0] || "";
      for (const blockKey of blockKeyCandidates(scope)) {
        const byBlock = root.querySelector(`[data-mei-block-id="${CSS.escape(blockKey)}"]`);
        if (byBlock instanceof HTMLElement) return [byBlock];
      }
      const scopedBlocks = Array.from(
        root.querySelectorAll("[data-mei-ui-scope], [data-mei-block-id], [data-mei-panel-area]"),
      ).filter((el) => {
        if (!(el instanceof HTMLElement)) return false;
        const uiScope = String(el.getAttribute("data-mei-ui-scope") || "").trim();
        const blockId = String(el.getAttribute("data-mei-block-id") || "").trim();
        const panelArea = String(el.getAttribute("data-mei-panel-area") || "").trim();
        const areaMatches = !areaHint || !panelArea || panelArea === areaHint;
        return (
          areaMatches &&
          blockKeyCandidates(scope).some(
            (key) =>
              blockId === key ||
              uiScope === scope ||
              normalizePreviewScopePath(uiScope) === normalizePreviewScopePath(scope) ||
              (scope && uiScope.endsWith(`/${key}`)) ||
              (blockId && scope.endsWith(`/${blockId}`)),
          )
        );
      });
      if (scopedBlocks.length) {
        const picked = pickBestInspectTarget(scopedBlocks, scope);
        return picked ? [picked] : [scopedBlocks[0]];
      }
      const useKey = contentUseKeyFromMeta(meta);
      if (useKey) {
        const byUse = root.querySelector(`[data-mei-use-key="${CSS.escape(useKey)}"]`);
        if (byUse instanceof HTMLElement) return [byUse];
      }
    }

    return fallbackScenePanelFromUiScope(root, nodeId);
  }

  function fallbackScenePanelFromUiScope(root, nodeId) {
    const raw = String(nodeId || "").trim();
    if (!raw.toLowerCase().startsWith("ui-scope:")) return [];
    const payload = raw.includes(":") ? raw.split(":").slice(1).join(":") : raw;
    const segments = payload.split("/").filter(Boolean);
    if (segments.length < 2) return [];
    const scene = segments[0];
    const tail = segments.slice(2);
    const leaf = tail[tail.length - 1] || segments[segments.length - 1];
    const candidates = Array.from(
      root.querySelectorAll(`[data-build-node^="scene-panel:${CSS.escape(scene)}/"]`),
    ).filter((el) => {
      if (!(el instanceof HTMLElement)) return false;
      const id = String(el.getAttribute("data-build-node") || "");
      if (leaf && id.endsWith(leaf)) return true;
      return tail.length > 0 && tail.every((seg) => id.includes(seg));
    });
    if (!candidates.length) return [];
    candidates.sort((a, b) => {
      const al = String(a.getAttribute("data-build-node") || "").length;
      const bl = String(b.getAttribute("data-build-node") || "").length;
      return bl - al;
    });
    return [candidates[0]];
  }

  function readUiScopeMetaFromNode(nodeId) {
    const entry = findReachabilityNodeEntry(nodeId);
    if (!entry?.node) return null;
    return {
      ui_role: String(entry.node.ui_role || entry.node.badges?.[0] || "").trim(),
      preview_scope: String(entry.node.preview_scope || "").trim(),
      plane_tier: String(entry.node.plane_tier || "").trim(),
      badges: Array.isArray(entry.node.badges) ? entry.node.badges : [],
    };
  }

  function normalizePreviewTier(value) {
    const raw = String(value || "").trim().toLowerCase();
    if (!raw) return "";
    if (raw === "t0" || raw === "t1" || raw === "t2") return raw;
    if (raw === "p" || raw === "c" || raw === "h") return raw;
    return raw;
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
