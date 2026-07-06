/**
 * Physical EvalStore cache maps (0524 E8). Loaded before eval-store.js in access bundle.
 */
(function initEvalStoreCaches(global) {
  "use strict";

  const boot = (global.__meiLangBoot = global.__meiLangBoot || {});

  const caches = {
    metricInflight: new Map(),
    metricResults: new Map(),
    metricScopeInflight: new Map(),
    metricScopeResults: new Map(),
    datasetInflight: new Map(),
    datasetResults: new Map(),
  };

  function clearAll() {
    caches.metricInflight.clear();
    caches.metricResults.clear();
    caches.metricScopeInflight.clear();
    caches.metricScopeResults.clear();
    caches.datasetInflight.clear();
    caches.datasetResults.clear();
  }

  boot.evalStoreCaches = caches;
  boot.evalStoreCache = {
    caches,
    clearAll,
    metricResults: () => caches.metricResults,
    datasetResults: () => caches.datasetResults,
    metricScopeResults: () => caches.metricScopeResults,
  };

  global.__meiEvalStoreCaches = caches;
})(typeof window !== "undefined" ? window : globalThis);
