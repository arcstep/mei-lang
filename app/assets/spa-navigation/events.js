  function dispatchPreviewUpdated(scope = "page", detail = {}) {
    window.dispatchEvent(
      new CustomEvent("meilang:preview-updated", {
        detail: {
          scope: String(scope || "page"),
          ...detail,
        },
      }),
    );
  }

  function dispatchPanelMetricPrefetch() {
    window.dispatchEvent(new CustomEvent(PREFETCH_PANEL_METRICS_EVENT));
  }

  function wakeRuntimeAfterSceneBundleLoaded() {
    dispatchPreviewUpdated("page", {
      resetRuntimeQueryCache: false,
      source: "scene_bundle_ready",
    });
    requestAnimationFrame(() => {
      dispatchPanelMetricPrefetch();
    });
  }
