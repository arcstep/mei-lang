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

  function readReviewProjectionFromUrl() {
    try {
      return String(
        new URL(global.location.href).searchParams.get("review_projection") || "",
      ).trim();
    } catch (_) {
      return "";
    }
  }

  const REVIEW_ROLE_DEPTH = { plane: 0, region: 1, section: 2, slot: 3, content: 4 };
  const REVIEW_PROJECTION_MAX_DEPTH = {
    plane: 0,
    plane_region: 1,
    plane_region_section: 2,
    plane_region_section_slot: 3,
    static_full: 99,
    live_full: 99,
    static: 99,
    live: 99,
  };

  function normalizeReviewProjection(value) {
    return String(value || "")
      .trim()
      .toLowerCase()
      .replace(/-/g, "_");
  }

  function elementReviewDepth(el) {
    if (global.MeiStructureAnchor?.elementReviewDepth) {
      return global.MeiStructureAnchor.elementReviewDepth(el);
    }
    if (!(el instanceof HTMLElement)) return 99;
    const role = String(el.getAttribute("data-mei-ui-role") || "")
      .trim()
      .toLowerCase();
    if (role && Object.prototype.hasOwnProperty.call(REVIEW_ROLE_DEPTH, role)) {
      return REVIEW_ROLE_DEPTH[role];
    }
    if (el.hasAttribute("data-mei-panel-id")) return 1;
    if (el.hasAttribute("data-preview-scope")) return 2;
    if (el.hasAttribute("data-mei-use-key") || el.hasAttribute("data-build-node")) return 3;
    return 99;
  }

  function applyReviewProjectionChrome(root) {
    if (global.MeiProjectionDepth?.applyReviewProjectionChrome) {
      global.MeiProjectionDepth.applyReviewProjectionChrome(root);
      return;
    }
    if (!(root instanceof HTMLElement)) return;
    const projection = normalizeReviewProjection(
      root.getAttribute("data-review-projection") || readReviewProjectionFromUrl(),
    );
    const maxDepth = REVIEW_PROJECTION_MAX_DEPTH[projection];
    root.querySelectorAll(".build-review-projection-dim").forEach((el) => {
      el.classList.remove("build-review-projection-dim");
      if (el instanceof HTMLElement) el.style.removeProperty("pointer-events");
    });
    if (maxDepth == null || maxDepth >= 99) {
      root.removeAttribute("data-review-projection-active");
      return;
    }
    root.setAttribute("data-review-projection-active", projection || "static_full");
    root
      .querySelectorAll(
        "[data-mei-ui-role], [data-mei-panel-id], [data-preview-scope], [data-mei-use-key], [data-build-node]",
      )
      .forEach((el) => {
        if (!(el instanceof HTMLElement)) return;
        const depth = elementReviewDepth(el);
        if (depth > maxDepth) {
          el.classList.add("build-review-projection-dim");
          el.style.pointerEvents = "none";
        }
      });
  }

  function applyScopedPreview(root) {
    const node = activeBuildNode();
    root.querySelectorAll("[data-preview-scope], [data-mei-panel-id], [data-chart-slot-index], [data-build-board-slot], [data-mei-use-key], [data-mei-tier], [data-mei-ui-scope]").forEach((el) => {
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
      !node.startsWith("scene-block:") &&
      !node.startsWith("ui-scope:")
    ) {
      syncBuildPreviewScopedChrome(root);
      return;
    }
    if (node.startsWith("ui-scope:")) {
      syncBuildPreviewScopedChrome(root);
      return;
    }
    const encoded = node.replace(/^scene-panel:/, "").replace(/^scene-block:/, "");
    const slash = encoded.indexOf("/");
    const scopePath = slash >= 0 ? encoded.slice(slash + 1) : "";
    if (!scopePath) return;
    const focusedScopeSelector = `[data-preview-scope="${CSS.escape(scopePath)}"], [data-preview-scope^="${CSS.escape(scopePath)}/"]`;
    root.querySelectorAll("[data-preview-scope]").forEach((el) => {
      const elScope = String(el.getAttribute("data-preview-scope") || "");
      if (elScope === scopePath || elScope.startsWith(`${scopePath}/`)) {
        return;
      }
      if (scopePath.startsWith(`${elScope}/`)) {
        return;
      }
      if (el.matches?.(focusedScopeSelector) || el.querySelector?.(focusedScopeSelector)) {
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

  function schedulePreviewRuntimeWake() {
    if (!isBuildRoute()) return;
    const tab = currentManageTab() || "overview";
    if (tab !== "preview") return;
    requestAnimationFrame(() => {
      try {
        global.dispatchEvent(
          new CustomEvent("meilang:preview-updated", {
            detail: { scope: "build-inspect" },
          }),
        );
      } catch (_) {}
    });
  }

  function applyHighlight(root) {
    try {
    const node = activeBuildNode();
    const focus = activeBuildFocus();
    clearHighlights(root);
    if (!node || !node.startsWith("ui-scope:")) {
      clearInspectModeAttributes(root);
      applyScopedPreview(root);
    }

    let focusEl = null;
    if (focus && focus.startsWith("scene-block:")) {
      const focusMatches = root.querySelectorAll(`[data-build-focus="${CSS.escape(focus)}"]`);
      focusMatches.forEach((el) => el.classList.add("build-inspect-focus-selected"));
      scrollIntoViewIfOne(focusMatches, root);
      focusEl = focusMatches[0] || null;
    }

    if (node.startsWith("ui-scope:")) {
      const meta = readUiScopeMetaFromNode(node);
      const role = String(meta?.ui_role || "").trim().toLowerCase();
      let selected = [];
      if (role === "plane" && meta?.plane_tier) {
        const tier = normalizePreviewTier(meta.plane_tier);
        selected = Array.from(root.querySelectorAll("[data-mei-tier]")).filter(
          (el) => normalizePreviewTier(el.getAttribute("data-mei-tier")) === tier,
        );
        if (selected.length === 0) {
          const stage = root.querySelector(
            ".preview-stage, .preview-surface, .preview-stage-shell, [class*='map-host'], [class*='cockpit-map']",
          );
          if (stage instanceof HTMLElement) selected = [stage];
        }
      } else if (role !== "scene") {
        selected = resolveUiScopeHighlightTargets(root, node, meta);
      }
      if (selected.length > 1) {
        selected = [selected[0]];
      }
      selected.forEach((el) => el.classList.add("build-inspect-selected"));
      applyInspectFocusChrome(root, meta, node);
      finalizeInspectHighlight(root, selected);
      if (!focusEl) {
        scrollIntoViewIfOne(selected, root);
      }
      if (selected.length === 0) {
        const scope = String(meta?.preview_scope || "").trim();
        const role = String(meta?.ui_role || "").trim().toLowerCase();
        if (role === "content" && isLayoutWorkspaceSurface()) {
          updateInspectBar(
            node,
            focus,
            null,
            "布局预览不渲染 content；请选 section/slot 节点，或切换到原型视图。",
          );
        } else if (role === "budget" && isLayoutWorkspaceSurface()) {
          updateInspectBar(
            node,
            focus,
            null,
            "Budget 为 gap/padding 元数据节点，无独立 DOM；请选其父 slot 试调布局。",
          );
        } else {
          updateInspectBar(
            node,
            focus,
            null,
            scope
              ? `预览区无 scope 锚点「${scope}」（需先 compose 或检查 scene）`
              : "预览区无对应锚点",
          );
        }
      } else {
        const layoutMsg = isLayoutWorkspaceSurface()
          ? layoutHighlightScopeMessage(meta, selected[0])
          : "";
        if (layoutMsg) {
          updateInspectBar(node, focus, focusEl || selected[0] || null, layoutMsg);
        } else {
          updateInspectBar(node, focus, focusEl || selected[0] || null);
        }
      }
      return;
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
      finalizeInspectHighlight(root, selected);
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
      finalizeInspectHighlight(root, selected);
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
    } finally {
      const node = activeBuildNode();
      const skipWake = !!(node && node.startsWith("ui-scope:"));
      if (!skipWake) {
        schedulePreviewRuntimeWake();
      }
      global.__meiBuildNavPrevUrl = "";
    }
  }

  function currentManageTab() {
    try {
      return String(new URL(global.location.href).searchParams.get("tab") || "").trim().toLowerCase();
    } catch (_) {
      return "";
    }
  }

  function isWorkspaceSurfaceRoute() {
    const path = String(global.location.pathname || "");
    if (/^\/apps\/[^/]+\/(?:layout|prototype)(?:\/|$)/.test(path)) {
      return true;
    }
    try {
      const boot = global.__meiLangBoot;
      if (typeof boot?.parseViewContext === "function") {
        const ctx = boot.parseViewContext(global.location.href);
        const surface = String(ctx?.surface || ctx?.mode || "").trim().toLowerCase();
        return surface === "layout" || surface === "prototype";
      }
      if (typeof isWorkspaceSurfaceRoute === "function" && isWorkspaceSurfaceRoute(path)) {
        return true;
      }
    } catch (_) {}
    return false;
  }

  function selectBuildNodeClient(nodeId, options) {
    const node = String(nodeId || "").trim();
    if (!node) return;
    const shell = activeShell();
    if (shell) {
      shell.setAttribute("data-build-node", node);
    }
    const focus = options && options.focus ? String(options.focus).trim() : "";
    syncShellFocus(focus);
    if (typeof isUnifiedViewRoute === "function" && isUnifiedViewRoute(global.location.pathname)) {
      const url = new URL(global.location.href);
      url.searchParams.set("node", node);
      if (focus) {
        url.searchParams.set("focus", focus);
      } else {
        url.searchParams.delete("focus");
      }
      if (url.href !== global.location.href) {
        global.history.replaceState({}, "", url.href);
      }
    }
    if (global.MeiBuildTreePersist?.refresh) {
      global.MeiBuildTreePersist.refresh({ activeNode: node });
    }
    const root = previewRoot();
    if (root) {
      applyReviewProjectionChrome(root);
      applyHighlight(root);
    }
    const meta = node.startsWith("ui-scope:") ? readUiScopeMetaFromNode(node) : null;
    try {
      global.dispatchEvent(
        new CustomEvent("mei:build-node-selected", {
          bubbles: true,
          detail: {
            nodeId: node,
            preview_scope: String(meta?.preview_scope || "").trim(),
            ui_role: String(meta?.ui_role || "").trim(),
            focus,
          },
        }),
      );
    } catch (_) {}
  }

  function pushBuildUrl(mutator) {
    const shell = activeShell();
    const appPath = shell?.getAttribute("data-app-path") || "";
    if (!appPath) return;
    const url = new URL(global.location.href);
    if (typeof isUnifiedViewRoute === "function" && isUnifiedViewRoute(url.pathname)) {
      const surface = String(url.searchParams.get("surface") || "layout").trim().toLowerCase();
      if (surface !== "layout" && surface !== "prototype") {
        url.searchParams.set("surface", "layout");
      }
      mutator(url);
      const tab = currentManageTab() || String(shell?.getAttribute("data-build-tab") || "").trim().toLowerCase();
      if (tab) {
        url.searchParams.set("tab", tab);
      }
      if (url.href === global.location.href) {
        applyHighlight(previewRoot() || document);
        return;
      }
      global.history.pushState({}, "", url.href);
      global.dispatchEvent(new PopStateEvent("popstate"));
      return;
    }
    if (!isBuildRoute()) return;
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
    if (isWorkspaceSurfaceRoute()) {
      selectBuildNodeClient(node);
      return;
    }
    pushBuildUrl((url) => {
      url.searchParams.set("node", node);
      url.searchParams.delete("focus");
    });
    syncShellFocus("");
  }

  function navigateToBuildFocus(focus) {
    if (!focus) return;
    if (isWorkspaceSurfaceRoute()) {
      selectBuildNodeClient(activeBuildNode(), { focus });
      return;
    }
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
    const eventScope = String(event?.detail?.scope || event?.scope || "").trim();
    if (eventScope === "build-inspect") {
      const root = previewRoot();
      if (!root) return;
      syncShellFocus(readFocusFromUrl());
      applyReviewProjectionChrome(root);
      applyHighlight(root);
      return;
    }
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
    applyReviewProjectionChrome(root);
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

  function debugInspectAnchor(node) {
    const nodeId = String(node || activeBuildNode() || "").trim();
    const meta = readUiScopeMetaFromNode(nodeId);
    const root = previewRoot() || document;
    const selected = resolveUiScopeHighlightTargets(root, nodeId, meta);
    const el = selected[0] || null;
    const targetScope = String(meta?.preview_scope || "").trim();
    const panelPath = el?.getAttribute("data-mei-panel-id") || "";
    const uiScope = el?.getAttribute("data-mei-ui-scope") || "";
    return {
      node: nodeId,
      preview_scope: targetScope,
      ui_role: String(meta?.ui_role || "").trim(),
      matched: el ? readStructureDomScope(el) : "",
      matched_panel: panelPath,
      matched_ui_scope: uiScope,
      affinity_depth: el
        ? previewScopeAffinityDepth(panelPath, uiScope, targetScope)
        : -1,
      panel_id: panelPath,
      rect: el?.getBoundingClientRect?.() || null,
      element: el,
    };
  }

  global.MeiBuildInspectHighlight = {
    refresh,
    navigateToBuildNode,
    navigateToBuildFocus,
    selectBuildNodeClient,
    readUiScopeMetaFromNode,
    readStructureDomScope,
    scopeAlignScore,
    previewScopeAffinityDepth,
    debugInspectAnchor,
  };
})(window);
