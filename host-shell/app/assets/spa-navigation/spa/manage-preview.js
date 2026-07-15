  function resolveBuildAppId(doc) {
    const root = doc || document;
    const shell = root.querySelector(".shell[data-app-path]");
    if (shell instanceof HTMLElement) {
      const fromShell = String(shell.getAttribute("data-app-path") || "").trim();
      if (fromShell) return fromShell;
    }
    if (typeof appIdFromAppsPathname === "function") {
      const fromApps = String(appIdFromAppsPathname(global.location.pathname) || "").trim();
      if (fromApps) return fromApps;
    }
    const pathMatch = String(global.location.pathname || "").match(
      /^\/apps\/(?:build|manage)\/([^/]+)/,
    );
    return pathMatch ? pathMatch[1] : "";
  }

  function maybeApplyThemeLayoutOverlay(doc) {
    const overlay = global.MeiOpsThemeLayoutOverlay || boot.MeiOpsThemeLayoutOverlay;
    if (!overlay?.applyHot) return;
    const appId = resolveBuildAppId(doc);
    if (!appId) return;
    void overlay.applyHot(appId, global).catch(() => {});
  }

  function pulseManagePreview(detail, options) {
    const onWorkspaceSurface =
      (typeof isWorkspaceSurfaceUrl === "function" &&
        isWorkspaceSurfaceUrl(window.location.href)) ||
      shouldRunBuildPreviewRuntimeForUrl(window.location.href);
    if (!onWorkspaceSurface) return;
    const opts = options || {};
    const resetCache = opts.resetRuntimeQueryCache === true;
    dispatchManageContextChange(detail);
    requestAnimationFrame(() => {
      if (typeof boot.scheduleFrameViewportRelayout === "function") {
        try {
          boot.scheduleFrameViewportRelayout();
        } catch (_) {}
      }
      requestAnimationFrame(() => {
        requestAnimationFrame(() => {
          if (!resetCache) {
            dispatchPanelMetricPrefetch();
          }
          dispatchPreviewUpdated("page", {
            resetRuntimeQueryCache: resetCache,
          });
          if (resetCache) {
            dispatchPanelMetricPrefetch();
          }
          if (typeof boot.mountManagePreviewBoard === "function") {
            void boot.mountManagePreviewBoard(document);
          }
          maybeApplyThemeLayoutOverlay(document);
        });
      });
    });
  }

  function publishManagePreviewFromDoc(doc, options) {
    const panelRoot =
      document.querySelector("#meilang-author-panel") ||
      (doc && doc.querySelector("#meilang-author-panel"));
    pulseManagePreview(extractManagePanelContext(panelRoot), options);
  }

