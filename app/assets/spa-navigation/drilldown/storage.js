  function stashSceneProjectionContext(detail, config) {
    try {
      sessionStorage.setItem(
        SCENE_PROJECTION_CONTEXT_KEY,
        JSON.stringify({
          stored_at: Date.now(),
          detail,
          config: {
            sceneId: config.sceneId,
            boardSceneFile: config.boardSceneFile,
            projection: config.projection,
            entry: nonEmptyString(config.popup?.entry, config.popup?.focus),
          },
        }),
      );
    } catch (_) {
      /* ignore */
    }
  }

  function consumeSceneProjectionContext() {
    let raw = "";
    try {
      raw = sessionStorage.getItem(SCENE_PROJECTION_CONTEXT_KEY) || "";
    } catch (_) {
      return null;
    }
    if (!raw) return null;
    try {
      sessionStorage.removeItem(SCENE_PROJECTION_CONTEXT_KEY);
    } catch (_) {
      /* ignore */
    }
    try {
      return JSON.parse(raw);
    } catch (_) {
      return null;
    }
  }

  function resolveBoardRouteUrl(config) {
    const appId = resolvePreviewAppId();
    if (!appId) return "";
    const boardFile = nonEmptyString(config.boardSceneFile);
    const boardSceneId = nonEmptyString(config.boardSceneId);
    if (!boardFile || !boardSceneId) return "";
    let url;
    try {
      url = new URL(window.location.href);
    } catch (_) {
      return "";
    }
    const routeSlug = appRouteSlugFromPathname(url.pathname);
    const accessLike =
      ACCESS_LIKE_ROUTE_SLUGS.has(routeSlug) && !WORKSPACE_SURFACE_SLUGS.has(routeSlug);
    if (accessLike) {
      url.pathname = `/apps/app/${encodeURIComponent(appId)}/scene/${encodeURIComponent(boardSceneId)}`;
      url.searchParams.delete("node");
      url.searchParams.delete("file");
      url.searchParams.delete("scene");
      url.searchParams.delete("tab");
      url.searchParams.delete("mei_projection");
      url.searchParams.delete("mei_entry_tab");
      return url.toString();
    }
    url.pathname = `/apps/${appId}/layout`;
    url.searchParams.set("node", `board-file:${boardFile}#${boardSceneId}`);
    url.searchParams.set("tab", "preview");
    url.searchParams.delete("file");
    url.searchParams.delete("scene");
    url.searchParams.set("mei_projection", "route");
    const entry = nonEmptyString(config.popup?.entry, config.popup?.focus);
    if (entry) {
      url.searchParams.set("mei_entry_tab", entry);
    }
    return url.toString();
  }

  function openBoardRouteProjection(detail, sceneRequest) {
    stashSceneProjectionContext(detail, sceneRequest);
    const targetUrl = resolveBoardRouteUrl(sceneRequest);
    if (!targetUrl) {
      openProjectionOverlay(detail, sceneRequest);
      return;
    }
    void navigateInternal(targetUrl, false);
  }

  function openScene(detail, sceneOpen = null) {
    openSceneProjection(detail, sceneOpen);
  }

  function applySceneProjectionContextFromStorage() {
    if (!shouldMountDrilldownHost()) return;
    const stored = consumeSceneProjectionContext();
    if (!stored?.detail) return;
    const projection = normalizeProjection(
      nonEmptyString(stored.config?.projection, stored.detail?.projection, "route"),
    );
    if (projection !== "route") return;
    const detail = { ...stored.detail };
    const entry = nonEmptyString(stored.config?.entry, detail.popup?.entry, detail.popup?.focus);
    if (entry) {
      detail.popup = {
        ...(detail.popup || {}),
        entry,
        focus: entry,
        entry_tab: entry,
      };
    }
    openSceneProjection(detail, stored.config || null);
  }

