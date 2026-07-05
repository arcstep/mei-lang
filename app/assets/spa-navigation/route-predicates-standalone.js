/**
 * Route predicates (global). Load before build-navigation / host-heartbeat.
 * Keep in sync with `UiRouteMode::from_slug` (app/src/ui/route.rs).
 */
(function initRoutePredicatesStandalone(global) {
  "use strict";

  const ACCESS_LIKE_ROUTE_SLUGS = new Set([
    "app",
    "access",
    "run",
    "copilot",
    "speaker",
    "access-only",
    "access_only",
    "presentation",
    "slides",
  ]);
  const BUILD_ROUTE_SLUGS = new Set(["build", "manage"]);
  const WORKSPACE_SURFACE_SLUGS = new Set(["layout", "prototype"]);
  const APP_WORKSPACE_SURFACE_SLUGS = new Set(["app", "layout", "prototype"]);
  const RUNTIME_ROUTE_SLUGS = new Set(["runtime"]);

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

  function appSurfaceSlugFromPathname(pathname = global.location?.pathname) {
    const segments = pathSegments(pathname);
    if (segments[0] !== "apps" || segments.length < 3) return "";
    return String(segments[2] || "").trim().toLowerCase();
  }

  function appRouteSlugFromPathname(pathname = global.location?.pathname) {
    const surface = appSurfaceSlugFromPathname(pathname);
    if (surface) return surface;
    return legacyRouteSlugFromPathname(pathname);
  }

  function isAppSurfaceRoute(pathname = global.location?.pathname) {
    return appSurfaceSlugFromPathname(pathname) === "app";
  }

  function isAppWorkspaceSurfaceRoute(pathname = global.location?.pathname) {
    return APP_WORKSPACE_SURFACE_SLUGS.has(appSurfaceSlugFromPathname(pathname));
  }

  function isWorkspaceSurfaceRoute(pathname = global.location?.pathname) {
    return WORKSPACE_SURFACE_SLUGS.has(appSurfaceSlugFromPathname(pathname));
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

  function isBuildRoute(pathname = global.location?.pathname) {
    if (isWorkspaceSurfaceRoute(pathname)) return true;
    return BUILD_ROUTE_SLUGS.has(legacyRouteSlugFromPathname(pathname));
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

  function isManageRoute(pathname = global.location?.pathname) {
    return isBuildRoute(pathname);
  }

  function shouldMountDrilldownHost(pathname = global.location?.pathname) {
    const slug = appRouteSlugFromPathname(pathname);
    return (
      ACCESS_LIKE_ROUTE_SLUGS.has(slug) ||
      WORKSPACE_SURFACE_SLUGS.has(slug) ||
      BUILD_ROUTE_SLUGS.has(slug) ||
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
      if (isWorkspaceSurfaceRoute(url.pathname)) return true;
      if (!isBuildRoute(url.pathname)) return true;
      return buildTabFromUrl(rawUrl) === "preview";
    } catch (_) {
      return true;
    }
  }

  function isBuildWorkspacePathname(pathname = global.location?.pathname) {
    const path = String(pathname || "");
    return (
      isWorkspaceSurfaceRoute(path) ||
      path.startsWith("/apps/build/") ||
      path.startsWith("/apps/manage/")
    );
  }

  /** `/apps/{app}/layout|prototype|app` → app id; legacy `/apps/build/{app}` → parts[2]. */
  function appIdFromAppsPathname(pathname = global.location?.pathname) {
    const segments = pathSegments(pathname);
    if (segments[0] !== "apps" || segments.length < 2) {
      return "";
    }
    const surface = appSurfaceSlugFromPathname(pathname);
    if (
      WORKSPACE_SURFACE_SLUGS.has(surface) ||
      surface === "app" ||
      ACCESS_LIKE_ROUTE_SLUGS.has(surface)
    ) {
      return String(segments[1] || "").trim();
    }
    if (
      (BUILD_ROUTE_SLUGS.has(segments[1]) || RUNTIME_ROUTE_SLUGS.has(segments[1])) &&
      segments.length >= 3
    ) {
      return String(segments[2] || "").trim();
    }
    if (ACCESS_LIKE_ROUTE_SLUGS.has(segments[1]) && segments.length >= 3) {
      return String(segments[2] || "").trim();
    }
    return String(segments[1] || "").trim();
  }

  function workspaceSurfaceSlugFromAppsPathname(pathname = global.location?.pathname) {
    const surface = appSurfaceSlugFromPathname(pathname);
    return WORKSPACE_SURFACE_SLUGS.has(surface) ? surface : "";
  }

  function sceneIdFromPathname(pathname = global.location?.pathname) {
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
    if (isAppWorkspaceSurfaceRoute(pathname)) return true;
    if (isAccessRoute(pathname)) return true;
    return false;
  }

  const api = {
    ACCESS_LIKE_ROUTE_SLUGS,
    BUILD_ROUTE_SLUGS,
    WORKSPACE_SURFACE_SLUGS,
    APP_WORKSPACE_SURFACE_SLUGS,
    RUNTIME_ROUTE_SLUGS,
    pathSegments,
    legacyRouteSlugFromPathname,
    appSurfaceSlugFromPathname,
    appRouteSlugFromPathname,
    isAppSurfaceRoute,
    isAppWorkspaceSurfaceRoute,
    isWorkspaceSurfaceRoute,
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
  };

  global.MeiRoutePredicates = api;
  global.appIdFromAppsPathname = appIdFromAppsPathname;
  global.workspaceSurfaceSlugFromAppsPathname = workspaceSurfaceSlugFromAppsPathname;
  global.appRouteSlugFromPathname = appRouteSlugFromPathname;
  global.isBuildWorkspacePathname = isBuildWorkspacePathname;
  global.isAppWorkspaceSurfaceRoute = isAppWorkspaceSurfaceRoute;
  global.isWorkspaceSurfaceRoute = isWorkspaceSurfaceRoute;
  global.isAppSurfaceRoute = isAppSurfaceRoute;
  global.isBuildRoute = isBuildRoute;
  global.isAppRoute = isAppRoute;
  global.isAccessRoute = isAccessRoute;
  global.isRevisionFirstShellPage = isRevisionFirstShellPage;
})(typeof window !== "undefined" ? window : globalThis);
