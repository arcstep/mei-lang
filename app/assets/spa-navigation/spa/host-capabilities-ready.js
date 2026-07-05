/**
 * Wait until shared host runtime APIs are available (access + manage bundles).
 */
(function initHostCapabilitiesReady(global) {
  "use strict";

  const boot = (global.__meiLangBoot = global.__meiLangBoot || {});

  const REQUIRED = [
    () => typeof boot.parseViewContext === "function",
    () => typeof boot.renderStructureTree === "function",
    () => typeof boot.viewCompositor?.composeFromLayers === "function",
    () => typeof boot.viewRevisionClient?.negotiateWithLocalMiss === "function",
  ];

  function capabilitiesReady() {
    return REQUIRED.every((check) => {
      try {
        return check();
      } catch (_) {
        return false;
      }
    });
  }

  function hostCapabilitiesReady(options) {
    const opts = options || {};
    const timeoutMs = Number.isFinite(opts.timeoutMs) ? opts.timeoutMs : 5000;
    if (capabilitiesReady()) {
      return Promise.resolve(true);
    }
    return new Promise((resolve, reject) => {
      const started = Date.now();
      const timer = setInterval(() => {
        if (capabilitiesReady()) {
          clearInterval(timer);
          resolve(true);
          return;
        }
        if (Date.now() - started >= timeoutMs) {
          clearInterval(timer);
          reject(new Error("host capabilities not ready"));
        }
      }, 16);
    });
  }

  boot.hostCapabilitiesReady = hostCapabilitiesReady;
  boot.hostCapabilitiesReadySync = capabilitiesReady;
})(typeof window !== "undefined" ? window : globalThis);
