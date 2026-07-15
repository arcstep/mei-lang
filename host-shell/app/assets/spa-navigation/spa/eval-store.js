(function initEvalStore(global) {
  "use strict";

  const boot = (global.__meiLangBoot = global.__meiLangBoot || {});

  const REVISION_LS_PREFIX = "mei:eval-store-revision:v1:";

  function revisionStorageKey(appId, scope) {
    return `${String(appId || "").trim()}:${String(scope || "home").trim()}`;
  }

  function readStoredRevision(appId, scope) {
    try {
      return (
        localStorage.getItem(`${REVISION_LS_PREFIX}${revisionStorageKey(appId, scope)}`) || ""
      ).trim();
    } catch (_) {
      return "";
    }
  }

  function writeStoredRevision(appId, scope, revision) {
    try {
      localStorage.setItem(
        `${REVISION_LS_PREFIX}${revisionStorageKey(appId, scope)}`,
        String(revision || ""),
      );
    } catch (_) {}
  }

  function currentRevisionCoords() {
    const mei = window.__mei || {};
    return {
      appId: String(mei.bootstrap_app_id || mei.app_id || window.__meiRuntimeAppId || "").trim(),
      scope: String(mei.bootstrap_scope || mei.active_scene_id || "home").trim(),
      clientRevision: String(mei.client_revision || mei.clientRevision || "").trim(),
      dataGeneration: String(
        mei.bootstrap_data_generation || mei.data_generation || window.__meiRuntimeDataGeneration || "",
      ).trim(),
      fingerprint: `${mei.compile_epoch || mei.bootstrap_compile_epoch || ""}|${mei.bootstrap_data_generation || mei.data_generation || ""}`,
    };
  }

  function clearRuntimeQueryCaches() {
    if (typeof boot.evalStoreCache?.clearAll === "function") {
      boot.evalStoreCache.clearAll();
      return;
    }
    if (typeof window.clearEvalRuntimeCaches === "function") {
      window.clearEvalRuntimeCaches();
    }
  }

  function ensureRevisionAligned() {
    const coords = currentRevisionCoords();
    if (!coords.appId || !coords.clientRevision) {
      return coords;
    }
    const prev = readStoredRevision(coords.appId, coords.scope);
    if (prev && prev !== coords.clientRevision) {
      clearRuntimeQueryCaches();
      window.__meiBootstrapSeeded = false;
      window.__meiBootstrapSeedCount = 0;
    }
    writeStoredRevision(coords.appId, coords.scope, coords.clientRevision);
    return coords;
  }

  function seedPack(source, meta = {}) {
    ensureRevisionAligned();
    if (typeof seedFromBootstrap !== "function") {
      return 0;
    }
    const count = seedFromBootstrap(source || window.__mei);
    if (count > 0) {
      window.__meiBootstrapSeeded = true;
      window.__meiBootstrapSeedCount = count;
      delete window.__meiBootstrapSeedError;
      window.__meiEvalPackSource = meta.source || window.__meiEvalPackSource || "eval_store";
    }
    return count;
  }

  function readEvalReaders() {
    return window.__meiEvalStoreReaders || {};
  }

  function getMetric(api, payload, fingerprint = "") {
    const metricMap = boot.evalStoreCache?.metricResults?.() || boot.evalStoreCaches?.metricResults;
    if (!metricMap) {
      return readEvalReaders().readMetric?.(api, payload, fingerprint) || null;
    }
    const readers = readEvalReaders();
    const cacheKey =
      typeof readers.metricCacheKey === "function"
        ? readers.metricCacheKey(api, payload, fingerprint)
        : "";
    return cacheKey ? metricMap.get(cacheKey)?.data || null : null;
  }

  function getDatasetPage1(api, payload, fingerprint = "") {
    const datasetMap = boot.evalStoreCache?.datasetResults?.() || boot.evalStoreCaches?.datasetResults;
    if (!datasetMap) {
      return readEvalReaders().readDataset?.(api, payload, fingerprint) || null;
    }
    const readers = readEvalReaders();
    const cacheKey =
      typeof readers.datasetCacheKey === "function"
        ? readers.datasetCacheKey(api, payload, fingerprint)
        : "";
    return cacheKey ? datasetMap.get(cacheKey)?.data || null : null;
  }

  function clear(revisionCoords = null) {
    const coords = revisionCoords || currentRevisionCoords();
    clearRuntimeQueryCaches();
    if (coords.appId && coords.scope) {
      writeStoredRevision(coords.appId, coords.scope, "");
    }
    window.__meiBootstrapSeeded = false;
    window.__meiBootstrapSeedCount = 0;
    delete window.__meiEvalDeliveryClassByMetric;
  }

  let jitPackInflight = null;

  function queryStateHasFilterDelta(state) {
    if (!state || typeof state !== "object") {
      return false;
    }
    if (String(state.search || "").trim()) {
      return true;
    }
    const filters = state.filters && typeof state.filters === "object" ? state.filters : {};
    return Object.keys(filters).length > 0;
  }

  function installQueryStateJitListener() {
    if (installQueryStateJitListener._installed) {
      return;
    }
    installQueryStateJitListener._installed = true;
    window.addEventListener("mei:query-state-change", (event) => {
      const state = event?.detail?.state;
      if (!queryStateHasFilterDelta(state)) {
        return;
      }
      const coords = currentRevisionCoords();
      if (!coords.appId) {
        return;
      }
      if (jitPackInflight) {
        return;
      }
      const fingerprint = [
        coords.fingerprint,
        String(event?.detail?.id || "").trim(),
        JSON.stringify(state.filters || {}),
        String(state.search || "").trim(),
      ].join("|");
      jitPackInflight = Promise.resolve(
        boot.fetchJitEvalPack?.(
          { appId: coords.appId, sceneId: coords.scope },
          { fingerprint },
        ),
      )
        .catch(() => 0)
        .finally(() => {
          jitPackInflight = null;
        });
    });
  }

  installQueryStateJitListener();

  boot.evalStore = {
    ensureRevisionAligned,
    seedPack,
    getMetric,
    getDatasetPage1,
    clear,
    clearRuntimeQueryCaches,
    currentRevisionCoords,
  };
})(typeof window !== "undefined" ? window : globalThis);
