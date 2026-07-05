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

  function resolveSurface(pathname, searchParams) {
    if (typeof isUnifiedViewRoute === "function" && isUnifiedViewRoute(pathname)) {
      const fromQuery = String(searchParams?.get("surface") || "app")
        .trim()
        .toLowerCase();
      if (fromQuery === "layout" || fromQuery === "prototype" || fromQuery === "app") {
        return fromQuery;
      }
      return "app";
    }
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
    if (slug === "layout" || slug === "prototype") return slug;
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
      const surface = resolveSurface(pathname, url.searchParams);
      const sceneFromQuery = String(url.searchParams.get("scene") || "").trim();
      const sceneId = sceneFromQuery
        || (typeof sceneIdFromPathname === "function"
          ? sceneIdFromPathname(pathname, url.search)
          : "")
        || "home";
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
    return slug === "layout" || slug === "prototype";
  }

  function resolveComposeRoot(surface) {
    const slug = String(surface || "").trim().toLowerCase();
    if (slug === "app") {
      const byId = global.document?.getElementById?.("mei-compose-root");
      if (byId instanceof HTMLElement) return byId;
    }
    if (isWorkspaceComposeSurface(slug)) {
      const preview =
        global.document?.querySelector?.("#mei-surface-workspace .preview-pane-scroll") ||
        global.document?.querySelector?.(".preview-pane-scroll") ||
        global.document?.querySelector?.('[data-manage-tab-panel="preview"] .preview-pane-scroll');
      if (preview instanceof HTMLElement) return preview;
    }
    const shell = global.document?.querySelector?.(".shell");
    return shell instanceof HTMLElement ? shell : null;
  }

  function canonicalizeViewUrl(urlLike) {
    try {
      const url = new URL(urlLike, global.location.href);
      if (typeof isUnifiedViewRoute !== "function" || !isUnifiedViewRoute(url.pathname)) {
        return url.href;
      }
      const surface = String(url.searchParams.get("surface") || "app").trim().toLowerCase();
      const next = new URL(url.href);
      next.search = "";
      next.searchParams.set("surface", surface);
      const scene = String(url.searchParams.get("scene") || "").trim();
      if (scene) next.searchParams.set("scene", scene);
      const chrome = String(url.searchParams.get("chrome") || "").trim().toLowerCase();
      if (chrome && chrome !== "full") next.searchParams.set("chrome", chrome);
      return next.href;
    } catch (_) {
      return urlLike;
    }
  }

  boot.parseViewContext = parseViewContext;
  boot.canonicalizeViewUrl = canonicalizeViewUrl;
  boot.isWorkspaceComposeSurface = isWorkspaceComposeSurface;
  boot.resolveComposeRoot = resolveComposeRoot;
})(typeof window !== "undefined" ? window : globalThis);
