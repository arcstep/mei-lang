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

  function scan() {
    document
      .querySelectorAll('[data-mei-frame-viewport="true"]')
      .forEach((root) => observeViewport(root));
  }

  function scheduleViewportRelayout() {
    requestAnimationFrame(() => {
      document
        .querySelectorAll('[data-mei-frame-viewport="true"]')
        .forEach((root) => {
          if (isManagePreviewRoute(root)) invalidateManageLayout(root);
          queueUpdateViewport(root);
        });
    });
  }

  let domReadyHandler = null;
  if (document.readyState === "loading") {
    domReadyHandler = () => scan();
    document.addEventListener("DOMContentLoaded", domReadyHandler, { once: true });
  } else {
    scan();
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

  window.addEventListener("resize", onWindowResize);
  window.visualViewport?.addEventListener("resize", onWindowResize);
  window.visualViewport?.addEventListener("scroll", onWindowResize);
  window.addEventListener("meilang:preview-updated", scan);
  document.addEventListener("mei:manage-tab-change", onManageTabChange);
  document.addEventListener("click", (event) => {
    const btn = event.target.closest("[data-preview-zoom]");
    if (!btn) return;
    const root = btn.closest('[data-mei-frame-viewport="true"]');
    if (!root || !isManagePreviewRoute(root)) return;
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
    window.removeEventListener("resize", onWindowResize);
    window.visualViewport?.removeEventListener("resize", onWindowResize);
    window.visualViewport?.removeEventListener("scroll", onWindowResize);
    window.removeEventListener("meilang:preview-updated", scan);
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
})();
