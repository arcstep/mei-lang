  function pulseManagePreview(detail, options) {
    const opts = options || {};
    dispatchManageContextChange(detail);
    dispatchPreviewUpdated("page", {
      resetRuntimeQueryCache: opts.resetRuntimeQueryCache !== false,
    });
    requestAnimationFrame(() => {
      dispatchPreviewUpdated("page", {
        resetRuntimeQueryCache: opts.resetRuntimeQueryCache !== false,
      });
      if (typeof boot.scheduleFrameViewportRelayout === "function") {
        try {
          boot.scheduleFrameViewportRelayout();
        } catch (_) {}
      }
    });
  }

  function publishManagePreviewFromDoc(doc, options) {
    const panelRoot =
      document.querySelector("#meilang-author-panel") ||
      (doc && doc.querySelector("#meilang-author-panel"));
    pulseManagePreview(extractManagePanelContext(panelRoot), options);
  }

