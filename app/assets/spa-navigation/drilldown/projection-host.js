  function useSceneBoardOverlay(config) {
    const hostMode = nonEmptyString(
      config?.sceneLocalNav?.hostMode,
      config?.popup?.scene_host_mode,
      config?.popup?.sceneHostMode,
    );
    if (config?.structuredBoard && hostMode === "scene_board") {
      return true;
    }
    return Boolean(
      config?.structuredBoard &&
        config?.sceneShell?.layoutMode === "analytics" &&
        nonEmptyString(config?.boardSceneFile) === "scenes/05-监督预警.board.mei",
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
  }

