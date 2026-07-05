  const GLOBAL_LAYER2_DEFAULT = {
    host: "layer2",
    tab_policy: "append",
    layout: "single",
    close: "tab_then_stack",
  };

  function readOverlayDefaults() {
    const mei = typeof window !== "undefined" ? window.__mei : null;
    if (mei && mei.overlay_defaults && typeof mei.overlay_defaults === "object") {
      return mei.overlay_defaults;
    }
    return {};
  }

  function resolveOverlayWorkspace(popup, detail) {
    const popupObj = popup && typeof popup === "object" && !Array.isArray(popup) ? popup : {};
    if (popupObj.overlay_workspace && typeof popupObj.overlay_workspace === "object") {
      return { ...popupObj.overlay_workspace };
    }
    const linkKey = nonEmptyString(
      popupObj.link_key,
      popupObj.linkKey,
      detail?.link_key,
      detail?.linkKey,
      detail?.projection_id,
      detail?.projectionId,
    );
    if (linkKey) {
      const defaults = readOverlayDefaults();
      const workspace = defaults[linkKey];
      if (workspace && typeof workspace === "object") {
        return { ...workspace };
      }
    }
    const overlaySize = nonEmptyString(popupObj.overlay_size, popupObj.overlaySize, "large");
    return { ...GLOBAL_LAYER2_DEFAULT, size: overlaySize };
  }

  boot.resolveOverlayWorkspace = resolveOverlayWorkspace;
  boot.readOverlayDefaults = readOverlayDefaults;
  boot.GLOBAL_LAYER2_DEFAULT = GLOBAL_LAYER2_DEFAULT;

  function buildSceneOpenRequest(config, detail = {}) {
    const popup = config?.popup && typeof config.popup === "object" ? config.popup : {};
    const metricId = nonEmptyString(detail?.metric_id, detail?.__mei_runtime_ref?.metric_id);
    const datasetId = nonEmptyString(detail?.dataset_id, detail?.__mei_runtime_ref?.dataset_id);
    return {
      sceneId: nonEmptyString(
        config.pageSceneId,
        config.boardSceneId,
        config.sceneId,
        popup?.page_scene_id,
        popup?.scene_id,
        popup?.sceneId,
      ),
      sceneFile: nonEmptyString(
        config.pageSceneFile,
        config.boardSceneFile,
        popup?.page_scene_file,
        popup?.scene_file,
        popup?.sceneFile,
      ),
      params: config.params || normalizeSceneParams(popup?.params),
      entry: nonEmptyString(popup?.entry, popup?.focus, popup?.entry_tab, popup?.entryTab),
      sceneAssembly: config.sceneShell || null,
      bindings: config.tabMetrics || {},
      hostContext: {
        hostSceneId: nonEmptyString(config.hostSceneId, detail?.scene_id),
        hostSceneFile: nonEmptyString(config.hostSceneFile, detail?.host_scene_file, detail?.scene_path),
        queryStateId: nonEmptyString(config.queryStateId, detail?.query_state_id, detail?.queryStateId),
        sceneOpenKind: nonEmptyString(detail?.kind, popup?.kind, "scene_open"),
        metricId,
        datasetId,
        metricContext: metricId
          ? {
              metricId,
              datasetId,
              analysisContract:
                detail?.analysis_contract && typeof detail.analysis_contract === "object"
                  ? detail.analysis_contract
                  : null,
            }
          : null,
      },
      projectionSlots: normalizeProjectionSlots(config.projectionSlots || popup?.projection_slots || popup?.projectionSlots),
      filterSchema: config.filterSchema || normalizeAnalyticsFilterSchema(popup?.filter_schema || popup?.filterSchema),
      layoutMode: nonEmptyString(config.sceneShell?.layoutMode, config.layoutMode, popup?.layout_mode),
    };
  }

  function buildProjectionMount(config, detail = {}) {
    const popup = config?.popup && typeof config.popup === "object" ? config.popup : {};
    const mapping = resolveListPreviewMapping(config);
    const hideTitle = isPreviewOnlyMapping(config) && !mappingShowsHeader(mapping);
    return {
      mode: normalizeProjection(nonEmptyString(config.projection, popup?.projection, detail?.projection, "overlay")),
      title: hideTitle
        ? ""
        : nonEmptyString(
            config.title,
            popup?.title,
            detail?.label,
            popup?.kind === "scene_open" && !nonEmptyString(detail?.metric_id) ? "看板" : "指标明细",
          ),
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
    return resolveAppPathByPrefixes(pathname, ["/upload", "/upload?", "/apps/upload/", "/config", "/config?", "/apps/config/"]);
  }

