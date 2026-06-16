  function initialLoadNavigationId() {
    return typeof boot.INITIAL_LOAD_NAVIGATION_ID === "number"
      ? boot.INITIAL_LOAD_NAVIGATION_ID
      : -1;
  }

  function readBodyPerf() {
    const body = document.body;
    if (!body || !body.dataset) return {};
    return {
      handlerReadyMs: Number(body.dataset.meiHandlerHtmlReadyMs),
      ssrBodyMs: Number(body.dataset.meiSsrHttpResponseBodyMs),
      compileMs: Number(body.dataset.meiCompileMs),
      compileCacheHit: body.dataset.meiCompileCacheHit,
      dataPropsBytes: Number(body.dataset.meiDataPropsBytes),
      dataPropsCount: Number(body.dataset.meiDataPropsCount),
    };
  }

  function buildPerfResponse(perf) {
    return {
      headers: {
        get(name) {
          if (name === "x-mei-compile-ms" && Number.isFinite(perf.compileMs)) {
            return String(perf.compileMs);
          }
          if (name === "x-mei-compile-cache-hit" && perf.compileCacheHit) {
            return perf.compileCacheHit;
          }
          if (name === "x-mei-handler-html-ready-ms" && Number.isFinite(perf.handlerReadyMs)) {
            return String(perf.handlerReadyMs);
          }
          if (name === "x-mei-data-props-bytes" && Number.isFinite(perf.dataPropsBytes)) {
            return String(perf.dataPropsBytes);
          }
          if (name === "x-mei-data-props-count" && Number.isFinite(perf.dataPropsCount)) {
            return String(perf.dataPropsCount);
          }
          return "";
        },
      },
    };
  }

  function shouldTrackInitialLoad(perf) {
    if (
      window.MeiPageLoadProgress &&
      typeof window.MeiPageLoadProgress.isTracking === "function" &&
      window.MeiPageLoadProgress.isTracking()
    ) {
      return false;
    }
    if (document.getElementById("mei-page-load-progress")) return true;
    if (Number.isFinite(perf.compileMs) && perf.compileMs > 0) return true;
    if (Number.isFinite(perf.dataPropsBytes) && perf.dataPropsBytes >= 20 * 1024 * 1024) return true;
    return false;
  }

  async function finishInitialLoadProgress() {
    if (typeof boot.waitForLoadingProgressReady === "function") {
      await boot.waitForLoadingProgressReady(initialLoadNavigationId());
    }
    if (
      window.MeiPageLoadProgress &&
      typeof window.MeiPageLoadProgress.isTracking === "function" &&
      window.MeiPageLoadProgress.isTracking()
    ) {
      return;
    }
    if (typeof hideLoading === "function") {
      hideLoading();
    } else if (window.MeiPageLoadProgress && typeof window.MeiPageLoadProgress.hide === "function") {
      window.MeiPageLoadProgress.hide();
    }
    if (typeof boot.clearLoadingProgressSession === "function") {
      boot.clearLoadingProgressSession(initialLoadNavigationId());
    }
  }

  function bootstrapInitialLoadProgress() {
    if (
      window.MeiPageLoadProgress &&
      typeof window.MeiPageLoadProgress.isTracking === "function" &&
      window.MeiPageLoadProgress.isTracking()
    ) {
      return;
    }
    const perf = readBodyPerf();
    if (!shouldTrackInitialLoad(perf)) return;
    if (
      !document.getElementById("mei-page-load-progress") &&
      window.MeiPageLoadProgress &&
      typeof window.MeiPageLoadProgress.mountFromHandoff === "function"
    ) {
      window.MeiPageLoadProgress.mountFromHandoff();
      return;
    }
    if (typeof showLoadingNow === "function") {
      showLoadingNow();
    }
    if (typeof boot.beginLoadingProgressSession !== "function") return;
    boot.beginLoadingProgressSession(initialLoadNavigationId(), window.location.href);
    if (typeof boot.recordLoadingNavigationResponse === "function") {
      boot.recordLoadingNavigationResponse(
        buildPerfResponse(perf),
        initialLoadNavigationId(),
        0,
      );
    }
    if (typeof boot.markLoadingRenderSwapDone === "function") {
      boot.markLoadingRenderSwapDone(initialLoadNavigationId());
    }
    if (typeof boot.markLoadingPostSpaDone === "function") {
      boot.markLoadingPostSpaDone(initialLoadNavigationId());
    }
    if (typeof boot.refreshLoadingProgressUi === "function") {
      boot.refreshLoadingProgressUi();
    }
    void finishInitialLoadProgress();
  }

  boot.bootstrapInitialLoadProgress = bootstrapInitialLoadProgress;
