/**
 * Route predicates (global). Load before host-heartbeat.
 * Keep in sync with `UiRouteMode::from_slug` (app/src/ui/route.rs).
 */
(function initRoutePredicatesStandalone(global) {
  "use strict";

  const ACCESS_LIKE_ROUTE_SLUGS = new Set([
    "app",
    "access",
    "access-only",
    "access_only",
  ]);
  const WORKSPACE_SURFACE_SLUGS = new Set(["layout", "prototype"]);
  const APP_WORKSPACE_SURFACE_SLUGS = new Set(["app", "layout", "prototype"]);
  const RUNTIME_ROUTE_SLUGS = new Set(["runtime"]);
  const LEGACY_REMOVED_ROUTE_SLUGS = new Set([
    "build",
    "manage",
    "run",
    "copilot",
    "speaker",
    "presentation",
    "slides",
  ]);

  function pathSegments(pathname = global.location?.pathname) {
    return String(pathname || "")
      .split("/")
      .filter((part) => part.trim().length > 0);
  }

  function legacyRouteSlugFromPathname(pathname = global.location?.pathname) {
    const segments = pathSegments(pathname);
    if (segments[0] !== "apps" || segments.length < 2) return "";
    return String(segments[1] || "").trim().toLowerCase();
  }

  function isUnifiedViewRoute(pathname = global.location?.pathname) {
    const segments = pathSegments(pathname);
    return segments[0] === "apps" && segments.length >= 3 && segments[2] === "view";
  }

  function surfaceSlugFromViewUrl(urlLike, search) {
    try {
      const raw = String(urlLike || "");
      const url =
        raw.includes("://") || raw.includes("?")
          ? new URL(raw, global.location?.href || "http://localhost")
          : new URL(
              `${raw}${search || global.location?.search || ""}`,
              global.location?.href || "http://localhost",
            );
      if (!isUnifiedViewRoute(url.pathname)) return "";
      const surface = String(url.searchParams.get("surface") || "app")
        .trim()
        .toLowerCase();
      if (surface === "layout" || surface === "prototype" || surface === "app") {
        return surface;
      }
      return "app";
    } catch (_) {
      return "";
    }
  }

  function appSurfaceSlugFromPathname(pathname = global.location?.pathname) {
    const segments = pathSegments(pathname);
    if (segments[0] !== "apps" || segments.length < 3) return "";
    if (segments[2] === "view") {
      if (global.location?.pathname === pathname) {
        return surfaceSlugFromViewUrl(pathname, global.location?.search) || "app";
      }
      return "app";
    }
    return String(segments[2] || "").trim().toLowerCase();
  }

  function appRouteSlugFromPathname(pathname = global.location?.pathname) {
    const surface = appSurfaceSlugFromPathname(pathname);
    if (surface) return surface;
    return legacyRouteSlugFromPathname(pathname);
  }

  function isAppSurfaceRoute(pathname = global.location?.pathname) {
    if (isUnifiedViewRoute(pathname)) {
      const surface =
        global.location?.pathname === pathname
          ? surfaceSlugFromViewUrl(pathname, global.location?.search)
          : "app";
      return (surface || "app") === "app";
    }
    return appSurfaceSlugFromPathname(pathname) === "app";
  }

  function isAppWorkspaceSurfaceRoute(pathname = global.location?.pathname) {
    if (isUnifiedViewRoute(pathname)) {
      const surface =
        global.location?.pathname === pathname
          ? surfaceSlugFromViewUrl(pathname, global.location?.search)
          : "app";
      return APP_WORKSPACE_SURFACE_SLUGS.has(surface || "app");
    }
    return APP_WORKSPACE_SURFACE_SLUGS.has(appSurfaceSlugFromPathname(pathname));
  }

  function isWorkspaceSurfaceRoute(pathname = global.location?.pathname) {
    if (isUnifiedViewRoute(pathname)) {
      const surface =
        global.location?.pathname === pathname
          ? surfaceSlugFromViewUrl(pathname, global.location?.search)
          : "";
      return WORKSPACE_SURFACE_SLUGS.has(surface);
    }
    return WORKSPACE_SURFACE_SLUGS.has(appSurfaceSlugFromPathname(pathname));
  }

  function isWorkspaceSurfaceUrl(urlLike) {
    try {
      const url = new URL(String(urlLike || ""), global.location?.href || "http://localhost");
      if (isUnifiedViewRoute(url.pathname)) {
        const surface = String(url.searchParams.get("surface") || "app").trim().toLowerCase();
        return WORKSPACE_SURFACE_SLUGS.has(surface);
      }
      return WORKSPACE_SURFACE_SLUGS.has(appSurfaceSlugFromPathname(url.pathname));
    } catch (_) {
      return false;
    }
  }

  function isLegacyRemovedRoute(pathname = global.location?.pathname) {
    return LEGACY_REMOVED_ROUTE_SLUGS.has(legacyRouteSlugFromPathname(pathname));
  }

  function isLegacyPresentationRoute(pathname = global.location?.pathname) {
    const slug = legacyRouteSlugFromPathname(pathname);
    return (
      slug === "run" ||
      slug === "copilot" ||
      slug === "speaker" ||
      slug === "presentation" ||
      slug === "slides"
    );
  }

  function appRoutePrefixesFromSlugs(slugs) {
    return Array.from(slugs, (slug) => `/apps/${slug}/`);
  }

  function isAppRoute(pathname = global.location?.pathname) {
    if (isAppSurfaceRoute(pathname)) return true;
    return ACCESS_LIKE_ROUTE_SLUGS.has(legacyRouteSlugFromPathname(pathname));
  }

  function isRuntimeRoute(pathname = global.location?.pathname) {
    const path = String(pathname || "");
    if (path === "/runtime" || path.startsWith("/runtime?")) return true;
    return RUNTIME_ROUTE_SLUGS.has(legacyRouteSlugFromPathname(pathname));
  }

  /** @deprecated Use isWorkspaceSurfaceRoute */
  function isBuildRoute(pathname = global.location?.pathname) {
    return isWorkspaceSurfaceRoute(pathname);
  }

  function isConfigRoute(pathname = global.location?.pathname) {
    const path = String(pathname || "");
    return path === "/config" || path.startsWith("/config?") || path.startsWith("/apps/config/");
  }

  function isUploadRoute(pathname = global.location?.pathname) {
    const path = String(pathname || "");
    return path === "/upload" || path.startsWith("/upload?") || path.startsWith("/apps/upload/");
  }

  function isStandaloneViewRoute(pathname = global.location?.pathname) {
    return isConfigRoute(pathname) || isUploadRoute(pathname);
  }

  function isAccessRoute(pathname = global.location?.pathname) {
    return isAppRoute(pathname);
  }

  /** @deprecated Use isWorkspaceSurfaceRoute */
  function isManageRoute(pathname = global.location?.pathname) {
    return isWorkspaceSurfaceRoute(pathname);
  }

  function shouldMountDrilldownHost(pathname = global.location?.pathname) {
    const slug = appRouteSlugFromPathname(pathname);
    return (
      ACCESS_LIKE_ROUTE_SLUGS.has(slug) ||
      WORKSPACE_SURFACE_SLUGS.has(slug) ||
      RUNTIME_ROUTE_SLUGS.has(slug)
    );
  }

  function isBoardLinkConfig(popup) {
    if (!popup || typeof popup !== "object") return false;
    return popup.mode === "board_link" || popup.__kind === "board_link";
  }

  function isPanelPopupConfig(popup) {
    if (!popup || typeof popup !== "object") return false;
    return popup.mode === "popup_panel" || popup.__kind === "popup_panel";
  }

  function buildTabFromUrl(rawUrl) {
    try {
      const url = new URL(rawUrl, global.location.href);
      if (isWorkspaceSurfaceRoute(url.pathname)) return "";
      const tab = url.searchParams.get("tab");
      if (tab) return String(tab).trim().toLowerCase();
      return "overview";
    } catch (_) {
      return "overview";
    }
  }

  function shouldRunBuildPreviewRuntimeForUrl(rawUrl) {
    try {
      const url = new URL(rawUrl, global.location.href);
      return isWorkspaceSurfaceRoute(url.pathname);
    } catch (_) {
      return false;
    }
  }

  function isBuildWorkspacePathname(pathname = global.location?.pathname) {
    return isWorkspaceSurfaceRoute(pathname);
  }

  function appIdFromAppsPathname(pathname = global.location?.pathname) {
    const segments = pathSegments(pathname);
    if (segments[0] !== "apps" || segments.length < 2) {
      return "";
    }
    const surface = appSurfaceSlugFromPathname(pathname);
    if (
      surface === "view" ||
      WORKSPACE_SURFACE_SLUGS.has(surface) ||
      surface === "app" ||
      ACCESS_LIKE_ROUTE_SLUGS.has(surface)
    ) {
      return String(segments[1] || "").trim();
    }
    if (RUNTIME_ROUTE_SLUGS.has(segments[1]) && segments.length >= 3) {
      return String(segments[2] || "").trim();
    }
    if (ACCESS_LIKE_ROUTE_SLUGS.has(segments[1]) && segments.length >= 3) {
      return String(segments[2] || "").trim();
    }
    return String(segments[1] || "").trim();
  }

  function workspaceSurfaceSlugFromAppsPathname(pathname = global.location?.pathname) {
    if (isUnifiedViewRoute(pathname)) {
      const surface = surfaceSlugFromViewUrl(pathname);
      return WORKSPACE_SURFACE_SLUGS.has(surface) ? surface : "";
    }
    const surface = appSurfaceSlugFromPathname(pathname);
    return WORKSPACE_SURFACE_SLUGS.has(surface) ? surface : "";
  }

  function sceneIdFromPathname(pathname = global.location?.pathname, search = global.location?.search) {
    if (isUnifiedViewRoute(pathname)) {
      try {
        const url = new URL(pathname + (search || ""), global.location?.href || "http://localhost");
        const fromQuery = String(url.searchParams.get("scene") || "").trim();
        if (fromQuery) return fromQuery;
      } catch (_) {}
    }
    const segments = pathSegments(pathname);
    if (segments[0] !== "apps") return "";
    const sceneIdx = segments.indexOf("scene");
    if (sceneIdx >= 0 && segments[sceneIdx + 1]) {
      return decodeURIComponent(segments[sceneIdx + 1]);
    }
    const shell = global.document?.querySelector?.(".shell[data-scene], [data-scene]");
    const fromDom = shell ? String(shell.getAttribute("data-scene") || "").trim() : "";
    if (fromDom) return fromDom;
    const bodyScene = global.document?.body?.getAttribute?.("data-scene-id");
    if (bodyScene) return String(bodyScene).trim();
    return "home";
  }

  function isRevisionFirstShellPage(pathname = global.location?.pathname) {
    if (globalThis.__mei?.thin_shell === true) return true;
    if (isUnifiedViewRoute(pathname)) return true;
    if (isAppWorkspaceSurfaceRoute(pathname)) return true;
    if (isAccessRoute(pathname)) return true;
    return false;
  }

  function isPresentationCapableRoute(pathname = global.location?.pathname) {
    return isAppSurfaceRoute(pathname) || isAccessRoute(pathname);
  }

  function rewriteLegacyPresentationRoute(route) {
    const raw = String(route || "").trim();
    if (!raw) return raw;
    try {
      const base = String(global.location?.origin || "http://localhost");
      const url = raw.startsWith("/") ? new URL(raw, base) : new URL(raw);
      let path = url.pathname;
      let rewritten = false;
      const runMatch = path.match(/^\/apps\/(?:run|presentation|slides)\/([^/]+)(\/.*)?$/);
      if (runMatch) {
        path = `/apps/${runMatch[1]}/view?surface=app`;
        rewritten = true;
      }
      const copilotMatch = path.match(/^\/apps\/(?:copilot|speaker)\/([^/]+)(\/.*)?$/);
      if (copilotMatch) {
        path = `/apps/${copilotMatch[1]}/view?surface=app`;
        rewritten = true;
      }
      const legacyAppMatch = path.match(
        /^\/apps\/(?:app|access|access-only|access_only)\/([^/]+)(\/.*)?$/,
      );
      if (legacyAppMatch) {
        path = `/apps/${legacyAppMatch[1]}/view?surface=app`;
        rewritten = true;
      }
      const legacySurfaceMatch = path.match(/^\/apps\/([^/]+)\/(layout|prototype)(\/.*)?$/);
      if (legacySurfaceMatch) {
        path = `/apps/${legacySurfaceMatch[1]}/view?surface=${legacySurfaceMatch[2]}`;
        rewritten = true;
      }
      if (!rewritten) return raw;
      return `${path}${url.search}${url.hash}`;
    } catch (_) {
      return raw;
    }
  }

  const api = {
    ACCESS_LIKE_ROUTE_SLUGS,
    WORKSPACE_SURFACE_SLUGS,
    APP_WORKSPACE_SURFACE_SLUGS,
    RUNTIME_ROUTE_SLUGS,
    LEGACY_REMOVED_ROUTE_SLUGS,
    pathSegments,
    isUnifiedViewRoute,
    surfaceSlugFromViewUrl,
    legacyRouteSlugFromPathname,
    appSurfaceSlugFromPathname,
    appRouteSlugFromPathname,
    isAppSurfaceRoute,
    isAppWorkspaceSurfaceRoute,
    isWorkspaceSurfaceRoute,
    isWorkspaceSurfaceUrl,
    isLegacyRemovedRoute,
    isLegacyPresentationRoute,
    appRoutePrefixesFromSlugs,
    isAppRoute,
    isRuntimeRoute,
    isBuildRoute,
    isConfigRoute,
    isUploadRoute,
    isStandaloneViewRoute,
    isAccessRoute,
    isManageRoute,
    shouldMountDrilldownHost,
    isBoardLinkConfig,
    isPanelPopupConfig,
    buildTabFromUrl,
    shouldRunBuildPreviewRuntimeForUrl,
    isBuildWorkspacePathname,
    appIdFromAppsPathname,
    workspaceSurfaceSlugFromAppsPathname,
    sceneIdFromPathname,
    isRevisionFirstShellPage,
    isPresentationCapableRoute,
    rewriteLegacyPresentationRoute,
  };

  global.MeiRoutePredicates = api;
  global.appIdFromAppsPathname = appIdFromAppsPathname;
  global.workspaceSurfaceSlugFromAppsPathname = workspaceSurfaceSlugFromAppsPathname;
  global.appRouteSlugFromPathname = appRouteSlugFromPathname;
  global.isBuildWorkspacePathname = isBuildWorkspacePathname;
  global.isAppWorkspaceSurfaceRoute = isAppWorkspaceSurfaceRoute;
  global.isWorkspaceSurfaceRoute = isWorkspaceSurfaceRoute;
  global.isWorkspaceSurfaceUrl = isWorkspaceSurfaceUrl;
  global.isAppSurfaceRoute = isAppSurfaceRoute;
  global.isBuildRoute = isBuildRoute;
  global.isAppRoute = isAppRoute;
  global.isAccessRoute = isAccessRoute;
  global.isUnifiedViewRoute = isUnifiedViewRoute;
  global.surfaceSlugFromViewUrl = surfaceSlugFromViewUrl;
  global.isPresentationCapableRoute = isPresentationCapableRoute;
  global.rewriteLegacyPresentationRoute = rewriteLegacyPresentationRoute;
})(typeof window !== "undefined" ? window : globalThis);
