/**
 * Unified /apps/{id}/view surface switching without document fetch.
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

  function surfaceSlugFromContext(ctx) {
    return String(ctx?.surface || ctx?.mode || "app")
      .trim()
      .toLowerCase();
  }

  function isWorkspaceSurface(slug) {
    return slug === "layout" || slug === "prototype";
  }

  function switchSurfacePanel(surface) {
    const slug = String(surface || "app").trim().toLowerCase();
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
      const keys = ["scene", "chrome", "tab", "file", "node", "scope", "focus", "data_mode", "review_projection"];
      return keys.every((key) => curUrl.searchParams.get(key) === nextUrl.searchParams.get(key));
    } catch (_) {
      return false;
    }
  }

  async function navigateSurface(url, replaceHistory) {
    if (typeof boot.parseViewContext !== "function") return false;
    const nextCtx = boot.parseViewContext(url);
    const currentCtx = boot.parseViewContext(global.location.href);
    if (!nextCtx || !isSurfaceOnlyNavigation(currentCtx, nextCtx)) {
      return false;
    }
    const surface = surfaceSlugFromContext(nextCtx);
    if (typeof boot.showThinShellFallback === "function") {
      boot.showThinShellFallback("正在切换视图…");
    }
    switchSurfacePanel(surface);
    let negotiated = null;
    if (typeof boot.negotiateAndAssemble === "function") {
      negotiated = await boot.negotiateAndAssemble(nextCtx, { silent: true });
    } else if (boot.viewRevisionClient?.negotiateWithLocalMiss) {
      const vrCtx = {
        app_id: nextCtx.app_id || nextCtx.appId,
        scene_id: nextCtx.scene_id || nextCtx.sceneId,
        surface,
        node: nextCtx.node || "",
        data_mode: nextCtx.data_mode || nextCtx.dataMode || "",
        review_projection: nextCtx.review_projection || nextCtx.reviewProjection || "",
        chrome: nextCtx.chrome || "",
        tab: nextCtx.tab || "",
        focus: nextCtx.focus || "",
        scope: nextCtx.scope || "",
      };
      negotiated = await boot.viewRevisionClient.negotiateWithLocalMiss(vrCtx);
    }
    if (!negotiated?.assemble?.ok) {
      if (typeof boot.showThinShellFallback === "function") {
        boot.showThinShellFallback("视图切换失败，请刷新后重试。");
      }
      return false;
    }
    if (typeof boot.hideThinShellFallback === "function") {
      boot.hideThinShellFallback();
    }
    syncTopbarActiveState(surface);
    const canonicalUrl =
      typeof boot.canonicalizeViewUrl === "function"
        ? boot.canonicalizeViewUrl(url)
        : url;
    if (replaceHistory) {
      global.history.replaceState({}, "", canonicalUrl);
    } else {
      global.history.pushState({}, "", canonicalUrl);
    }
    if (typeof runPostSpaWork === "function") {
      runPostSpaWork(global.document, canonicalUrl, null, null, new URL(canonicalUrl, global.location.href));
    }
    return true;
  }

  boot.isUnifiedViewPathname = isUnifiedViewPathname;
  boot.switchSurfacePanel = switchSurfacePanel;
  boot.navigateSurface = navigateSurface;

  function initViewSurfacePanelFromLocation() {
    if (typeof boot.parseViewContext !== "function") return;
    const ctx = boot.parseViewContext(global.location.href);
    if (!ctx) return;
    try {
      const path = new URL(global.location.href).pathname;
      if (isUnifiedViewPathname(path)) {
        switchSurfacePanel(surfaceSlugFromContext(ctx));
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
