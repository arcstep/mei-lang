/**
 * Unified view context for access / layout / prototype / build surfaces.
 */
(function initViewContext(global) {
  "use strict";

  const boot = (global.__meiLangBoot = global.__meiLangBoot || {});

  function resolveBuildNodeFromUrl(url) {
    const fromQuery = String(url.searchParams.get("node") || "").trim();
    if (fromQuery) return fromQuery;
    const shell = global.document?.querySelector?.(".shell[data-build-node]");
    if (shell instanceof HTMLElement) {
      const fromDom = String(shell.getAttribute("data-build-node") || "").trim();
      if (fromDom) return fromDom;
    }
    return "";
  }

  function resolveSurface(pathname) {
    const wsSurface =
      typeof workspaceSurfaceSlugFromAppsPathname === "function"
        ? workspaceSurfaceSlugFromAppsPathname(pathname)
        : "";
    if (wsSurface) return wsSurface;
    const slug =
      typeof appRouteSlugFromPathname === "function"
        ? appRouteSlugFromPathname(pathname)
        : "";
    if (slug === "app" || slug === "access" || slug === "access-only" || slug === "access_only") {
      return "app";
    }
    if (slug === "run" || slug === "copilot" || slug === "speaker" || slug === "presentation") {
      return slug === "speaker" ? "copilot" : slug;
    }
    if (slug === "build" || slug === "manage") return "build";
    return slug || "app";
  }

  function parseViewContext(urlLike) {
    try {
      const url = new URL(urlLike, global.location.href);
      const pathname = url.pathname;
      const appId =
        typeof appIdFromAppsPathname === "function"
          ? appIdFromAppsPathname(pathname)
          : "";
      if (!appId) return null;
      const surface = resolveSurface(pathname);
      const sceneId =
        typeof sceneIdFromPathname === "function"
          ? sceneIdFromPathname(pathname)
          : String(url.searchParams.get("scene") || "home").trim() || "home";
      const dataMode = String(url.searchParams.get("data_mode") || "")
        .trim()
        .toLowerCase();
      const reviewProjection = String(url.searchParams.get("review_projection") || "")
        .trim()
        .toLowerCase();
      const chrome = String(url.searchParams.get("chrome") || "").trim().toLowerCase();
      const tab = String(url.searchParams.get("tab") || "").trim().toLowerCase();
      const node = resolveBuildNodeFromUrl(url);
      return {
        app_id: appId,
        appId,
        scene_id: sceneId,
        sceneId,
        surface,
        mode: surface,
        node,
        data_mode: dataMode,
        dataMode,
        review_projection: reviewProjection,
        reviewProjection,
        chrome,
        tab,
        focus: String(url.searchParams.get("focus") || "").trim(),
        scope: String(url.searchParams.get("scope") || "").trim(),
        url: url.href,
      };
    } catch (_) {
      return null;
    }
  }

  function isWorkspaceComposeSurface(surface) {
    const slug = String(surface || "").trim().toLowerCase();
    return slug === "build" || slug === "layout" || slug === "prototype";
  }

  function resolveComposeRoot(surface) {
    const slug = String(surface || "").trim().toLowerCase();
    if (isWorkspaceComposeSurface(slug)) {
      const preview =
        global.document?.querySelector?.(".preview-pane-scroll") ||
        global.document?.querySelector?.('[data-manage-tab-panel="preview"] .preview-pane-scroll');
      if (preview instanceof HTMLElement) return preview;
    }
    const shell = global.document?.querySelector?.(".shell");
    return shell instanceof HTMLElement ? shell : null;
  }

  boot.parseViewContext = parseViewContext;
  boot.isWorkspaceComposeSurface = isWorkspaceComposeSurface;
  boot.resolveComposeRoot = resolveComposeRoot;
})(typeof window !== "undefined" ? window : globalThis);
