  function resolveBuildAppId(doc) {
    const root = doc || document;
    const shell = root.querySelector(".shell[data-app-path]");
    if (shell instanceof HTMLElement) {
      const fromShell = String(shell.getAttribute("data-app-path") || "").trim();
      if (fromShell) return fromShell;
    }
    const pathMatch = String(global.location.pathname || "").match(
      /^\/apps\/(?:build|manage)\/([^/]+)/,
    );
    return pathMatch ? pathMatch[1] : "";
  }

  function maybeApplyLayoutTuningOverlay(doc) {
    const overlay = global.MeiOpsLayoutTuningOverlay;
    if (!overlay?.applyHot) return;
    const appId = resolveBuildAppId(doc);
    if (!appId) return;
    void overlay.applyHot(appId, global).catch(() => {});
  }

  function pulseManagePreview(detail, options) {
    if (!shouldRunBuildPreviewRuntimeForUrl(window.location.href)) return;
    const opts = options || {};
    const resetCache = opts.resetRuntimeQueryCache !== false;
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
          maybeApplyLayoutTuningOverlay(document);
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

