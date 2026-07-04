/**
 * Shared revision contract for access-like scene shell and Build workspace fragments.
 */
(function initRevisionContract(global) {
  "use strict";

  const boot = (global.__meiLangBoot = global.__meiLangBoot || {});

  function revisionsMatch(localRevision, remoteRevision) {
    if (!localRevision || !remoteRevision) return false;
    if (localRevision.revision_digest && remoteRevision.revision_digest) {
      return localRevision.revision_digest === remoteRevision.revision_digest;
    }
    if (localRevision.cache_key && remoteRevision.cache_key) {
      return localRevision.cache_key === remoteRevision.cache_key;
    }
    return (
      localRevision.registry_revision === remoteRevision.registry_revision &&
      localRevision.client_revision === remoteRevision.client_revision &&
      localRevision.data_generation === remoteRevision.data_generation &&
      (localRevision.scene_bundle_revision || "") ===
        (remoteRevision.scene_bundle_revision || "") &&
      (localRevision.draft_digest || "") === (remoteRevision.draft_digest || "")
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
  boot.surfaceRevisionKey = surfaceRevisionKey;
  boot.pruneRevisionStore = pruneRevisionStore;
})(window);
