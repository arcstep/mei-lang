(() => {
  const boot = (window.__meiLangBoot = window.__meiLangBoot || {});
  if (typeof boot.disposeFrameStage === "function") {
    try {
      boot.disposeFrameStage();
    } catch (_) {}
    boot.disposeFrameStage = null;
  }
  const tracked = new WeakMap();
  const observers = new Set();

  function round(value) {
    return Math.round(value * 1000) / 1000;
  }

  function computeScale(mode, hostWidth, hostHeight, designWidth, designHeight) {
    if (hostWidth <= 0 || hostHeight <= 0 || designWidth <= 0 || designHeight <= 0) {
      return 1;
    }
    const sx = hostWidth / designWidth;
    const sy = hostHeight / designHeight;
    const fit = Math.min(sx, sy);
    if (mode === "cover") return Math.max(sx, sy);
    return fit;
  }

  function overflowModeIsDebug(mode) {
    const value = String(mode || "").trim().toLowerCase();
    return value === "debug" || value === "scroll" || value === "visible";
  }

  function showDesignBoundsEnabled(root) {
    const raw = String(root.dataset.showDesignBounds || "").trim().toLowerCase();
    return raw !== "false" && raw !== "0";
  }

  /** max_width 预览：宽随宿主、上限封顶，禁止 transform 缩放。 */
  function applyFluidWidthLayout(shell, stage, contentMaxWidth, hostWidth) {
    const stageWidth = Math.min(contentMaxWidth, hostWidth);
    shell.style.width = `${round(stageWidth)}px`;
    shell.style.height = "auto";
    shell.style.maxHeight = "none";
    shell.style.overflow = "visible";
    stage.style.width = `${round(stageWidth)}px`;
    stage.style.height = "auto";
    stage.style.maxHeight = "none";
    stage.style.transform = "none";
    removeDesignBounds(shell);
  }

  function removeViewportChrome(root) {
    root?.querySelector(".preview-viewport-chrome")?.remove();
  }

  function removeDesignBounds(shell) {
    shell?.querySelector(".preview-design-bounds")?.remove();
    shell?.querySelector(".preview-design-overflow-veil")?.remove();
    removeViewportChrome(shell?.closest('[data-mei-frame-viewport="true"]'));
  }

  function ensureDesignBounds(shell, designWidth, designHeight, scale) {
    let bounds = shell.querySelector(".preview-design-bounds");
    if (!bounds) {
      bounds = document.createElement("div");
      bounds.className = "preview-design-bounds";
      bounds.setAttribute("aria-hidden", "true");
      shell.appendChild(bounds);
    }
    bounds.style.width = `${round(designWidth * scale)}px`;
    bounds.style.height = `${round(designHeight * scale)}px`;
    return bounds;
  }

  /** 分辨率提示放在视口上方安全区内，不压住驾驶舱内容。 */
  function ensureViewportChrome(root, designWidth, designHeight, aspectRatio) {
    let chrome = root.querySelector(".preview-viewport-chrome");
    if (!chrome) {
      chrome = document.createElement("div");
      chrome.className = "preview-viewport-chrome";
      chrome.setAttribute("aria-hidden", "true");
      root.insertBefore(chrome, root.firstChild);
    }
    const aspectSuffix =
      aspectRatio && String(aspectRatio).trim()
        ? ` · ${String(aspectRatio).trim()}`
        : "";
    chrome.textContent = `${Math.round(designWidth)} × ${Math.round(designHeight)}${aspectSuffix}`;
  }

  function ensureOverflowVeil(shell, designWidth, designHeight, scale, contentHeight) {
    if (contentHeight <= designHeight + 1) {
      shell.querySelector(".preview-design-overflow-veil")?.remove();
      return;
    }
    let veil = shell.querySelector(".preview-design-overflow-veil");
    if (!veil) {
      veil = document.createElement("div");
      veil.className = "preview-design-overflow-veil";
      veil.setAttribute("aria-hidden", "true");
      shell.appendChild(veil);
    }
    const top = round(designHeight * scale);
    const left = 0;
    veil.style.top = `${top}px`;
    veil.style.left = `${left}px`;
    veil.style.width = `${round(designWidth * scale)}px`;
    veil.style.height = `${round((contentHeight - designHeight) * scale)}px`;
  }

  /**
   * 编辑调试：contain 等比缩放 + 1920×1080 虚线框；超出设计高度的 panel 仍可滚动查看。
   */
  function applyDebugPreviewLayout(
    root,
    shell,
    stage,
    scaleMode,
    hostWidth,
    hostHeight,
    designWidth,
    designHeight,
    showBounds,
  ) {
    const scale = computeScale(scaleMode, hostWidth, hostHeight, designWidth, designHeight);
    const scaleText = round(scale);

    stage.style.width = `${round(designWidth)}px`;
    stage.style.minHeight = `${round(designHeight)}px`;
    stage.style.height = "auto";
    stage.style.maxHeight = "none";
    stage.style.transformOrigin = "top left";
    stage.style.transform = scaleText < 1 || scaleText > 1 ? `scale(${scaleText})` : "none";

    shell.style.overflow = "visible";
    shell.style.maxHeight = "none";
    shell.style.position = "relative";

    const contentHeight = Math.max(designHeight, stage.scrollHeight, stage.offsetHeight);
    const shellWidth = round(designWidth * scale);
    const shellHeight = round(contentHeight * scale);
    shell.style.width = `${shellWidth}px`;
    shell.style.height = `${shellHeight}px`;

    root.dataset.meiFrameScale = String(scaleText);

    if (showBounds) {
      const aspectRatio = String(root.dataset.aspectRatio || "").trim();
      ensureViewportChrome(root, designWidth, designHeight, aspectRatio);
      ensureDesignBounds(shell, designWidth, designHeight, scale);
      ensureOverflowVeil(shell, designWidth, designHeight, scale, contentHeight);
    } else {
      removeDesignBounds(shell);
    }
  }

  function applyCanvasScaleLayout(
    shell,
    stage,
    scaleMode,
    hostWidth,
    hostHeight,
    designWidth,
    designHeight,
  ) {
    removeDesignBounds(shell);
    const rawScale = computeScale(scaleMode, hostWidth, hostHeight, designWidth, designHeight);
    const scale = scaleMode === "cover" ? rawScale : Math.min(1, rawScale);
    const shellWidth = round(designWidth * scale);
    const shellHeight = round(designHeight * scale);

    shell.style.width = `${shellWidth}px`;
    shell.style.height = `${shellHeight}px`;
    shell.style.overflow = "hidden";
    stage.style.width = `${round(designWidth)}px`;
    stage.style.height = `${round(designHeight)}px`;
    stage.style.minHeight = "";
    stage.style.transformOrigin = "top left";
    stage.style.transform = Math.abs(scale - 1) > 0.001 ? `scale(${round(scale)})` : "none";
  }

  function updateViewport(root) {
    const designWidth = Number(root.dataset.designWidth || 0);
    const designHeight = Number(root.dataset.designHeight || 0);
    const contentMaxWidth = Number(root.dataset.contentMaxWidth || 0);
    const contentHeight = Number(root.dataset.contentHeight || 0) || designHeight;
    const scaleMode = String(root.dataset.scaleMode || "contain").trim().toLowerCase();
    const editScaleMode = String(root.dataset.editScaleMode || "contain").trim().toLowerCase();
    const overflowMode = String(root.dataset.overflowMode || "clip").trim().toLowerCase();
    const safeTop = Number(root.dataset.safeTop || 0);
    const safeRight = Number(root.dataset.safeRight || 0);
    const safeBottom = Number(root.dataset.safeBottom || 0);
    const safeLeft = Number(root.dataset.safeLeft || 0);
    const shell = root.querySelector(".preview-stage-shell");
    const stage = root.querySelector(".preview-stage");
    if (!shell || !stage || !designWidth) return;

    const rect = root.getBoundingClientRect();
    const hostWidth = Math.max(1, rect.width - safeLeft - safeRight);
    const hostHeight = Math.max(1, rect.height - safeTop - safeBottom);

    if (contentMaxWidth > 0) {
      applyFluidWidthLayout(shell, stage, contentMaxWidth, hostWidth);
      return;
    }

    if (overflowModeIsDebug(overflowMode)) {
      applyDebugPreviewLayout(
        root,
        shell,
        stage,
        editScaleMode || "contain",
        hostWidth,
        hostHeight,
        designWidth,
        designHeight || contentHeight,
        showDesignBoundsEnabled(root),
      );
      return;
    }

    if (!designHeight && !contentHeight) return;
    applyCanvasScaleLayout(
      shell,
      stage,
      scaleMode,
      hostWidth,
      hostHeight,
      designWidth,
      designHeight || contentHeight,
    );
  }

  function observeHostResize(root) {
    const host =
      root.closest(".preview-pane-scroll") ||
      root.closest(".main-pane-scroll") ||
      root.parentElement;
    if (!host || host === root) return null;
    const observer = new ResizeObserver(() => updateViewport(root));
    observer.observe(host);
    observers.add(observer);
    return observer;
  }

  function observeViewport(root) {
    if (tracked.has(root)) {
      updateViewport(root);
      return;
    }
    const observersForRoot = [];
    const rootObserver = new ResizeObserver(() => updateViewport(root));
    rootObserver.observe(root);
    observersForRoot.push(rootObserver);
    observers.add(rootObserver);

    const hostObserver = observeHostResize(root);
    if (hostObserver) observersForRoot.push(hostObserver);

    tracked.set(root, observersForRoot);
    updateViewport(root);
  }

  function scan() {
    document
      .querySelectorAll('[data-mei-frame-viewport="true"]')
      .forEach((root) => observeViewport(root));
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
      requestAnimationFrame(() => scan());
    }
  }

  window.addEventListener("resize", scan);
  window.addEventListener("meilang:preview-updated", scan);
  document.addEventListener("mei:manage-tab-change", onManageTabChange);
  boot.disposeFrameStage = () => {
    window.removeEventListener("resize", scan);
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
  };
})();
