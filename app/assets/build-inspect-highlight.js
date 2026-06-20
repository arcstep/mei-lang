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
    return String(activeShell()?.getAttribute("data-build-node") || "").trim();
  }

  function activeBuildFocus() {
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

  function updateInspectBar(node, focus, el) {
    const bar = inspectBarLabel();
    if (!bar) return;
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
      root.querySelector("[data-preview-scope].build-preview-scoped-dim") != null;
    document.body.classList.toggle("build-preview-scoped-active", scopedActive);
  }

  function applyScopedPreview(root) {
    const node = activeBuildNode();
    root.querySelectorAll("[data-preview-scope]").forEach((el) => {
      el.classList.remove("build-preview-scoped-dim");
    });
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

  function scrollIntoViewIfOne(matches) {
    if (matches.length === 1) {
      matches[0].scrollIntoView({ block: "nearest", inline: "nearest", behavior: "smooth" });
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
      scrollIntoViewIfOne(focusMatches);
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
        scrollIntoViewIfOne(selected);
      }
      updateInspectBar(node, focus, focusEl || selected[0] || null);
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

  function clearBuildPreviewArtifacts(root) {
    if (!(root instanceof HTMLElement)) return;
    root.querySelectorAll("[data-preview-scope].build-preview-scoped-dim").forEach((el) => {
      el.classList.remove("build-preview-scoped-dim");
    });
    document.body.classList.remove("build-preview-scoped-active");
    root.querySelectorAll(".preview-surface, .preview-stage").forEach((surface) => {
      if (!(surface instanceof HTMLElement)) return;
      delete surface.dataset.meiPreviewBoardMounted;
      surface.classList.remove("preview-board-mounted");
    });
  }

  function refresh() {
    if (!isBuildRoute()) return;
    document.body.classList.remove("access-drilldown-open", "access-scene-board-open");
    const root = previewRoot();
    if (!root) return;
    clearBuildPreviewArtifacts(root);
    syncShellFocus(readFocusFromUrl());
    bindPreviewInspect(root);
    applyHighlight(root);
  }

  function bind() {
    if (!isBuildRoute()) return;
    refresh();
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
