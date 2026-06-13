  function buildSceneOpenRequest(config, detail = {}) {
    const popup = config?.popup && typeof config.popup === "object" ? config.popup : {};
    return {
      sceneId: nonEmptyString(config.boardSceneId, config.sceneId, popup?.scene_id, popup?.sceneId),
      sceneFile: nonEmptyString(config.boardSceneFile, popup?.scene_file, popup?.sceneFile),
      params: config.params || normalizeSceneParams(popup?.params),
      entry: nonEmptyString(popup?.entry, popup?.focus, popup?.entry_tab, popup?.entryTab),
      sceneAssembly: config.sceneShell || null,
      bindings: config.tabMetrics || {},
      hostContext: {
        hostSceneId: nonEmptyString(config.hostSceneId, detail?.scene_id),
        hostSceneFile: nonEmptyString(config.hostSceneFile, detail?.host_scene_file, detail?.scene_path),
        queryStateId: nonEmptyString(config.queryStateId, detail?.query_state_id, detail?.queryStateId),
        metricId: nonEmptyString(detail?.metric_id, detail?.__mei_runtime_ref?.metric_id),
      },
      projectionSlots: normalizeProjectionSlots(config.projectionSlots || popup?.projection_slots || popup?.projectionSlots),
      filterSchema: config.filterSchema || normalizeAnalyticsFilterSchema(popup?.filter_schema || popup?.filterSchema),
      layoutMode: nonEmptyString(config.sceneShell?.layoutMode, config.layoutMode, popup?.layout_mode),
    };
  }

  function buildProjectionMount(config, detail = {}) {
    const popup = config?.popup && typeof config.popup === "object" ? config.popup : {};
    return {
      mode: normalizeProjection(nonEmptyString(config.projection, popup?.projection, detail?.projection, "overlay")),
      title: nonEmptyString(config.title, popup?.title, detail?.label, "指标明细"),
      overlaySize: nonEmptyString(config.overlaySize, popup?.overlay_size, popup?.overlaySize, "large"),
      restoreContext: {
        hostSceneId: nonEmptyString(config.hostSceneId, detail?.scene_id),
        hostSceneFile: nonEmptyString(config.hostSceneFile, detail?.host_scene_file, detail?.scene_path),
      },
    };
  }

  function resolveSceneOpenRequest(detail) {
    const config = resolveLegacySceneProjectionConfig(detail);
    return {
      ...config,
      request: buildSceneOpenRequest(config, detail),
      mount: buildProjectionMount(config, detail),
    };
  }

  function resolveAppPathByPrefixes(pathname, prefixes) {
    const raw = String(pathname || "");
    if (!raw || !Array.isArray(prefixes)) return "";
    for (const prefix of prefixes) {
      const normalizedPrefix = String(prefix || "");
      if (!normalizedPrefix || !raw.startsWith(normalizedPrefix)) continue;
      const tail = raw.slice(normalizedPrefix.length);
      const slash = tail.indexOf("/");
      const app = slash >= 0 ? tail.slice(0, slash) : tail;
      const trimmed = String(app || "").trim();
      if (trimmed) return trimmed;
    }
    return "";
  }

  function resolveAccessAppPath(pathname = window.location.pathname) {
    return resolveAppPathByPrefixes(pathname, appRoutePrefixesFromSlugs(ACCESS_LIKE_ROUTE_SLUGS));
  }

  function resolvePreviewAppId(pathname = window.location.pathname) {
    const slug = appRouteSlugFromPathname(pathname);
    if (ACCESS_LIKE_ROUTE_SLUGS.has(slug) || BUILD_ROUTE_SLUGS.has(slug)) {
      return resolveAppPathByPrefixes(pathname, [`/apps/${slug}/`]);
    }
    return resolveAppPathByPrefixes(pathname, ["/apps/upload/", "/apps/config/"]);
  }

