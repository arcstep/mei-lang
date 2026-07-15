/**
 * Client LayerStore: semantic artifact_id keys + in-memory L1 with IDB L2.
 */
(function initLayerStore(global) {
  "use strict";

  const boot = (global.__meiLangBoot = global.__meiLangBoot || {});
  const store = new Map();
  const revisionStore = new Map();
  const holdingsIndex = new Map();
  const scheduledPrunes = new Set();

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

  function fillMemoryByRef(appId, sceneId, holding, bytes) {
    const key = semanticLayerKey(holding.artifact_id);
    if (!key) return false;
    putLayer(key, bytes, holding.content_hash);
    const record = {
      name: holding.name,
      artifact_id: holding.artifact_id,
      content_hash: holding.content_hash,
    };
    rememberHolding(appId, sceneId, record);
    return true;
  }

  function schedulePrune(manifest, appId, sceneId) {
    if (!manifest || !boot.layerArtifactCache?.pruneStale) return;
    const digest = String(
      manifest.manifest_digest || manifest.surface_digest || manifest.revision || "",
    );
    const key = `${appId}:${sceneId}:${digest}`;
    if (scheduledPrunes.has(key)) return;
    scheduledPrunes.add(key);
    const run = () => {
      void boot.layerArtifactCache
        .pruneStale(manifest, appId, sceneId)
        .catch(() => {})
        .finally(() => scheduledPrunes.delete(key));
    };
    if (typeof global.requestIdleCallback === "function") {
      global.requestIdleCallback(run, { timeout: 5000 });
    } else {
      global.setTimeout(run, 1000);
    }
  }

  async function putLayersByRef(appId, sceneId, entries, manifest, options) {
    const rows = Array.isArray(entries) ? entries : [];
    const records = [];
    for (const entry of rows) {
      if (!entry?.holding || !fillMemoryByRef(appId, sceneId, entry.holding, entry.bytes)) {
        continue;
      }
      records.push({
        artifact_id: entry.holding.artifact_id,
        name: entry.holding.name,
        content_hash: entry.holding.content_hash,
        app_id: appId,
        scene_id: sceneId,
        bytes: entry.bytes,
      });
    }
    const persist = async () => {
      const ok = boot.layerArtifactCache?.putLayers
        ? await boot.layerArtifactCache.putLayers(records)
        : true;
      schedulePrune(manifest, appId, sceneId);
      return ok;
    };
    if (options?.awaitPersist === true) return persist();
    void persist();
    return true;
  }

  async function putLayerByRef(appId, sceneId, holding, bytes, manifest) {
    return putLayersByRef(
      appId,
      sceneId,
      [{ holding, bytes }],
      manifest,
      { awaitPersist: true },
    );
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

  async function restoreLayersByRefs(appId, sceneId, holdings) {
    const rows = Array.isArray(holdings) ? holdings : [];
    const resolved = new Map();
    const missing = [];
    let l1Hits = 0;
    for (const holding of rows) {
      const bytes = takeLayerByRef(holding);
      if (bytes != null) {
        resolved.set(holding.name, bytes);
        l1Hits += 1;
      } else if (holding?.artifact_id) {
        missing.push(holding);
      }
    }
    const persisted = boot.layerArtifactCache?.getLayers
      ? await boot.layerArtifactCache.getLayers(missing.map((holding) => holding.artifact_id))
      : new Map();
    let idbHits = 0;
    for (const holding of missing) {
      const row = persisted.get(String(holding.artifact_id));
      if (!row) continue;
      if (
        holding.content_hash &&
        row.content_hash &&
        String(holding.content_hash) !== String(row.content_hash)
      ) {
        continue;
      }
      fillMemoryByRef(appId, sceneId, holding, row.bytes);
      resolved.set(holding.name, row.bytes);
      idbHits += 1;
    }
    const misses = rows.filter((holding) => !resolved.has(holding.name));
    boot.renderPipelineMark?.("layer_restore:end", {
      count: rows.length,
      l1Hits,
      idbHits,
      misses: misses.length,
    });
    return { resolved, misses, l1Hits, idbHits };
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
    fillMemoryByRef,
    putLayerByRef,
    putLayersByRef,
    takeLayer,
    takeLayerByRef,
    restoreLayersByRefs,
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
