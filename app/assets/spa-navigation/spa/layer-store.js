/**
 * Client LayerStore: semantic artifact_id keys + in-memory L1 with IDB L2.
 */
(function initLayerStore(global) {
  "use strict";

  const boot = (global.__meiLangBoot = global.__meiLangBoot || {});
  const store = new Map();
  const revisionStore = new Map();
  const holdingsIndex = new Map();

  function semanticLayerKey(artifactId) {
    return String(artifactId || "").trim();
  }

  function legacyLayerKey(surface, appId, sceneId, layerName, axes) {
    return [
      surface,
      appId,
      sceneId,
      layerName,
      axes?.data_mode || "",
    ]
      .filter(Boolean)
      .join(":");
  }

  function layerKey(surface, appId, sceneId, layerName, axes, artifactId) {
    const semantic = semanticLayerKey(artifactId);
    if (semantic) return semantic;
    return legacyLayerKey(surface, appId, sceneId, layerName, axes);
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

  function indexKey(appId, sceneId) {
    return `${appId}:${sceneId}`;
  }

  function rememberHolding(appId, sceneId, holding) {
    const key = indexKey(appId, sceneId);
    const list = holdingsIndex.get(key) || [];
    const next = list.filter((row) => row.name !== holding.name);
    next.push(holding);
    holdingsIndex.set(key, next);
  }

  async function putLayerByRef(appId, sceneId, holding, bytes, manifest) {
    const key = semanticLayerKey(holding.artifact_id);
    if (!key) return;
    putLayer(key, bytes, holding.content_hash);
    const record = {
      name: holding.name,
      artifact_id: holding.artifact_id,
      content_hash: holding.content_hash,
    };
    rememberHolding(appId, sceneId, record);
    if (boot.layerArtifactCache?.putLayer) {
      await boot.layerArtifactCache.putLayer({
        artifact_id: holding.artifact_id,
        name: holding.name,
        content_hash: holding.content_hash,
        app_id: appId,
        scene_id: sceneId,
        bytes,
      });
    }
    if (manifest && boot.layerArtifactCache?.pruneStale) {
      await boot.layerArtifactCache.pruneStale(manifest, appId, sceneId);
    }
  }

  function takeLayerByRef(holding) {
    const key = semanticLayerKey(holding?.artifact_id);
    if (!key) return null;
    const cached = takeLayer(key);
    if (!cached) return null;
    const revision = revisionFor(key);
    if (holding.content_hash && revision && revision !== holding.content_hash) {
      return null;
    }
    return cached;
  }

  async function listHoldings(appId, sceneId) {
    const mem = holdingsIndex.get(indexKey(appId, sceneId)) || [];
    if (boot.layerArtifactCache?.listHoldings) {
      const idb = await boot.layerArtifactCache.listHoldings(appId, sceneId);
      const merged = new Map();
      for (const row of [...mem, ...idb]) {
        merged.set(row.name, row);
      }
      return Array.from(merged.values());
    }
    return mem.slice();
  }

  function syncHoldingsFromManifest(manifest) {
    if (!manifest?.layers) return [];
    const holdings = [];
    for (const [name, value] of Object.entries(manifest.layers)) {
      const artifactId = String(value?.artifact_id || "").trim();
      const contentHash = String(value?.content_hash || "").trim();
      if (!artifactId || !contentHash) continue;
      holdings.push({ name, artifact_id: artifactId, content_hash: contentHash });
    }
    const appId = manifest.app_id || manifest.appId;
    const sceneId = manifest.scene_id || manifest.sceneId;
    if (appId && sceneId) {
      holdingsIndex.set(indexKey(appId, sceneId), holdings);
    }
    return holdings;
  }

  boot.layerStore = {
    semanticLayerKey,
    layerKey,
    rememberRevision,
    revisionFor,
    putLayer,
    putLayerByRef,
    takeLayer,
    takeLayerByRef,
    hasLayer,
    listHoldings,
    syncHoldingsFromManifest,
    clear() {
      store.clear();
      revisionStore.clear();
      holdingsIndex.clear();
    },
  };
})(typeof window !== "undefined" ? window : globalThis);
