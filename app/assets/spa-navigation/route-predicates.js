  // Keep in sync with `UiRouteMode::from_slug` (app/src/ui/route.rs).
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
  const RUNTIME_ROUTE_SLUGS = new Set(["runtime"]);

  function pathSegments(pathname = window.location.pathname) {
    return String(pathname || "")
      .split("/")
      .filter((part) => part.trim().length > 0);
  }

  function legacyRouteSlugFromPathname(pathname = window.location.pathname) {
    const segments = pathSegments(pathname);
    if (segments[0] !== "apps" || segments.length < 2) return "";
    return String(segments[1] || "").trim().toLowerCase();
  }

  function appSurfaceSlugFromPathname(pathname = window.location.pathname) {
    const segments = pathSegments(pathname);
    if (segments[0] !== "apps" || segments.length < 3) return "";
    return String(segments[2] || "").trim().toLowerCase();
  }

  function appRouteSlugFromPathname(pathname = window.location.pathname) {
    const surface = appSurfaceSlugFromPathname(pathname);
    if (surface) return surface;
    return legacyRouteSlugFromPathname(pathname);
  }

  function isAppSurfaceRoute(pathname = window.location.pathname) {
    return appSurfaceSlugFromPathname(pathname) === "app";
  }

  function isWorkspaceSurfaceRoute(pathname = window.location.pathname) {
    return WORKSPACE_SURFACE_SLUGS.has(appSurfaceSlugFromPathname(pathname));
  }

  function appRoutePrefixesFromSlugs(slugs) {
    return Array.from(slugs, (slug) => `/apps/${slug}/`);
  }

  function isAppRoute(pathname = window.location.pathname) {
    if (isAppSurfaceRoute(pathname)) return true;
    return ACCESS_LIKE_ROUTE_SLUGS.has(legacyRouteSlugFromPathname(pathname));
  }

  function isRuntimeRoute(pathname = window.location.pathname) {
    const path = String(pathname || "");
    if (path === "/runtime" || path.startsWith("/runtime?")) return true;
    return RUNTIME_ROUTE_SLUGS.has(legacyRouteSlugFromPathname(pathname));
  }

  function isBuildRoute(pathname = window.location.pathname) {
    if (isWorkspaceSurfaceRoute(pathname)) return true;
    return BUILD_ROUTE_SLUGS.has(legacyRouteSlugFromPathname(pathname));
  }

  function isConfigRoute(pathname = window.location.pathname) {
    const path = String(pathname || "");
    return path === "/config" || path.startsWith("/config?") || path.startsWith("/apps/config/");
  }

  function isUploadRoute(pathname = window.location.pathname) {
    const path = String(pathname || "");
    return path === "/upload" || path.startsWith("/upload?") || path.startsWith("/apps/upload/");
  }

  function isStandaloneViewRoute(pathname = window.location.pathname) {
    return isConfigRoute(pathname) || isUploadRoute(pathname);
  }

  function isAccessRoute(pathname = window.location.pathname) {
    return isAppRoute(pathname);
  }

  function isManageRoute(pathname = window.location.pathname) {
    return isBuildRoute(pathname);
  }

  /** Preview-capable host routes: access-like scene shells + build/manage editors. */
  function shouldMountDrilldownHost(pathname = window.location.pathname) {
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
      return String(new URL(rawUrl, window.location.href).searchParams.get("tab") || "overview")
        .trim()
        .toLowerCase();
    } catch (_) {
      return "overview";
    }
  }

  function shouldRunBuildPreviewRuntimeForUrl(rawUrl) {
    try {
      const pathname = new URL(rawUrl, window.location.href).pathname;
      if (!isBuildRoute(pathname)) return true;
      return buildTabFromUrl(rawUrl) === "preview";
    } catch (_) {
      return true;
    }
  }

  function isBuildWorkspacePathname(pathname = window.location.pathname) {
    const path = String(pathname || "");
    return (
      isWorkspaceSurfaceRoute(path) ||
      path.startsWith("/apps/build/") ||
      path.startsWith("/apps/manage/")
    );
  }

