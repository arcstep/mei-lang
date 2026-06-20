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

