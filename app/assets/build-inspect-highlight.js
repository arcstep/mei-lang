/**
 * Build view: preview inspect — highlight, click-to-select node, inspect bar, suppress drilldown.
 */
(function (global) {
  "use strict";

  const SELECTOR =
    "[data-build-node^='scene-panel:'], [data-build-node^='scene-block:']";

  function isBuildRoute() {
    return /^\/apps\/(?:build|manage)\//.test(String(global.location.pathname || ""));
  }

  function activeBuildNode() {
    const shell = document.querySelector(".shell[data-build-node]");
    return String(shell?.getAttribute("data-build-node") || "").trim();
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
    root.querySelectorAll(".build-inspect-selected").forEach((el) => {
      el.classList.remove("build-inspect-selected");
    });
  }

  function updateInspectBar(node, el) {
    const bar = inspectBarLabel();
    if (!bar) return;
    if (!node) {
      bar.textContent = "在左侧体验树选择 Panel/Block，或在预览中点击组件以指认上下文。";
      return;
    }
    const blockId = el?.getAttribute("data-mei-block-id") || "";
    const useKey = el?.getAttribute("data-mei-use-key") || "";
    const panelId = el?.getAttribute("data-mei-panel-id") || "";
    const bits = [`node=${node}`];
    if (panelId) bits.push(`panel=${panelId}`);
    if (blockId) bits.push(`block=${blockId}`);
    if (useKey) bits.push(`use=${useKey}`);
    bar.textContent = bits.join(" · ");
  }

  function applyHighlight(root) {
    const node = activeBuildNode();
    clearHighlights(root);
    if (!node || (!node.startsWith("scene-panel:") && !node.startsWith("scene-block:"))) {
      updateInspectBar("");
      return;
    }
    const matches = root.querySelectorAll(`[data-build-node="${CSS.escape(node)}"]`);
    matches.forEach((el) => el.classList.add("build-inspect-selected"));
    if (matches.length === 1) {
      matches[0].scrollIntoView({ block: "nearest", inline: "nearest", behavior: "smooth" });
      updateInspectBar(node, matches[0]);
    } else {
      updateInspectBar(node, matches[0] || null);
    }
  }

  function navigateToBuildNode(node) {
    if (!node || !isBuildRoute()) return;
    const shell = document.querySelector(".shell[data-build-node]");
    const appPath = shell?.getAttribute("data-app-path") || "";
    if (!appPath) return;
    const url = new URL(global.location.href);
    url.searchParams.set("node", node);
    if (url.searchParams.get("tab") === "" || !url.searchParams.get("tab")) {
      url.searchParams.set("tab", "overview");
    }
    if (url.href === global.location.href) {
      applyHighlight(previewRoot() || document);
      return;
    }
    global.history.pushState({}, "", url.href);
    global.dispatchEvent(new PopStateEvent("popstate"));
  }

  function bindPreviewInspect(root) {
    if (!root || root.__buildInspectBound) return;
    root.__buildInspectBound = true;

    root.addEventListener(
      "click",
      (event) => {
        if (!isBuildRoute()) return;
        const target = event.target.closest(SELECTOR);
        if (target) {
          const node = String(target.getAttribute("data-build-node") || "").trim();
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

  function refresh() {
    if (!isBuildRoute()) return;
    const root = previewRoot();
    if (!root) return;
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

  global.MeiBuildInspectHighlight = { refresh, navigateToBuildNode };
})(window);
