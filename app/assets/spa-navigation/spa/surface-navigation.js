/**
 * Surface navigation stubs — Stage-only Access no longer switches layout/prototype panels.
 * Exports remain so navigation.js / view-assembly-coordinator imports stay safe.
 */
(function initSurfaceNavigation(global) {
  "use strict";

  const boot = (global.__meiLangBoot = global.__meiLangBoot || {});

  function isUnifiedViewPathname(pathname) {
    const segments = String(pathname || "")
      .split("/")
      .filter((part) => part.trim().length > 0);
    return segments[0] === "apps" && segments.length >= 3 && segments[2] === "view";
  }

  /** No-op: dual-panel #mei-surface-workspace switching removed. */
  function switchSurfacePanel(surface, _options) {
    const slug = "app";
    const appPanel = global.document?.getElementById?.("mei-surface-app");
    const workspacePanel = global.document?.getElementById?.("mei-surface-workspace");
    if (appPanel instanceof HTMLElement) {
      appPanel.hidden = false;
      appPanel.classList.remove("hidden");
    }
    if (workspacePanel instanceof HTMLElement) {
      workspacePanel.hidden = true;
      workspacePanel.classList.add("hidden");
    }
    if (global.document?.body instanceof HTMLElement) {
      global.document.body.setAttribute("data-surface", slug);
      global.document.body.setAttribute("data-mei-view", slug);
      global.document.body.removeAttribute("data-mei-prototype");
      global.document.body.removeAttribute("data-data-mode");
    }
    void surface;
  }

  function captureSurfacePreviewSnapshot(_surface) {}

  function restoreSurfacePreviewSnapshot(_surface) {
    return false;
  }

  function stashWorkspacePreviewSnapshot() {}

  function stashAppPreviewSnapshot() {}

  function restoreAppPreviewSnapshot() {
    return false;
  }

  function syncTopbarActiveState(_surface) {
    // 应用/布局/原型顶栏入口已移除；舞台切换由 stage-switcher 承担。
  }

  /**
   * No-op: surface-only SPA switching removed. Return false so callers fall through
   * to normal stage navigation.
   */
  async function navigateSurface(_url, _replaceHistory, _navigationId) {
    return false;
  }

  boot.isUnifiedViewPathname = isUnifiedViewPathname;
  boot.switchSurfacePanel = switchSurfacePanel;
  boot.captureSurfacePreviewSnapshot = captureSurfacePreviewSnapshot;
  boot.restoreSurfacePreviewSnapshot = restoreSurfacePreviewSnapshot;
  boot.stashWorkspacePreviewSnapshot = stashWorkspacePreviewSnapshot;
  boot.stashAppPreviewSnapshot = stashAppPreviewSnapshot;
  boot.restoreAppPreviewSnapshot = restoreAppPreviewSnapshot;
  boot.syncTopbarActiveState = syncTopbarActiveState;
  boot.navigateSurface = navigateSurface;

  function initViewSurfacePanelFromLocation() {
    try {
      const path = new URL(global.location.href).pathname;
      const isStage =
        typeof global.isAccessStageRoute === "function" && global.isAccessStageRoute(path);
      if (isUnifiedViewPathname(path) || isStage) {
        switchSurfacePanel("app");
        syncTopbarActiveState("app");
      }
    } catch (_) {
      /* ignore */
    }
  }

  if (global.document?.readyState === "loading") {
    global.document.addEventListener("DOMContentLoaded", initViewSurfacePanelFromLocation, {
      once: true,
    });
  } else {
    initViewSurfacePanelFromLocation();
  }
})(typeof window !== "undefined" ? window : globalThis);
