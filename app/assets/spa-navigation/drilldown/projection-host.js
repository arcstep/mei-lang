  function useSceneBoardOverlay(config) {
    const hostMode = nonEmptyString(
      config?.sceneLocalNav?.hostMode,
      config?.popup?.scene_host_mode,
      config?.popup?.sceneHostMode,
    );
    const navKind = nonEmptyString(config?.sceneLocalNav?.kind);
    return Boolean(
      config?.structuredBoard &&
        (hostMode === "scene_page" ||
          hostMode === "scene_board" ||
          navKind === "analytics_drilldown_page" ||
          navKind === "analytics_drilldown_board"),
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
    const boardSceneId = nonEmptyString(config?.boardSceneId, config?.sceneId);
    const canonicalUrl =
      typeof resolveBoardRouteUrl === "function" ? resolveBoardRouteUrl(config) : "";
    let canonicalPathname = "";
    if (canonicalUrl) {
      try {
        canonicalPathname = new URL(canonicalUrl, window.location.href).pathname;
      } catch (_) {
        canonicalPathname = "";
      }
    }
    return {
      label: nonEmptyString(config?.title, boardSceneId, "下钻看板"),
      path: boardSceneId,
      scene: boardSceneId,
      url: canonicalUrl || undefined,
      pathname: canonicalPathname || undefined,
    };
  }

  function projectionOpenDedupeKey(detail, config) {
    const sceneId = nonEmptyString(config?.boardSceneId, config?.sceneId, detail?.scene_id);
    const datasetId = nonEmptyString(detail?.dataset_id, detail?.__mei_runtime_ref?.dataset_id);
    const metricId = nonEmptyString(detail?.metric_id, detail?.__mei_runtime_ref?.metric_id);
    return [sceneId, datasetId, metricId].filter(Boolean).join("|");
  }

  function markProjectionOpenHandled(detail, config) {
    if (!detail || typeof detail !== "object") {
      return false;
    }
    if (detail.__meiProjectionOpenHandled === true) {
      return true;
    }
    const key = projectionOpenDedupeKey(detail, config);
    const now = Date.now();
    const lastKey = boot.__meiLastProjectionOpenKey || "";
    const lastAt = Number(boot.__meiLastProjectionOpenAt || 0);
    if (key && key === lastKey && now - lastAt < 600) {
      return true;
    }
    if (key) {
      boot.__meiLastProjectionOpenKey = key;
      boot.__meiLastProjectionOpenAt = now;
    }
    try {
      Object.defineProperty(detail, "__meiProjectionOpenHandled", {
        value: true,
        configurable: true,
        enumerable: false,
      });
    } catch (_) {
      detail.__meiProjectionOpenHandled = true;
    }
    return false;
  }

  async function prewarmProjectionScope(config) {
    try {
      if (
        (config?.structuredBoard || useSceneBoardOverlay(config)) &&
        typeof prefetchStructuredDrilldownWidgets === "function"
      ) {
        await prefetchStructuredDrilldownWidgets(config);
      }
    } catch (_) {
      /* ignore widget prewarm failures; render path will retry */
    }
    try {
      if (typeof seedFromBootstrap === "function") {
        seedFromBootstrap(window.__mei);
      }
    } catch (_) {
      /* ignore bootstrap seed failures */
    }
  }

  function triggerScopeActivationWarmup(config) {
    const scope = nonEmptyString(config?.boardSceneId, config?.sceneId);
    if (!scope || typeof fetch !== "function") {
      return;
    }
    const shell = document.querySelector("[data-runtime-node][data-app-path], .shell[data-app-path]");
    const appId = shell ? String(shell.getAttribute("data-app-path") || "").trim() : "";
    const appQuery = appId ? `&appId=${encodeURIComponent(appId)}` : "";
    const url = `/api/host/mrg/activate?scope=${encodeURIComponent(scope)}&hops=1${appQuery}`;
    void fetch(url, {
      method: "POST",
      headers: { Accept: "application/json" },
    })
      .then((response) => (response.ok ? response.json() : null))
      .then((result) => {
        const payload =
          result?.payload && typeof result.payload === "object" ? result.payload : null;
        if (payload && typeof seedFromBootstrap === "function") {
          seedFromBootstrap(payload);
        }
      })
      .catch(() => {
        /* ignore activation warmup failures; runtime API path remains fallback */
      });
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
    const popup = config?.popup && typeof config.popup === "object" ? config.popup : {};
    const overlayWorkspace =
      typeof boot.resolveOverlayWorkspace === "function"
        ? boot.resolveOverlayWorkspace(popup, detail)
        : popup.overlay_workspace && typeof popup.overlay_workspace === "object"
          ? popup.overlay_workspace
          : null;
    const layer2Config = {
      ...config,
      overlayWorkspace,
      overlaySize: nonEmptyString(
        config.overlaySize,
        overlayWorkspace?.size,
        popup?.overlay_size,
        popup?.overlaySize,
        "large",
      ),
    };
    await prewarmProjectionScope(layer2Config);
    const useLayer2 = typeof boot.useUnifiedLayer2 !== "function" || boot.useUnifiedLayer2();
    if (useLayer2 && typeof boot.openLayer2Tab === "function") {
      closeSceneBoardOverlay();
      const root = boot.openLayer2Tab(layer2Config);
      if (typeof boot.beginDrilldownLoadSession === "function") {
        boot.beginDrilldownLoadSession(drilldownSessionMeta(config));
      }
      triggerScopeActivationWarmup(layer2Config);
      if (config.boardFrameScene) {
        await renderFrameBoardSceneContent(root, detail, config);
        return;
      }
      if (config.structuredBoard || useSceneBoardOverlay(config)) {
        await renderStructuredDrilldownContent(root, detail, config);
        return;
      }
      const activeTab = renderDrilldownTabs(root, detail, config);
      if (!renderDrilldownContent(root, detail, config, activeTab)) {
        return;
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
      if (typeof boot.beginDrilldownLoadSession === "function") {
        boot.beginDrilldownLoadSession(drilldownSessionMeta(config));
      }
      triggerScopeActivationWarmup(layer2Config);
      await renderStructuredDrilldownContent(root, detail, config);
      return;
    }
    closeSceneBoardOverlay();
    const root = ensureDrilldownOverlayRoot();
    applyDrilldownOverlayMeta(root, config);
    root.removeAttribute("hidden");
    root.classList.add("is-open");
    document.body.classList.add("access-drilldown-open");
    if (typeof boot.beginDrilldownLoadSession === "function") {
      boot.beginDrilldownLoadSession(drilldownSessionMeta(config));
    }
    triggerScopeActivationWarmup(layer2Config);
    if (config.boardFrameScene) {
      if (!(await renderFrameBoardSceneContent(root, detail, config))) {
        return;
      }
      return;
    }
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
    if (typeof installOverlayCloseDelegation === "function") {
      installOverlayCloseDelegation();
    }
    const openByEvent = async (event) => {
      if (!shouldMountDrilldownHost()) return;
      if (typeof isBuildRoute === "function" && isBuildRoute()) return;
      const detail = event?.detail || {};
      const config = resolveSceneOpenRequest(detail);
      if (markProjectionOpenHandled(detail, config)) return;
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
    document.addEventListener(SCENE_OPEN_EVENT, openByEvent);
    document.addEventListener("keydown", (event) => {
      if (event.key === "Escape") {
        if (typeof boot.useUnifiedLayer2 === "function" && boot.useUnifiedLayer2()) {
          if (typeof boot.closeLayer2Tab === "function" && boot.closeLayer2Tab()) {
            return;
          }
          if (typeof boot.closeLayer2Stack === "function") {
            boot.closeLayer2Stack();
          }
          return;
        }
        closeDrilldownOverlay();
        closeSceneBoardOverlay();
      }
    });
    boot.openSceneProjection = openSceneProjection;
    const root = typeof globalThis !== "undefined" ? globalThis : window;
    root.MeiDrilldown = root.MeiDrilldown || {};
    root.MeiDrilldown.openProjectionPreview = function openProjectionPreview(options) {
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

