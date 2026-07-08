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
    if (
      !resolved.enabled ||
      !(request.sceneId || resolved.pageSceneId || resolved.boardSceneId || resolved.sceneId)
    ) {
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
      pageSceneId: request.sceneId,
      pageSceneFile: request.sceneFile,
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
    const pageSceneId = nonEmptyString(config?.pageSceneId, config?.boardSceneId, config?.sceneId);
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
      label: nonEmptyString(config?.title, pageSceneId, "T2 页面"),
      path: pageSceneId,
      scene: pageSceneId,
      url: canonicalUrl || undefined,
      pathname: canonicalPathname || undefined,
    };
  }

  function projectionOpenDedupeKey(detail, config) {
    const sceneId = nonEmptyString(
      config?.pageSceneId,
      config?.boardSceneId,
      config?.sceneId,
      detail?.page_scene_id,
      detail?.scene_id,
    );
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

  function resolveAppIdFromShell() {
    const shell = document.querySelector("[data-runtime-node][data-app-path], .shell[data-app-path]");
    return shell ? String(shell.getAttribute("data-app-path") || "").trim() : "";
  }

  function resolveClientBootstrapNeighborHops() {
    const raw =
      window.__mei?.runtime?.clientBootstrap?.neighborHops ??
      window.__mei?.clientBootstrapNeighborHops ??
      1;
    const hops = Number(raw);
    return Number.isFinite(hops) && hops > 0 ? Math.floor(hops) : 1;
  }

  async function activateProjectionScope(config) {
    const scope = nonEmptyString(config?.pageSceneId, config?.boardSceneId, config?.sceneId);
    if (!scope || typeof fetch !== "function") {
      return false;
    }
    if (!window.__meiBootstrapPayloadReady && !window.__meiBootstrapSeeded) {
      const ctx =
        typeof boot.parseViewContext === "function"
          ? boot.parseViewContext(window.location.href)
          : null;
      if (typeof boot.ensureBootstrapSeeded === "function" && ctx) {
        await boot.ensureBootstrapSeeded(ctx, {});
      }
      if (!window.__meiBootstrapPayloadReady && !window.__meiBootstrapSeeded) {
        console.warn("[projection-host] eval pack blocked until bootstrap seeded");
        return false;
      }
    }
    const appId = resolveAppIdFromShell();
    const hops = resolveClientBootstrapNeighborHops();
    try {
      const fetchPack =
        typeof boot.fetchEvalPackFromApi === "function"
          ? boot.fetchEvalPackFromApi
          : typeof boot.evalPackLoader?.fetchEvalPackFromApi === "function"
            ? (ctx, opts) => boot.evalPackLoader.fetchEvalPackFromApi(ctx, opts)
            : null;
      let payload = null;
      if (fetchPack) {
        payload = await fetchPack(
          { appId, sceneId: scope },
          { neighborHops: hops, fingerprint: config?.fingerprint || "" },
        );
      } else {
        const params = new URLSearchParams({
          app: appId,
          scene: scope,
          scope,
          pack: "unified",
          neighbor_hops: String(hops),
        });
        const response = await fetch(`/api/host/scene-eval-pack?${params.toString()}`, {
          credentials: "same-origin",
          headers: { Accept: "application/json" },
        });
        if (!response.ok) return false;
        const result = await response.json();
        payload =
          result?.payload && typeof result.payload === "object" ? result.payload : result;
        if (payload && typeof boot.applyBootstrapPayload === "function") {
          boot.applyBootstrapPayload(payload);
        }
      }
      if (typeof boot.dispatchScopeActivation === "function") {
        boot.dispatchScopeActivation({
          scope,
          sceneId: scope,
          appId,
          source: "eval-pack",
          projection: nonEmptyString(config?.projection, "overlay"),
        });
      } else {
        document.dispatchEvent(
          new CustomEvent("meilang:scope-activation", {
            detail: { scope, sceneId: scope, appId, source: "eval-pack" },
          }),
        );
      }
      if (typeof seedFromBootstrap === "function") {
        seedFromBootstrap(window.__mei || payload);
      }
      if (typeof window !== "undefined") {
        window.__meiLastScopeActivation = { scope, sceneId: scope, appId, at: Date.now() };
        window.__meiEvalPackSource = "eval_pack_api";
      }
      return true;
    } catch (_) {
      return false;
    }
  }

  function triggerScopeActivationWarmup(config) {
    void activateProjectionScope(config);
  }

  async function openProjectionOverlay(detail, preResolvedRequest = null) {
    const resolved = preResolvedRequest || resolveSceneOpenRequest(detail);
    const config = resolved;
    if (!config.enabled || !(config.pageSceneId || config.boardSceneId || config.sceneId)) {
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
    await activateProjectionScope(layer2Config);
    const useLayer2 = typeof boot.useUnifiedLayer2 !== "function" || boot.useUnifiedLayer2();
    if (useLayer2 && typeof boot.openLayer2Tab === "function") {
      closeSceneBoardOverlay();
      const root = boot.openLayer2Tab(layer2Config);
      if (typeof boot.beginDrilldownLoadSession === "function") {
        boot.beginDrilldownLoadSession(drilldownSessionMeta(config));
      }
      if (
        root &&
        typeof window.__meiDatasetRuntime?.prefetchVisiblePanelMetrics === "function"
      ) {
        window.__meiDatasetRuntime.prefetchVisiblePanelMetrics(root);
      }
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
      if (typeof window.__meiDatasetRuntime?.prefetchVisiblePanelMetrics === "function") {
        window.__meiDatasetRuntime.prefetchVisiblePanelMetrics(root);
      }
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
      const popup = detail?.popup && typeof detail.popup === "object" ? detail.popup : {};
      const inlineT2Kind = String(detail?.kind || popup?.kind || "").trim();
      if (inlineT2Kind === "t2_panel_open") {
        const panelId = String(
          detail?.page_panel_id ||
            detail?.pagePanelId ||
            popup?.page_panel_id ||
            popup?.pagePanelId ||
            popup?.panel_id ||
            popup?.panelId ||
            "",
        ).trim();
        if (!panelId || typeof boot.openT2Panel !== "function") {
          return;
        }
        boot.openT2Panel(panelId);
        return;
      }
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

