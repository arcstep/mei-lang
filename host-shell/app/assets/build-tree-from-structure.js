/**
 * Manage bundle shim: delegates to shared structure-tree-materializer.
 */
(function initBuildTreeFromStructure(global) {
  "use strict";

  const boot = (global.__meiLangBoot = global.__meiLangBoot || {});
  if (typeof boot.renderStructureTree !== "function") {
    console.warn("[build-tree-from-structure] structure-tree-materializer not loaded");
  }
})(typeof window !== "undefined" ? window : globalThis);
