  // Keep in sync with `UiRouteMode::from_slug` (app/src/ui/route.rs).
  const ACCESS_LIKE_ROUTE_SLUGS = new Set([
    "app",
    "access",
    "run",
    "access-only",
    "access_only",
    "presentation",
    "slides",
  ]);
  const BUILD_ROUTE_SLUGS = new Set(["build", "manage"]);

  function appRouteSlugFromPathname(pathname = window.location.pathname) {
    const path = String(pathname || "");
    const match = path.match(/^\/apps\/([^/]+)\//);
    return match ? String(match[1] || "").trim().toLowerCase() : "";
  }

  function appRoutePrefixesFromSlugs(slugs) {
    return Array.from(slugs, (slug) => `/apps/${slug}/`);
  }

  function isAppRoute(pathname = window.location.pathname) {
    return ACCESS_LIKE_ROUTE_SLUGS.has(appRouteSlugFromPathname(pathname));
  }

  function isBuildRoute(pathname = window.location.pathname) {
    return BUILD_ROUTE_SLUGS.has(appRouteSlugFromPathname(pathname));
  }

  function isConfigRoute(pathname = window.location.pathname) {
    return String(pathname || "").startsWith("/apps/config/");
  }

  function isUploadRoute(pathname = window.location.pathname) {
    return String(pathname || "").startsWith("/apps/upload/");
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
    return ACCESS_LIKE_ROUTE_SLUGS.has(slug) || BUILD_ROUTE_SLUGS.has(slug);
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
    return (
      String(pathname || "").startsWith("/apps/build/") ||
      String(pathname || "").startsWith("/apps/manage/")
    );
  }

