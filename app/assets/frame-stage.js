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

  /** max_width 预览：宽随宿主、上限封顶，禁止 transform 缩放（避免宽屏放大、窄屏缩小）。 */
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
    const scale = Math.min(
      1,
      computeScale(scaleMode, hostWidth, hostHeight, designWidth, designHeight),
    );
    const shellWidth = round(designWidth * scale);
    const shellHeight = round(designHeight * scale);

    shell.style.width = `${shellWidth}px`;
    shell.style.height = `${shellHeight}px`;
    shell.style.overflow = "hidden";
    stage.style.width = `${designWidth}px`;
    stage.style.height = `${designHeight}px`;
    stage.style.transform = scale < 1 ? `scale(${round(scale)})` : "none";
  }

  function updateViewport(root) {
    const designWidth = Number(root.dataset.designWidth || 0);
    const designHeight = Number(root.dataset.designHeight || 0);
    const contentMaxWidth = Number(root.dataset.contentMaxWidth || 0);
    const contentHeight = Number(root.dataset.contentHeight || 0) || designHeight;
    const scaleMode = String(root.dataset.scaleMode || "contain").trim().toLowerCase();
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

  function observeViewport(root) {
    if (tracked.has(root)) {
      updateViewport(root);
      return;
    }
    const observer = new ResizeObserver(() => updateViewport(root));
    observer.observe(root);
    tracked.set(root, observer);
    observers.add(observer);
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
  window.addEventListener("resize", scan);
  window.addEventListener("meilang:preview-updated", scan);
  boot.disposeFrameStage = () => {
    window.removeEventListener("resize", scan);
    window.removeEventListener("meilang:preview-updated", scan);
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
