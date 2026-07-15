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

