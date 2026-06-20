  function useSceneBoardOverlay(config) {
    const hostMode = nonEmptyString(
      config?.sceneLocalNav?.hostMode,
      config?.popup?.scene_host_mode,
      config?.popup?.sceneHostMode,
    );
    const navKind = nonEmptyString(config?.sceneLocalNav?.kind);
    return Boolean(
      config?.structuredBoard &&
        (hostMode === "scene_board" || navKind === "analytics_drilldown_board"),
    );
  }

  async function openSceneProjection(detail, preResolvedRequest = null) {
    const resolved = preResolvedRequest || resolveSceneOpenRequest(detail);
    const request = resolved.request || buildSceneOpenRequest(resolved, detail);
    const mount = resolved.mount || buildProjectionMount(resolved, detail);
    if (!resolved.enabled || !(request.sceneId || resolved.boardSceneId || resolved.sceneId)) {
      if (resolved.errorMessage) {
        recordPopupDebugIssue({
          phase: resolved.errorCode || "scene_projection",
          message: resolved.errorMessage,
          detail,
          config: resolved,
          datasetId: nonEmptyString(detail?.dataset_id, detail?.__mei_runtime_ref?.dataset_id),
          metricId: nonEmptyString(detail?.metric_id, detail?.__mei_runtime_ref?.metric_id),
        });
      }
      return;
    }
    const renderConfig = {
      ...resolved,
      projection: mount.mode,
      title: mount.title,
      overlaySize: mount.overlaySize,
      params: request.params,
      boardSceneId: request.sceneId,
      boardSceneFile: request.sceneFile,
    };
    if (mount.mode === "route") {
      openBoardRouteProjection(detail, renderConfig);
      return;
    }
    await openProjectionOverlay(detail, renderConfig);
  }

  function drilldownSessionMeta(config) {
    return {
      label: nonEmptyString(config?.title, config?.boardSceneId, config?.sceneId, "下钻看板"),
      path: nonEmptyString(config?.boardSceneFile, config?.boardSceneId, config?.sceneId),
    };
  }

  async function openProjectionOverlay(detail, preResolvedRequest = null) {
    const resolved = preResolvedRequest || resolveSceneOpenRequest(detail);
    const config = resolved;
    if (!config.enabled || !(config.boardSceneId || config.sceneId)) {
      if (config.errorMessage) {
        recordPopupDebugIssue({
          phase: config.errorCode || "scene_projection",
          message: config.errorMessage,
          detail,
          config,
          datasetId: nonEmptyString(detail?.dataset_id, detail?.__mei_runtime_ref?.dataset_id),
          metricId: nonEmptyString(detail?.metric_id, detail?.__mei_runtime_ref?.metric_id),
        });
      }
      return;
    }
    if (typeof boot.beginDrilldownLoadSession === "function") {
      boot.beginDrilldownLoadSession(drilldownSessionMeta(config));
    }
    if (useSceneBoardOverlay(config)) {
      closeDrilldownOverlay();
      const root = ensureSceneBoardOverlayRoot();
      applyDrilldownOverlayMeta(root, config);
      root.removeAttribute("hidden");
      root.classList.add("is-open");
      document.body.classList.add("access-scene-board-open");
      await renderStructuredDrilldownContent(root, detail, config);
      return;
    }
    closeSceneBoardOverlay();
    const root = ensureDrilldownOverlayRoot();
    applyDrilldownOverlayMeta(root, config);
    root.removeAttribute("hidden");
    root.classList.add("is-open");
    document.body.classList.add("access-drilldown-open");
    if (config.structuredBoard) {
      await renderStructuredDrilldownContent(root, detail, config);
      return;
    }
    const activeTab = renderDrilldownTabs(root, detail, config);
    if (!renderDrilldownContent(root, detail, config, activeTab)) {
      return;
    }
  }

  function installSceneProjectionHost() {
    if (window.self !== window.top) return;
    if (!shouldMountDrilldownHost()) return;
    if (boot.metricDrilldownHostMounted) return;
    boot.metricDrilldownHostMounted = true;
    boot.sceneProjectionHostMounted = true;
    const openByEvent = async (event) => {
      if (!shouldMountDrilldownHost()) return;
      if (typeof isBuildRoute === "function" && isBuildRoute()) return;
      const detail = event?.detail || {};
      const config = resolveSceneOpenRequest(detail);
      if (!config.enabled || !(config.boardSceneId || config.sceneId)) {
        if (config.errorMessage) {
          recordPopupDebugIssue({
            phase: config.errorCode || "scene_projection",
            message: config.errorMessage,
            detail,
            config,
            datasetId: nonEmptyString(detail?.dataset_id, detail?.__mei_runtime_ref?.dataset_id),
            metricId: nonEmptyString(detail?.metric_id, detail?.__mei_runtime_ref?.metric_id),
          });
        }
        return;
      }
      await openSceneProjection(detail, config);
    };
    document.addEventListener(METRIC_DRILLDOWN_EVENT, openByEvent);
    document.addEventListener(ANALYSIS_OPEN_EVENT, openByEvent);
    document.addEventListener(POPUP_OPEN_EVENT, openByEvent);
    document.addEventListener("keydown", (event) => {
      if (event.key === "Escape") {
        closeDrilldownOverlay();
        closeSceneBoardOverlay();
      }
    });
    boot.openSceneProjection = openSceneProjection;
    global.MeiDrilldown = global.MeiDrilldown || {};
    global.MeiDrilldown.openProjectionPreview = function openProjectionPreview(options) {
      const sceneId = nonEmptyString(options?.sceneId);
      const projectionId = nonEmptyString(options?.projectionId);
      const assembly = options?.assembly && typeof options.assembly === "object" ? options.assembly : {};
      if (!sceneId || !projectionId) return Promise.resolve();
      const popup =
        (assembly.overlays && assembly.overlays[projectionId]) ||
        (assembly.boards && assembly.boards[projectionId]) ||
        assembly[projectionId] ||
        {};
      return openSceneProjection({
        scene_id: sceneId,
        projection_id: projectionId,
        popup,
        __mei_build_isolated: Boolean(options?.isolated),
      });
    };
  }

