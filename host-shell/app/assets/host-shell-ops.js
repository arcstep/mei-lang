/**
 * mei-host-shell ops helpers (used by host-runtime-console.js).
 */
(function (global) {
  "use strict";

  global.MeiHostShellOps = {
    refreshPanel: () =>
      global.MeiHostRuntimeConsole && global.MeiHostRuntimeConsole.refreshConsole
        ? global.MeiHostRuntimeConsole.refreshConsole()
        : Promise.resolve(false),
    initHostShellOps: () => {},
  };
})(typeof window !== "undefined" ? window : globalThis);
