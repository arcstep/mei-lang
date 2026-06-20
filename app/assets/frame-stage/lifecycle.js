  function observeHostResize(root) {
    const hosts = [];
    let node = root.parentElement;
    while (node && node !== document.body) {
      hosts.push(node);
      node = node.parentElement;
    }
    if (isChromeNoneAccess()) {
      hosts.push(document.body);
    }
    const seen = new Set();
    const callbacks = hosts
      .filter((host) => {
        if (!host || host === root || seen.has(host)) return false;
        seen.add(host);
        return true;
      })
      .map((host) => {
        const observer = new ResizeObserver(() => queueUpdateViewport(root));
        observer.observe(host);
        observers.add(observer);
        return observer;
      });
    return callbacks;
  }

  function observeViewport(root) {
    if (tracked.has(root)) {
      if (isManagePreviewRoute(root)) invalidateManageLayout(root);
      queueUpdateViewport(root);
      return;
    }
    const observersForRoot = [];
    const manage = isManagePreviewRoute(root);
    const scrollPane = root.closest(".preview-pane-scroll");
    const resizeTarget = manage && scrollPane ? scrollPane : root;
    const rootObserver = new ResizeObserver(() => queueUpdateViewport(root));
    rootObserver.observe(resizeTarget);
    observersForRoot.push(rootObserver);
    observers.add(rootObserver);

    if (!manage) {
      const hostObservers = observeHostResize(root);
      if (hostObservers?.length) observersForRoot.push(...hostObservers);
    }

    tracked.set(root, observersForRoot);
    invalidateManageLayout(root);
    updateViewport(root);
  }

  function scan(event) {
    if (event?.detail?.scope === "drilldown") return;
    if (!shouldMountBuildPreviewRuntime()) return;
    document
      .querySelectorAll('[data-mei-frame-viewport="true"], [data-mei-layout-audit-root="true"]')
      .forEach((root) => observeViewport(root));
    scheduleMetricPrefetch();
  }

  const RUNTIME_QUERY_READY_EVENT = "meilang:runtime-query-ready";
  let runtimeQueryReady = Boolean(
    window.__meiDatasetRuntime &&
      typeof window.__meiDatasetRuntime.prefetchVisiblePanelMetrics === "function",
  );
  let pendingMetricPrefetch = false;

  function markRuntimeQueryReady() {
    runtimeQueryReady = true;
    if (pendingMetricPrefetch) {
      pendingMetricPrefetch = false;
      scheduleMetricPrefetch(0, { force: true });
    }
  }

  let metricPrefetchTimer = null;
  function scheduleMetricPrefetch(delayMs = 0, options = {}) {
    const opts = options || {};
    if (!shouldMountBuildPreviewRuntime()) return;
    if (!opts.force && !runtimeQueryReady) {
      pendingMetricPrefetch = true;
      return;
    }
    if (metricPrefetchTimer != null && !opts.force) {
      return;
    }
    if (metricPrefetchTimer != null) {
      clearTimeout(metricPrefetchTimer);
    }
    metricPrefetchTimer = window.setTimeout(() => {
      metricPrefetchTimer = null;
      if (!opts.force && !runtimeQueryReady) {
        pendingMetricPrefetch = true;
        return;
      }
      if (document.body?.classList?.contains("access-drilldown-open")) {
        return;
      }
      window.dispatchEvent(new CustomEvent("meilang:prefetch-panel-metrics"));
    }, Math.max(0, Number(delayMs) || 0));
  }

  function scheduleViewportRelayout() {
    requestAnimationFrame(() => {
      document
        .querySelectorAll('[data-mei-frame-viewport="true"], [data-mei-layout-audit-root="true"]')
        .forEach((root) => {
          if (isManagePreviewRoute(root)) invalidateManageLayout(root);
          queueUpdateViewport(root);
        });
      scheduleMetricPrefetch(0);
    });
  }

  let domReadyHandler = null;
  if (document.readyState === "loading") {
    domReadyHandler = () => {
      scan();
      scheduleMetricPrefetch(0);
    };
    document.addEventListener("DOMContentLoaded", domReadyHandler, { once: true });
  } else {
    scan();
    scheduleMetricPrefetch(0);
  }

  function onManageTabChange(event) {
    const tab = event?.detail?.tab;
    if (!tab || tab === "preview") {
      scheduleViewportRelayout();
    }
  }

  function onWindowResize() {
    scheduleViewportRelayout();
  }

  function onRuntimeQueryReady() {
    markRuntimeQueryReady();
  }

  window.addEventListener("resize", onWindowResize);
  window.visualViewport?.addEventListener("resize", onWindowResize);
  window.visualViewport?.addEventListener("scroll", onWindowResize);
  window.addEventListener("meilang:preview-updated", scan);
  window.addEventListener(RUNTIME_QUERY_READY_EVENT, onRuntimeQueryReady);
  document.addEventListener("mei:manage-tab-change", onManageTabChange);
  document.addEventListener("click", (event) => {
    const btn = event.target.closest("[data-preview-zoom]");
    if (!btn) return;
    const root = btn.closest('[data-mei-frame-viewport="true"]');
    if (!root || !isManagePreviewRoute(root) || !viewportToolbarEnabled(root)) return;
    const mode = String(btn.dataset.previewZoom || "");
    if (mode === "minus") {
      stepManagePreviewZoom(root, -0.1);
      return;
    }
    if (mode === "plus") {
      stepManagePreviewZoom(root, 0.1);
      return;
    }
    setManagePreviewZoom(root, mode);
  });

  boot.scheduleFrameViewportRelayout = scheduleViewportRelayout;
  boot.disposeFrameStage = () => {
    if (metricPrefetchTimer != null) {
      window.clearTimeout(metricPrefetchTimer);
      metricPrefetchTimer = null;
    }
    window.removeEventListener("resize", onWindowResize);
    window.visualViewport?.removeEventListener("resize", onWindowResize);
    window.visualViewport?.removeEventListener("scroll", onWindowResize);
    window.removeEventListener("meilang:preview-updated", scan);
    window.removeEventListener(RUNTIME_QUERY_READY_EVENT, onRuntimeQueryReady);
    document.removeEventListener("mei:manage-tab-change", onManageTabChange);
    if (domReadyHandler) {
      document.removeEventListener("DOMContentLoaded", domReadyHandler);
      domReadyHandler = null;
    }
    observers.forEach((observer) => {
      try {
        observer.disconnect();
      } catch (_) {}
    });
    observers.clear();
    tracked.clear();
    boot.scheduleFrameViewportRelayout = null;
  };
