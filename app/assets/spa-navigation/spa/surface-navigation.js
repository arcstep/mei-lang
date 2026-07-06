/**
 * Unified /apps/{id}/view surface switching — same cold_start assembly as F5.
 */
(function initSurfaceNavigation(global) {
  "use strict";

  const boot = (global.__meiLangBoot = global.__meiLangBoot || {});
  const surfacePreviewSnapshots = new Map();

  function isUnifiedViewPathname(pathname) {
    const segments = String(pathname || "")
      .split("/")
      .filter((part) => part.trim().length > 0);
    return segments[0] === "apps" && segments.length >= 3 && segments[2] === "view";
  }

  function surfaceSlugFromContext(ctx) {
    return String(ctx?.surface || ctx?.mode || "app")
      .trim()
      .toLowerCase();
  }

  function isWorkspaceSurface(slug) {
    return slug === "layout" || slug === "prototype";
  }

  function workspacePreviewRoot() {
    return global.document?.querySelector?.("#mei-surface-workspace .preview-pane-scroll");
  }

  function appPreviewRoot() {
    return global.document?.getElementById?.("mei-compose-root");
  }

  function previewRootForSurface(surface) {
    const slug = String(surface || "app").trim().toLowerCase();
    if (slug === "app") return appPreviewRoot();
    if (isWorkspaceSurface(slug)) return workspacePreviewRoot();
    return null;
  }

  function previewHasMarkers(root) {
    if (!(root instanceof HTMLElement)) return false;
    return !!root.querySelector(
      "[data-preview-scope], [data-mei-frame-viewport], .preview-viewport, .preview-board-mounted, [data-mei-compose-materialized]",
    );
  }

  function captureSurfacePreviewSnapshot(surface) {
    const slug = String(surface || "").trim().toLowerCase();
    if (!slug) return;
    const el = previewRootForSurface(slug);
    if (!(el instanceof HTMLElement) || !previewHasMarkers(el)) return;
    surfacePreviewSnapshots.set(slug, el.innerHTML);
  }

  function restoreSurfacePreviewSnapshot(surface) {
    const slug = String(surface || "").trim().toLowerCase();
    const el = previewRootForSurface(slug);
    if (!(el instanceof HTMLElement)) return false;
    if (previewHasMarkers(el)) return true;
    const html = surfacePreviewSnapshots.get(slug);
    if (!html) return false;
    el.innerHTML = html;
    el.removeAttribute("data-mei-compose-materialized");
    return true;
  }

  function stashWorkspacePreviewSnapshot() {
    captureSurfacePreviewSnapshot("layout");
    captureSurfacePreviewSnapshot("prototype");
  }

  function restoreWorkspacePreviewSnapshot() {
    const surface = String(
      global.document?.body?.getAttribute("data-surface") || "layout",
    )
      .trim()
      .toLowerCase();
    return restoreSurfacePreviewSnapshot(isWorkspaceSurface(surface) ? surface : "layout");
  }

  function stashAppPreviewSnapshot() {
    captureSurfacePreviewSnapshot("app");
  }

  function restoreAppPreviewSnapshot() {
    return restoreSurfacePreviewSnapshot("app");
  }

  function switchSurfacePanel(surface, options) {
    const opts = options || {};
    const slug = String(surface || "app").trim().toLowerCase();
    const previousSlug = String(
      global.document?.body?.getAttribute("data-surface") ||
        global.document?.body?.getAttribute("data-mei-view") ||
        "app",
    )
      .trim()
      .toLowerCase();
    if (previousSlug !== slug) {
      captureSurfacePreviewSnapshot(previousSlug);
    }
    const appPanel = global.document?.getElementById?.("mei-surface-app");
    const workspacePanel = global.document?.getElementById?.("mei-surface-workspace");
    const showWorkspace = isWorkspaceSurface(slug);
    if (appPanel instanceof HTMLElement) {
      appPanel.hidden = showWorkspace;
      appPanel.classList.toggle("hidden", showWorkspace);
    }
    if (workspacePanel instanceof HTMLElement) {
      workspacePanel.hidden = !showWorkspace;
      workspacePanel.classList.toggle("hidden", !showWorkspace);
    }
    if (global.document?.body instanceof HTMLElement) {
      global.document.body.setAttribute("data-surface", slug);
      global.document.body.setAttribute("data-mei-view", slug);
      if (slug === "prototype") {
        global.document.body.setAttribute("data-mei-prototype", "true");
        global.document.body.setAttribute("data-data-mode", "static");
      } else {
        global.document.body.removeAttribute("data-mei-prototype");
        if (slug === "layout") {
          global.document.body.setAttribute("data-data-mode", "static");
        } else {
          global.document.body.removeAttribute("data-data-mode");
        }
      }
    }
    if (!opts.skipPreviewRestore) {
      restoreSurfacePreviewSnapshot(slug);
    }
    if (showWorkspace) {
      if (typeof boot.installManageTabs === "function") {
        boot.installManageTabs();
      }
      if (typeof globalThis.MeiBuildTreePersist?.refresh === "function") {
        globalThis.MeiBuildTreePersist.refresh();
      }
    }
  }

  function syncTopbarActiveState(surface) {
    const slug = String(surface || "app").trim().toLowerCase();
    const labelMap = {
      app: "应用",
      layout: "布局",
      prototype: "原型",
    };
    const label = labelMap[slug] || "";
    const buttons = global.document?.querySelectorAll?.("sl-button[data-mei-app-view]");
    if (!buttons) return;
    buttons.forEach((button) => {
      if (!(button instanceof HTMLElement)) return;
      const active = String(button.getAttribute("data-mei-app-view") || "").trim() === label;
      button.classList.toggle("is-active", active);
      if (button.classList.contains("mode-tab-btn")) {
        button.classList.toggle("is-active", active);
      }
    });
  }

  function isSameAppViewHost(current, next) {
    if (!current || !next) return false;
    const currentApp = String(current.app_id || current.appId || "").trim();
    const nextApp = String(next.app_id || next.appId || "").trim();
    if (!currentApp || currentApp !== nextApp) return false;
    try {
      const curPath = new URL(current.url || global.location.href, global.location.href).pathname;
      const nextPath = new URL(next.url || global.location.href, global.location.href).pathname;
      return isUnifiedViewPathname(curPath) && isUnifiedViewPathname(nextPath);
    } catch (_) {
      return false;
    }
  }

  function isSurfaceOnlyNavigation(current, next) {
    if (!isSameAppViewHost(current, next)) return false;
    try {
      const curUrl = new URL(current.url || global.location.href, global.location.href);
      const nextUrl = new URL(next.url || global.location.href, global.location.href);
      if (curUrl.pathname !== nextUrl.pathname) return false;
      if (surfaceSlugFromContext(current) === surfaceSlugFromContext(next)) {
        return false;
      }
      const keys = ["scene", "chrome"];
      return keys.every((key) => curUrl.searchParams.get(key) === nextUrl.searchParams.get(key));
    } catch (_) {
      return false;
    }
  }

  function recordSurfaceVisit(url, surface) {
    if (typeof boot.finalizeLoadSession !== "function" || typeof boot.getActiveLoadSession !== "function") {
      return;
    }
    const session = boot.getActiveLoadSession();
    if (!session || session.finalized) return;
    session.label = `surface:${surface}`;
    session.path = url;
    session.url = url;
    session.surface = surface;
    session.kind = session.kind || "navigation";
  }

  async function navigateSurface(url, replaceHistory, navigationId) {
    if (typeof boot.parseViewContext !== "function") return false;
    const nextCtx = boot.parseViewContext(url);
    const currentCtx = boot.parseViewContext(global.location.href);
    if (!nextCtx || !isSurfaceOnlyNavigation(currentCtx, nextCtx)) {
      return false;
    }
    const surface = surfaceSlugFromContext(nextCtx);
    const canonicalUrl =
      typeof boot.canonicalizeViewUrl === "function"
        ? boot.canonicalizeViewUrl(url)
        : url;

    recordSurfaceVisit(canonicalUrl, surface);

    if (replaceHistory) {
      global.location.replace(canonicalUrl);
    } else {
      global.location.assign(canonicalUrl);
    }
    return true;
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
    if (typeof boot.parseViewContext !== "function") return;
    const ctx = boot.parseViewContext(global.location.href);
    if (!ctx) return;
    try {
      const path = new URL(global.location.href).pathname;
      if (isUnifiedViewPathname(path)) {
        const surface = surfaceSlugFromContext(ctx);
        switchSurfacePanel(surface);
        syncTopbarActiveState(surface);
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
