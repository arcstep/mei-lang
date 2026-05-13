(() => {
  const tracked = new WeakMap();

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

  function updateViewport(root) {
    const designWidth = Number(root.dataset.designWidth || 0);
    const designHeight = Number(root.dataset.designHeight || 0);
    const scaleMode = String(root.dataset.scaleMode || "contain").trim().toLowerCase();
    const safeTop = Number(root.dataset.safeTop || 0);
    const safeRight = Number(root.dataset.safeRight || 0);
    const safeBottom = Number(root.dataset.safeBottom || 0);
    const safeLeft = Number(root.dataset.safeLeft || 0);
    const shell = root.querySelector(".preview-stage-shell");
    const stage = root.querySelector(".preview-stage");
    if (!shell || !stage || !designWidth || !designHeight) return;

    const rect = root.getBoundingClientRect();
    const hostWidth = Math.max(1, rect.width - safeLeft - safeRight);
    const hostHeight = Math.max(1, rect.height - safeTop - safeBottom);
    const scale = computeScale(scaleMode, hostWidth, hostHeight, designWidth, designHeight);
    const shellWidth = round(designWidth * scale);
    const shellHeight = round(designHeight * scale);

    shell.style.width = `${shellWidth}px`;
    shell.style.height = `${shellHeight}px`;
    stage.style.width = `${designWidth}px`;
    stage.style.height = `${designHeight}px`;
    stage.style.transform = `scale(${round(scale)})`;
  }

  function observeViewport(root) {
    if (tracked.has(root)) {
      updateViewport(root);
      return;
    }
    const observer = new ResizeObserver(() => updateViewport(root));
    observer.observe(root);
    tracked.set(root, observer);
    updateViewport(root);
  }

  function scan() {
    document
      .querySelectorAll('[data-mei-frame-viewport="true"]')
      .forEach((root) => observeViewport(root));
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", scan, { once: true });
  } else {
    scan();
  }
  window.addEventListener("resize", scan);
  window.addEventListener("meilang:preview-updated", scan);
})();
