  function dispatchPreviewUpdated(scope = "page", detail = {}) {
    const normalizedScope = String(scope || "page");
    const mergedDetail = {
      scope: normalizedScope,
      ...detail,
    };
    if (
      normalizedScope === "drilldown" &&
      mergedDetail.resetRuntimeQueryCache === undefined
    ) {
      mergedDetail.resetRuntimeQueryCache = false;
    }
    window.dispatchEvent(
      new CustomEvent("meilang:preview-updated", {
        detail: mergedDetail,
      }),
    );
  }

  function dispatchPanelMetricPrefetch() {
    window.dispatchEvent(new CustomEvent(PREFETCH_PANEL_METRICS_EVENT));
  }

  function wakeRuntimeAfterSceneBundleLoaded() {
    requestAnimationFrame(() => {
      if (typeof boot.scheduleFrameViewportRelayout === "function") {
        try {
          boot.scheduleFrameViewportRelayout();
        } catch (_) {}
      }
      requestAnimationFrame(() => {
        requestAnimationFrame(() => {
          dispatchPanelMetricPrefetch();
          dispatchPreviewUpdated("page", {
            resetRuntimeQueryCache: false,
            source: "scene_bundle_ready",
          });
        });
      });
    });
  }
