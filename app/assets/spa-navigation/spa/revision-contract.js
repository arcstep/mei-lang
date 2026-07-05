/**
 * Shared revision contract for access-like scene shell and Build workspace fragments.
 */
(function initRevisionContract(global) {
  "use strict";

  const boot = (global.__meiLangBoot = global.__meiLangBoot || {});
  const VIEW_REVISION_STORE_KEY = "mei-view-revisions";
  const VIEW_REVISION_LS_KEY = "mei:view-revisions:v1";
  const LEGACY_SCENE_REVISION_STORE_KEY = "mei-scene-revisions";
  const LEGACY_BUILD_REVISION_STORE_KEY = "mei-build-fragment-revisions";

  function normalizeRevision(revision) {
    if (!revision || typeof revision !== "object") return revision;
    return {
      ...revision,
      revision_digest: String(
        revision.revision_digest || revision.revisionDigest || "",
      ).trim(),
      manifest_revision_digest: String(
        revision.manifest_revision_digest || revision.manifestRevisionDigest || "",
      ).trim(),
      surface_revision_digest: String(
        revision.surface_revision_digest || revision.surfaceRevisionDigest || "",
      ).trim(),
      cache_key: String(revision.cache_key || revision.cacheKey || "").trim() || undefined,
      client_revision: String(
        revision.client_revision || revision.clientRevision || "",
      ).trim(),
      registry_revision: String(
        revision.registry_revision || revision.registryRevision || "",
      ).trim(),
      data_generation: String(
        revision.data_generation || revision.dataGeneration || "",
      ).trim(),
      scene_bundle_revision:
        revision.scene_bundle_revision || revision.sceneBundleRevision || "",
      draft_digest: revision.draft_digest || revision.draftDigest || "",
    };
  }

  function revisionsMatch(localRevision, remoteRevision) {
    const local = normalizeRevision(localRevision);
    const remote = normalizeRevision(remoteRevision);
    if (!local || !remote) return false;
    if (local.revision_digest && remote.revision_digest) {
      return local.revision_digest === remote.revision_digest;
    }
    if (local.manifest_revision_digest && remote.manifest_revision_digest) {
      return local.manifest_revision_digest === remote.manifest_revision_digest;
    }
    if (local.cache_key && remote.cache_key) {
      return local.cache_key === remote.cache_key;
    }
    return (
      local.registry_revision === remote.registry_revision &&
      local.client_revision === remote.client_revision &&
      local.data_generation === remote.data_generation &&
      (local.scene_bundle_revision || "") === (remote.scene_bundle_revision || "") &&
      (local.draft_digest || "") === (remote.draft_digest || "")
    );
  }

  function semanticRevisionKey(ctx) {
    const payload = ctx || {};
    return [
      payload.app_id || payload.appId || "",
      payload.scene_id || payload.sceneId || "",
      payload.data_mode || payload.dataMode || "",
    ]
      .filter(Boolean)
      .join(":");
  }

  function surfaceComposeKey(ctx) {
    const payload = ctx || {};
    return [
      payload.surface || payload.mode || payload.route_mode || "",
      payload.tab || "",
      payload.chrome || "",
      payload.review_projection || payload.reviewProjection || "",
    ]
      .filter(Boolean)
      .join(":");
  }

  function surfaceRevisionKey(parts) {
    const payload = parts || {};
    return [
      payload.surface || payload.route_mode || "",
      payload.app_id || payload.appId || "",
      payload.scene_id || payload.sceneId || "",
      payload.node || "",
      payload.data_mode || payload.dataMode || "",
      payload.review_projection || payload.reviewProjection || "",
      payload.chrome || "",
      payload.focus || "",
      payload.scope || "",
      payload.draft_session || payload.draftSession || "",
    ]
      .filter(Boolean)
      .join(":");
  }

  function readViewRevisionStore() {
    try {
      const raw = global.sessionStorage.getItem(VIEW_REVISION_STORE_KEY);
      if (raw) return JSON.parse(raw);
    } catch (_) {}
    try {
      const raw = global.localStorage.getItem(VIEW_REVISION_LS_KEY);
      if (raw) return JSON.parse(raw);
    } catch (_) {}
    const merged = {};
    try {
      const sceneRaw =
        global.sessionStorage.getItem(LEGACY_SCENE_REVISION_STORE_KEY) ||
        global.localStorage.getItem("mei:scene-revisions:v1");
      if (sceneRaw) Object.assign(merged, JSON.parse(sceneRaw));
    } catch (_) {}
    try {
      const buildRaw = global.sessionStorage.getItem(LEGACY_BUILD_REVISION_STORE_KEY);
      if (buildRaw) Object.assign(merged, JSON.parse(buildRaw));
    } catch (_) {}
    return merged;
  }

  function writeViewRevisionStore(store) {
    const payload = JSON.stringify(store || {});
    try {
      global.sessionStorage.setItem(VIEW_REVISION_STORE_KEY, payload);
    } catch (_) {}
    try {
      global.localStorage.setItem(VIEW_REVISION_LS_KEY, payload);
    } catch (_) {}
  }

  function resolveComposeKeyCtx(ctx) {
    const payload = ctx || {};
    const tab = String(payload.tab || "").trim();
    if (tab) return payload;
    const surface = String(payload.surface || payload.mode || payload.route_mode || "app")
      .trim()
      .toLowerCase();
    const defaultTab =
      boot.sceneManifestLoader?.defaultTabForSurface?.(surface) ||
      (surface === "layout" || surface === "prototype" ? "preview" : "scene");
    return { ...payload, tab: defaultTab };
  }

  function viewRevisionStoreKey(ctx) {
    const resolved = resolveComposeKeyCtx(ctx);
    const semantic = semanticRevisionKey(resolved);
    const compose = surfaceComposeKey(resolved);
    if (!semantic || !compose) return semantic || "";
    return `${semantic}::${compose}`;
  }

  function ssrManifestMatchesSurface(ctx) {
    const refs = globalThis.__mei?.scene_manifest_refs;
    if (!refs || typeof refs !== "object") return false;
    const surface = String(ctx?.surface || ctx?.mode || "app")
      .trim()
      .toLowerCase();
    const routeMode = String(refs.compose_defaults?.route_mode || "")
      .trim()
      .toLowerCase();
    return !routeMode || routeMode === surface;
  }

  function rememberViewRevision(ctx, revision) {
    const resolved = resolveComposeKeyCtx(ctx);
    const storeKey = viewRevisionStoreKey(resolved);
    if (!storeKey || !revision) return;
    let manifest =
      revision.manifest ||
      revision.assembly_plan?.manifest ||
      revision.response?.manifest ||
      revision.response?.assembly_plan?.manifest ||
      revision.manifest_snapshot ||
      null;
    if (!manifest?.layers) {
      const refs = globalThis.__mei?.scene_manifest_refs;
      if (refs?.layers && ssrManifestMatchesSurface(resolved)) {
        manifest = refs;
      }
    }
    const store = readViewRevisionStore();
    store[storeKey] = {
      ...normalizeRevision(revision),
      manifest_revision_digest: String(
        revision.manifest_revision_digest ||
          manifest?.revision_digest ||
          revision.assembly_plan?.manifest?.revision_digest ||
          "",
      ).trim(),
      surface_revision_digest: String(
        revision.surface_revision_digest ||
          manifest?.surface_revision_digest ||
          "",
      ).trim(),
      surface_compose: surfaceComposeKey(resolved),
      manifest_snapshot: manifest,
    };
    pruneRevisionStore(store, storeKey, 96);
    writeViewRevisionStore(store);
  }

  function readViewRevision(ctx) {
    const resolved = resolveComposeKeyCtx(ctx);
    const storeKey = viewRevisionStoreKey(resolved);
    if (!storeKey) return null;
    const store = readViewRevisionStore();
    if (store[storeKey]) return store[storeKey];
    const legacyKey = semanticRevisionKey(resolved);
    const legacy = store[legacyKey];
    const composeKey = surfaceComposeKey(resolved);
    if (
      legacy &&
      legacy.surface_compose === composeKey &&
      legacy.manifest_revision_digest
    ) {
      return legacy;
    }
    return null;
  }

  function readSharedManifestDigest(ctx) {
    const resolved = resolveComposeKeyCtx(ctx);
    const semantic = semanticRevisionKey(resolved);
    if (!semantic) return "";
    const store = readViewRevisionStore();
    for (const [key, entry] of Object.entries(store)) {
      if (!key.startsWith(`${semantic}::`)) continue;
      const digest = String(entry?.manifest_revision_digest || "").trim();
      if (digest) return digest;
    }
    const refs = globalThis.__mei?.scene_manifest_refs;
    if (refs?.revision_digest) {
      return String(refs.revision_digest || "").trim();
    }
    return "";
  }

  function composeDefaultsForSurface(ctx) {
    const resolved = resolveComposeKeyCtx(ctx);
    const surface = String(resolved.surface || resolved.mode || "app").trim().toLowerCase();
    const defaultTab =
      boot.sceneManifestLoader?.defaultTabForSurface?.(surface) ||
      (surface === "layout" || surface === "prototype" ? "preview" : "scene");
    return {
      route_mode: surface,
      tab: String(resolved.tab || "").trim() || defaultTab,
      chrome: resolved.chrome || "",
      review_projection: resolved.review_projection || resolved.reviewProjection || "",
      data_mode: resolved.data_mode || resolved.dataMode || "",
      focus: resolved.focus || "",
      scope: resolved.scope || "",
    };
  }

  const SHARED_MANIFEST_LAYER_NAMES = new Set(["structure.full", "eval", "theme", "overlay"]);

  function pickSharedManifestLayers(layers) {
    const picked = {};
    if (!layers || typeof layers !== "object") return picked;
    for (const name of SHARED_MANIFEST_LAYER_NAMES) {
      if (layers[name]) picked[name] = layers[name];
    }
    return picked;
  }

  function readSharedManifestSnapshot(ctx) {
    const resolved = resolveComposeKeyCtx(ctx);
    const semantic = semanticRevisionKey(resolved);
    if (!semantic) return null;
    const store = readViewRevisionStore();
    let sharedLayers = {};
    for (const [key, entry] of Object.entries(store)) {
      if (!key.startsWith(`${semantic}::`)) continue;
      if (entry?.manifest_snapshot?.layers) {
        Object.assign(sharedLayers, pickSharedManifestLayers(entry.manifest_snapshot.layers));
      }
    }
    const refs = globalThis.__mei?.scene_manifest_refs;
    if (refs?.layers && ssrManifestMatchesSurface(resolved)) {
      return refs;
    }
    if (refs?.layers) {
      Object.assign(sharedLayers, pickSharedManifestLayers(refs.layers));
    }
    if (!Object.keys(sharedLayers).length) return null;
    return {
      layers: sharedLayers,
      compose_defaults: composeDefaultsForSurface(resolved),
    };
  }

  function readClientDigests(ctx) {
    const resolved = resolveComposeKeyCtx(ctx);
    const stored = readViewRevision(resolved);
    const manifest_revision_digest = String(
      stored?.manifest_revision_digest || readSharedManifestDigest(resolved) || "",
    ).trim();
    const surface_revision_digest = String(stored?.surface_revision_digest || "").trim();
    if (manifest_revision_digest && surface_revision_digest) {
      return { manifest_revision_digest, surface_revision_digest };
    }
    if (manifest_revision_digest) {
      return { manifest_revision_digest, surface_revision_digest: "" };
    }
    if (!ssrManifestMatchesSurface(resolved)) {
      return { manifest_revision_digest: "", surface_revision_digest: "" };
    }
    const refs = globalThis.__mei?.scene_manifest_refs;
    if (!refs || typeof refs !== "object") {
      return { manifest_revision_digest: "", surface_revision_digest: "" };
    }
    return {
      manifest_revision_digest: String(
        refs.revision_digest || refs.manifest_revision_digest || "",
      ).trim(),
      surface_revision_digest: String(refs.surface_revision_digest || "").trim(),
    };
  }

  function pruneRevisionStore(store, key, maxEntries) {
    if (!store || typeof store !== "object") return;
    const limit = Number.isFinite(maxEntries) ? maxEntries : 32;
    const keys = Object.keys(store);
    if (keys.length <= limit) return;
    for (const stale of keys) {
      if (stale === key) continue;
      delete store[stale];
      if (Object.keys(store).length <= limit) break;
    }
  }

  const ViewRevisionOutcome = {
    REFETCH: "refetch",
    ASSEMBLE_LOCAL: "assemble_local",
    LOCAL_MISS: "local_miss",
  };

  function holdingsFromLayerCache(holdings) {
    return (holdings || [])
      .map((row) => ({
        name: String(row?.name || "").trim(),
        artifact_id: String(row?.artifact_id || "").trim(),
        content_hash: String(row?.content_hash || "").trim(),
      }))
      .filter((row) => row.name && row.artifact_id && row.content_hash);
  }

  function revisionsMatchManifest(manifestDigest, localDigest) {
    const a = String(manifestDigest || "").trim();
    const b = String(localDigest || "").trim();
    if (!a || !b) return false;
    return a === b;
  }

  boot.revisionsMatch = revisionsMatch;
  boot.normalizeRevision = normalizeRevision;
  boot.semanticRevisionKey = semanticRevisionKey;
  boot.surfaceComposeKey = surfaceComposeKey;
  boot.surfaceRevisionKey = surfaceRevisionKey;
  boot.resolveComposeKeyCtx = resolveComposeKeyCtx;
  boot.viewRevisionStoreKey = viewRevisionStoreKey;
  boot.readViewRevisionStore = readViewRevisionStore;
  boot.writeViewRevisionStore = writeViewRevisionStore;
  boot.rememberViewRevision = rememberViewRevision;
  boot.readViewRevision = readViewRevision;
  boot.readClientDigests = readClientDigests;
  boot.readSharedManifestDigest = readSharedManifestDigest;
  boot.readSharedManifestSnapshot = readSharedManifestSnapshot;
  boot.pruneRevisionStore = pruneRevisionStore;
  boot.ViewRevisionOutcome = ViewRevisionOutcome;
  boot.holdingsFromLayerCache = holdingsFromLayerCache;
  boot.revisionsMatchManifest = revisionsMatchManifest;
})(window);
