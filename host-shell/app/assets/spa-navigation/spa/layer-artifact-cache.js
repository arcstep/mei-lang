/**
 * IndexedDB persistence for semantic layer artifacts (0514 artifact_id keys).
 */
(function initLayerArtifactCache(global) {
  "use strict";

  const boot = (global.__meiLangBoot = global.__meiLangBoot || {});
  const DB_NAME = "mei-layer-artifact-cache-v1";
  const STORE_NAME = "layers";
  const DB_VERSION = 1;
  const MAX_ENTRIES_PER_APP = 512;
  let dbPromise = null;
  const diagnostics = {
    opens: 0,
    readonlyTransactions: 0,
    readwriteTransactions: 0,
    completedReadwriteTransactions: 0,
    reads: 0,
    writes: 0,
    prunes: 0,
  };

  function openDb() {
    if (typeof indexedDB === "undefined") {
      return Promise.resolve(null);
    }
    if (dbPromise) return dbPromise;
    const startedAt = typeof performance !== "undefined" ? performance.now() : Date.now();
    diagnostics.opens += 1;
    dbPromise = new Promise((resolve) => {
      try {
        const request = indexedDB.open(DB_NAME, DB_VERSION);
        request.onupgradeneeded = () => {
          const db = request.result;
          if (!db.objectStoreNames.contains(STORE_NAME)) {
            const store = db.createObjectStore(STORE_NAME, { keyPath: "artifact_id" });
            store.createIndex("app_scene", ["app_id", "scene_id"], { unique: false });
          }
        };
        request.onsuccess = () => {
          const db = request.result;
          db.onversionchange = () => {
            db.close();
            dbPromise = null;
          };
          boot.renderPipelineMark?.("idb_open:end", {
            durationMs: Math.round(
              (typeof performance !== "undefined" ? performance.now() : Date.now()) - startedAt,
            ),
          });
          resolve(db);
        };
        request.onerror = () => {
          dbPromise = null;
          resolve(null);
        };
        request.onblocked = () => {
          boot.renderPipelineMark?.("idb_open:blocked");
        };
      } catch (_) {
        dbPromise = null;
        resolve(null);
      }
    });
    boot.renderPipelineMark?.("idb_open:begin");
    return dbPromise;
  }

  async function getLayers(artifactIds) {
    const keys = Array.from(
      new Set((Array.isArray(artifactIds) ? artifactIds : []).map((value) => String(value || "").trim()).filter(Boolean)),
    );
    if (!keys.length) return new Map();
    const db = await openDb();
    if (!db) return new Map();
    diagnostics.readonlyTransactions += 1;
    diagnostics.reads += keys.length;
    const startedAt = typeof performance !== "undefined" ? performance.now() : Date.now();
    boot.renderPipelineMark?.("idb_transaction:begin", {
      mode: "readonly",
      count: keys.length,
    });
    return new Promise((resolve) => {
      const rows = new Map();
      try {
        const tx = db.transaction(STORE_NAME, "readonly");
        const store = tx.objectStore(STORE_NAME);
        for (const key of keys) {
          const request = store.get(key);
          request.onsuccess = () => {
            if (request.result) rows.set(key, request.result);
          };
        }
        tx.oncomplete = () => {
          boot.renderPipelineMark?.("idb_transaction:end", {
            mode: "readonly",
            count: keys.length,
            hits: rows.size,
            durationMs: Math.round(
              (typeof performance !== "undefined" ? performance.now() : Date.now()) - startedAt,
            ),
          });
          resolve(rows);
        };
        tx.onerror = () => resolve(rows);
        tx.onabort = () => resolve(rows);
      } catch (_) {
        resolve(rows);
      }
    });
  }

  async function getLayer(artifactId) {
    const key = String(artifactId || "").trim();
    if (!key) return null;
    const rows = await getLayers([key]);
    return rows.get(key) || null;
  }

  function normalizeRecord(entry) {
    if (!entry?.artifact_id) return null;
    return {
      artifact_id: String(entry.artifact_id),
      name: String(entry.name || ""),
      content_hash: String(entry.content_hash || ""),
      app_id: String(entry.app_id || ""),
      scene_id: String(entry.scene_id || ""),
      bytes: entry.bytes,
      stored_at: Date.now(),
    };
  }

  async function putLayers(entries) {
    const records = (Array.isArray(entries) ? entries : []).map(normalizeRecord).filter(Boolean);
    if (!records.length) return true;
    const db = await openDb();
    if (!db) return false;
    diagnostics.readwriteTransactions += 1;
    diagnostics.writes += records.length;
    const startedAt = typeof performance !== "undefined" ? performance.now() : Date.now();
    boot.renderPipelineMark?.("idb_transaction:begin", {
      mode: "readwrite",
      count: records.length,
    });
    return new Promise((resolve) => {
      try {
        const tx = db.transaction(STORE_NAME, "readwrite");
        const store = tx.objectStore(STORE_NAME);
        for (const record of records) store.put(record);
        tx.oncomplete = () => {
          diagnostics.completedReadwriteTransactions += 1;
          boot.renderPipelineMark?.("idb_transaction:end", {
            mode: "readwrite",
            count: records.length,
            durationMs: Math.round(
              (typeof performance !== "undefined" ? performance.now() : Date.now()) - startedAt,
            ),
          });
          resolve(true);
        };
        tx.onerror = () => resolve(false);
        tx.onabort = () => resolve(false);
      } catch (_) {
        resolve(false);
      }
    });
  }

  async function putLayer(entry) {
    return putLayers([entry]);
  }

  async function deleteLayers(artifactIds) {
    const keys = Array.from(
      new Set((Array.isArray(artifactIds) ? artifactIds : []).map((value) => String(value || "").trim()).filter(Boolean)),
    );
    if (!keys.length) return true;
    const db = await openDb();
    if (!db) return false;
    diagnostics.readwriteTransactions += 1;
    diagnostics.writes += keys.length;
    return new Promise((resolve) => {
      try {
        const tx = db.transaction(STORE_NAME, "readwrite");
        const store = tx.objectStore(STORE_NAME);
        for (const key of keys) store.delete(key);
        tx.oncomplete = () => {
          diagnostics.completedReadwriteTransactions += 1;
          resolve(true);
        };
        tx.onerror = () => resolve(false);
        tx.onabort = () => resolve(false);
      } catch (_) {
        resolve(false);
      }
    });
  }

  async function deleteLayer(artifactId) {
    return deleteLayers([artifactId]);
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
    diagnostics.prunes += 1;
    const validIds = new Set();
    for (const value of Object.values(manifest.layers)) {
      const artifactId = value?.artifact_id;
      if (artifactId) validIds.add(String(artifactId));
    }
    const db = await openDb();
    if (!db) return;
    const holdings = await listHoldings(appId, sceneId);
    const stale = holdings.filter((row) => !validIds.has(row.artifact_id));
    const deleteIds = stale.map((row) => row.artifact_id);
    if (holdings.length > MAX_ENTRIES_PER_APP) {
      const sorted = holdings
        .slice()
        .sort((a, b) => String(a.artifact_id).localeCompare(String(b.artifact_id)));
      for (const row of sorted.slice(0, sorted.length - MAX_ENTRIES_PER_APP)) {
        if (!validIds.has(row.artifact_id)) {
          deleteIds.push(row.artifact_id);
        }
      }
    }
    await deleteLayers(deleteIds);
  }

  function readDiagnostics() {
    return { ...diagnostics };
  }

  function resetDiagnostics() {
    for (const key of Object.keys(diagnostics)) diagnostics[key] = 0;
  }

  boot.layerArtifactCache = {
    openDb,
    getLayer,
    getLayers,
    putLayer,
    putLayers,
    deleteLayer,
    deleteLayers,
    listHoldings,
    pruneStale,
    readDiagnostics,
    resetDiagnostics,
  };
})(typeof window !== "undefined" ? window : globalThis);
