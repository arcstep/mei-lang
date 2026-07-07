/**
 * IndexedDB persistence for preview surfaceHtml (Route A thin-shell F5 cache).
 */
(function initPreviewSurfaceCache(global) {
  "use strict";

  const boot = (global.__meiLangBoot = global.__meiLangBoot || {});
  const DB_NAME = "mei-preview-surface-cache-v1";
  const STORE_NAME = "surfaces";
  const DB_VERSION = 1;
  const MAX_ENTRIES_PER_APP = 16;
  const DISABLE_LS = "mei:preview-surface-cache";

  function cacheEnabled() {
    try {
      if (global.localStorage?.getItem(DISABLE_LS) === "0") return false;
    } catch (_) {}
    return true;
  }

  function stableComposeHash(ctx) {
    const payload = {
      data_mode: String(ctx?.data_mode || ctx?.dataMode || "").trim(),
      review_projection: String(ctx?.review_projection || ctx?.reviewProjection || "").trim(),
      chrome: String(ctx?.chrome || "").trim(),
    };
    return JSON.stringify(payload);
  }

  function resolveDigests(ctx, digestsIn) {
    const fromArg = digestsIn && typeof digestsIn === "object" ? digestsIn : null;
    if (fromArg?.surface_revision_digest) {
      return {
        manifest_revision_digest: String(fromArg.manifest_revision_digest || "").trim(),
        surface_revision_digest: String(fromArg.surface_revision_digest || "").trim(),
      };
    }
    if (typeof boot.readClientDigests === "function") {
      const read = boot.readClientDigests(ctx) || {};
      if (read.surface_revision_digest) {
        return {
          manifest_revision_digest: String(read.manifest_revision_digest || "").trim(),
          surface_revision_digest: String(read.surface_revision_digest || "").trim(),
        };
      }
    }
    const refs = global.__mei?.scene_manifest_refs || {};
    return {
      manifest_revision_digest: String(
        refs.revision_digest || refs.manifest_revision_digest || "",
      ).trim(),
      surface_revision_digest: String(refs.surface_revision_digest || "").trim(),
    };
  }

  function resolveDataGeneration() {
    const mei = global.__mei || {};
    return String(
      mei.bootstrap_data_generation || mei.data_generation || boot.readBootstrapMeta?.("mei-bootstrap-data-generation") || "",
    ).trim();
  }

  function buildCacheKey(ctx, digestsIn) {
    const appId = String(ctx?.app_id || ctx?.appId || "").trim();
    const sceneId = String(ctx?.scene_id || ctx?.sceneId || "home").trim() || "home";
    const digests = resolveDigests(ctx, digestsIn);
    const surfaceDigest = digests.surface_revision_digest;
    if (!appId || !sceneId || !surfaceDigest) return "";
    const composeHash = stableComposeHash(ctx);
    const dataGen = resolveDataGeneration();
    return `${appId}:${sceneId}:${surfaceDigest}:${composeHash}:${dataGen}:fragment`;
  }

  function openDb() {
    if (!cacheEnabled() || typeof indexedDB === "undefined") {
      return Promise.resolve(null);
    }
    return new Promise((resolve) => {
      try {
        const request = indexedDB.open(DB_NAME, DB_VERSION);
        request.onupgradeneeded = () => {
          const db = request.result;
          if (!db.objectStoreNames.contains(STORE_NAME)) {
            const store = db.createObjectStore(STORE_NAME, { keyPath: "cache_key" });
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

  async function getSurfaceHtml(cacheKey) {
    const key = String(cacheKey || "").trim();
    if (!key || !cacheEnabled()) return null;
    const db = await openDb();
    if (!db) return null;
    return new Promise((resolve) => {
      try {
        const tx = db.transaction(STORE_NAME, "readonly");
        const request = tx.objectStore(STORE_NAME).get(key);
        request.onsuccess = () => {
          const row = request.result;
          if (!row?.surface_html) {
            resolve(null);
            return;
          }
          resolve({
            surfaceHtml: row.surface_html,
            bytes: row.bytes || row.surface_html.length,
            stored_at: row.stored_at || 0,
            cache_key: row.cache_key,
          });
        };
        request.onerror = () => resolve(null);
      } catch (_) {
        resolve(null);
      }
    });
  }

  async function putSurfaceHtml(cacheKey, surfaceHtml, meta) {
    const key = String(cacheKey || "").trim();
    const html = String(surfaceHtml || "");
    if (!key || !html || !cacheEnabled()) return false;
    const db = await openDb();
    if (!db) return false;
    const info = meta && typeof meta === "object" ? meta : {};
    const record = {
      cache_key: key,
      app_id: String(info.app_id || "").trim(),
      scene_id: String(info.scene_id || "").trim(),
      surface_revision_digest: String(info.surface_revision_digest || "").trim(),
      data_generation: String(info.data_generation || resolveDataGeneration()).trim(),
      surface_html: html,
      bytes: html.length,
      stored_at: Date.now(),
    };
    return new Promise((resolve) => {
      try {
        const tx = db.transaction(STORE_NAME, "readwrite");
        tx.objectStore(STORE_NAME).put(record);
        tx.oncomplete = () => {
          if (typeof boot.cacheDiagTrace === "function") {
            boot.cacheDiagTrace("preview-idb-write", {
              cache_key: key,
              bytes: record.bytes,
            });
          }
          void pruneOverflow(record.app_id, record.scene_id, key);
          resolve(true);
        };
        tx.onerror = () => resolve(false);
      } catch (_) {
        resolve(false);
      }
    });
  }

  async function deleteEntry(cacheKey) {
    const key = String(cacheKey || "").trim();
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

  async function listEntries(appId, sceneId) {
    const db = await openDb();
    if (!db) return [];
    return new Promise((resolve) => {
      try {
        const tx = db.transaction(STORE_NAME, "readonly");
        const index = tx.objectStore(STORE_NAME).index("app_scene");
        const request = index.getAll([String(appId || ""), String(sceneId || "")]);
        request.onsuccess = () => resolve(request.result || []);
        request.onerror = () => resolve([]);
      } catch (_) {
        resolve([]);
      }
    });
  }

  async function pruneStale(appId, sceneId, validKeys) {
    const valid = validKeys instanceof Set ? validKeys : new Set(validKeys || []);
    const rows = await listEntries(appId, sceneId);
    for (const row of rows) {
      if (!valid.has(row.cache_key)) {
        await deleteEntry(row.cache_key);
      }
    }
  }

  async function pruneOverflow(appId, sceneId, keepKey) {
    const rows = await listEntries(appId, sceneId);
    if (rows.length <= MAX_ENTRIES_PER_APP) return;
    const sorted = rows
      .slice()
      .filter((row) => row.cache_key !== keepKey)
      .sort((a, b) => (a.stored_at || 0) - (b.stored_at || 0));
    const excess = sorted.length - MAX_ENTRIES_PER_APP + 1;
    for (let i = 0; i < excess && i < sorted.length; i += 1) {
      await deleteEntry(sorted[i].cache_key);
    }
  }

  async function deleteAllForApp(appId) {
    const db = await openDb();
    if (!db) return;
    return new Promise((resolve) => {
      try {
        const tx = db.transaction(STORE_NAME, "readwrite");
        const store = tx.objectStore(STORE_NAME);
        const request = store.openCursor();
        request.onsuccess = () => {
          const cursor = request.result;
          if (!cursor) return;
          if (String(cursor.value?.app_id || "") === String(appId || "")) {
            cursor.delete();
          }
          cursor.continue();
        };
        tx.oncomplete = () => resolve(true);
        tx.onerror = () => resolve(false);
      } catch (_) {
        resolve(false);
      }
    });
  }

  async function tryGetCachedSurface(ctx, options) {
    if (!cacheEnabled() || options?.forceRematerialize === true) {
      if (typeof boot.cacheDiagTrace === "function") {
        boot.cacheDiagTrace("preview-idb-miss", { reason: "disabled_or_force" });
      }
      return null;
    }
    const cacheKey = buildCacheKey(ctx, options?.digests);
    if (!cacheKey) {
      if (typeof boot.cacheDiagTrace === "function") {
        boot.cacheDiagTrace("preview-idb-miss", { reason: "no_cache_key" });
      }
      return null;
    }
    const cached = await getSurfaceHtml(cacheKey);
    if (!cached?.surfaceHtml) {
      if (typeof boot.cacheDiagTrace === "function") {
        boot.cacheDiagTrace("preview-idb-miss", { cache_key: cacheKey });
      }
      return null;
    }
    if (typeof boot.cacheDiagTrace === "function") {
      boot.cacheDiagTrace("preview-idb-hit", {
        cache_key: cacheKey,
        bytes: cached.bytes,
      });
    }
    return { ...cached, cacheKey };
  }

  async function storeCachedSurface(ctx, surfaceHtml, options) {
    const cacheKey = buildCacheKey(ctx, options?.digests);
    if (!cacheKey) return false;
    const digests = resolveDigests(ctx, options?.digests);
    return putSurfaceHtml(cacheKey, surfaceHtml, {
      app_id: ctx?.app_id || ctx?.appId,
      scene_id: ctx?.scene_id || ctx?.sceneId,
      surface_revision_digest: digests.surface_revision_digest,
      data_generation: resolveDataGeneration(),
    });
  }

  boot.previewSurfaceCache = {
    cacheEnabled,
    buildCacheKey,
    getSurfaceHtml,
    putSurfaceHtml,
    tryGetCachedSurface,
    storeCachedSurface,
    pruneStale,
    deleteAllForApp,
    listEntries,
  };
})(typeof window !== "undefined" ? window : globalThis);
