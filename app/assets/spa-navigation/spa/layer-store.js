/**
 * Client LayerStore: artifact revision keys + in-memory layer bytes.
 */
(function initLayerStore(global) {
  "use strict";

  const boot = (global.__meiLangBoot = global.__meiLangBoot || {});
  const store = new Map();
  const revisionStore = new Map();

  function layerKey(surface, appId, sceneId, layerName, axes) {
    const base = boot.surfaceRevisionKey
      ? boot.surfaceRevisionKey({
          surface,
          app_id: appId,
          scene_id: sceneId,
          layer: layerName,
          data_mode: axes?.data_mode || "",
          review_projection: axes?.review_projection || "",
        })
      : [surface, appId, sceneId, layerName].join(":");
    return base;
  }

  function rememberRevision(key, revision) {
    if (!key || !revision) return;
    revisionStore.set(key, String(revision));
  }

  function revisionFor(key) {
    return revisionStore.get(key) || "";
  }

  function putLayer(key, bytes, revision) {
    if (!key) return;
    store.set(key, bytes);
    rememberRevision(key, revision);
  }

  function takeLayer(key) {
    return store.get(key) || null;
  }

  function hasLayer(key) {
    return store.has(key);
  }

  boot.layerStore = {
    layerKey,
    rememberRevision,
    revisionFor,
    putLayer,
    takeLayer,
    hasLayer,
    clear() {
      store.clear();
      revisionStore.clear();
    },
  };
})(typeof window !== "undefined" ? window : globalThis);
