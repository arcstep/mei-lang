/**
 * Shared revision contract for access-like scene shell and Build workspace fragments.
 */
(function initRevisionContract(global) {
  "use strict";

  const boot = (global.__meiLangBoot = global.__meiLangBoot || {});

  function normalizeRevision(revision) {
    if (!revision || typeof revision !== "object") return revision;
    return {
      ...revision,
      revision_digest: String(
        revision.revision_digest || revision.revisionDigest || "",
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

  boot.revisionsMatch = revisionsMatch;
  boot.normalizeRevision = normalizeRevision;
  boot.surfaceRevisionKey = surfaceRevisionKey;
  boot.pruneRevisionStore = pruneRevisionStore;
})(window);
