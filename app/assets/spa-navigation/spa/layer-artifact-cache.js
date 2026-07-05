/**
 * IndexedDB persistence for semantic layer artifacts (0514 artifact_id keys).
 */
(function initLayerArtifactCache(global) {
  "use strict";

  const boot = (global.__meiLangBoot = global.__meiLangBoot || {});
  const DB_NAME = "mei-layer-artifact-cache-v1";
  const STORE_NAME = "layers";
  const DB_VERSION = 1;
  const MAX_ENTRIES_PER_APP = 64;

  function openDb() {
    if (typeof indexedDB === "undefined") {
      return Promise.resolve(null);
    }
    return new Promise((resolve) => {
      try {
        const request = indexedDB.open(DB_NAME, DB_VERSION);
        request.onupgradeneeded = () => {
          const db = request.result;
          if (!db.objectStoreNames.contains(STORE_NAME)) {
            const store = db.createObjectStore(STORE_NAME, { keyPath: "artifact_id" });
            store.createIndex("app_scene", ["app_id", "scene_id"], { unique: false });
          }
        };
        request.onsuccess = () => resolve(request.result);
        request.onerror = () => resolve(null);
      } catch (_) {
        resolve(null);
      }
    });
  }

  async function getLayer(artifactId) {
    const key = String(artifactId || "").trim();
    if (!key) return null;
    const db = await openDb();
    if (!db) return null;
    return new Promise((resolve) => {
      try {
        const tx = db.transaction(STORE_NAME, "readonly");
        const request = tx.objectStore(STORE_NAME).get(key);
        request.onsuccess = () => resolve(request.result || null);
        request.onerror = () => resolve(null);
      } catch (_) {
        resolve(null);
      }
    });
  }

  async function putLayer(entry) {
    if (!entry?.artifact_id) return false;
    const db = await openDb();
    if (!db) return false;
    const record = {
      artifact_id: String(entry.artifact_id),
      name: String(entry.name || ""),
      content_hash: String(entry.content_hash || ""),
      app_id: String(entry.app_id || ""),
      scene_id: String(entry.scene_id || ""),
      bytes: entry.bytes,
      stored_at: Date.now(),
    };
    return new Promise((resolve) => {
      try {
        const tx = db.transaction(STORE_NAME, "readwrite");
        tx.objectStore(STORE_NAME).put(record);
        tx.oncomplete = () => resolve(true);
        tx.onerror = () => resolve(false);
      } catch (_) {
        resolve(false);
      }
    });
  }

  async function deleteLayer(artifactId) {
    const key = String(artifactId || "").trim();
    if (!key) return false;
    const db = await openDb();
    if (!db) return false;
    return new Promise((resolve) => {
      try {
        const tx = db.transaction(STORE_NAME, "readwrite");
        tx.objectStore(STORE_NAME).delete(key);
        tx.oncomplete = () => resolve(true);
        tx.onerror = () => resolve(false);
      } catch (_) {
        resolve(false);
      }
    });
  }

  async function listHoldings(appId, sceneId) {
    const db = await openDb();
    if (!db) return [];
    return new Promise((resolve) => {
      try {
        const tx = db.transaction(STORE_NAME, "readonly");
        const index = tx.objectStore(STORE_NAME).index("app_scene");
        const request = index.getAll([String(appId || ""), String(sceneId || "")]);
        request.onsuccess = () => {
          const rows = request.result || [];
          resolve(
            rows.map((row) => ({
              name: row.name,
              artifact_id: row.artifact_id,
              content_hash: row.content_hash,
            })),
          );
        };
        request.onerror = () => resolve([]);
      } catch (_) {
        resolve([]);
      }
    });
  }

  async function pruneStale(manifest, appId, sceneId) {
    if (!manifest?.layers) return;
    const validIds = new Set();
    for (const value of Object.values(manifest.layers)) {
      const artifactId = value?.artifact_id;
      if (artifactId) validIds.add(String(artifactId));
    }
    const db = await openDb();
    if (!db) return;
    const holdings = await listHoldings(appId, sceneId);
    const stale = holdings.filter((row) => !validIds.has(row.artifact_id));
    for (const row of stale) {
      await deleteLayer(row.artifact_id);
    }
    if (holdings.length > MAX_ENTRIES_PER_APP) {
      const sorted = holdings
        .slice()
        .sort((a, b) => String(a.artifact_id).localeCompare(String(b.artifact_id)));
      for (const row of sorted.slice(0, sorted.length - MAX_ENTRIES_PER_APP)) {
        if (!validIds.has(row.artifact_id)) {
          await deleteLayer(row.artifact_id);
        }
      }
    }
  }

  boot.layerArtifactCache = {
    openDb,
    getLayer,
    putLayer,
    deleteLayer,
    listHoldings,
    pruneStale,
  };
})(typeof window !== "undefined" ? window : globalThis);
