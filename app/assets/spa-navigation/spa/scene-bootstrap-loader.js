/**
 * Scene bootstrap loader — thin compatibility wrapper over eval-pack-loader (E9).
 */
(function initSceneBootstrapLoader(global) {
  "use strict";

  const boot = (global.__meiLangBoot = global.__meiLangBoot || {});
  const loader = () => boot.evalPackLoader;

  function applyBootstrapPayload(payload) {
    if (loader()?.applyEvalPackPayload) {
      return loader().applyEvalPackPayload(payload, { source: payload?.__source });
    }
    return false;
  }

  function tryRestoreBootstrapFromLocalStorage(appId, sceneId, clientRevision) {
    return loader()?.ensureEvalPackPayload?.(
      { appId, sceneId },
      { client_revision: clientRevision },
    );
  }

  async function ensureSceneBootstrapPayload(ctx, revision) {
    if (loader()?.ensureEvalPackPayload) {
      return loader().ensureEvalPackPayload(ctx, revision || {});
    }
    return null;
  }

  async function ensureBootstrapSeeded(ctx, revision) {
    if (loader()?.ensureEvalPackSeeded) {
      return loader().ensureEvalPackSeeded(ctx, revision || {});
    }
    return 0;
  }

  function seedBootstrapRuntimeCache() {
    if (loader()?.seedEvalPackRuntimeCache) {
      return loader().seedEvalPackRuntimeCache();
    }
    return 0;
  }

  async function fetchJitEvalPack(ctx, options) {
    if (loader()?.fetchJitEvalPack) {
      return loader().fetchJitEvalPack(ctx, options || {});
    }
    return 0;
  }

  function applyLayoutBudgetManifestProjection(doc) {
    if (global.MeiProjectionDepth?.applyLayoutBudgetManifest) {
      global.MeiProjectionDepth.applyLayoutBudgetManifest(doc);
    }
  }

  function resolveBootstrapAppId() {
    const mei = global.__mei || {};
    const direct = String(
      global.__meiRuntimeAppId || mei.bootstrap_app_id || mei.app_id || "",
    ).trim();
    if (direct) return direct;
    const host =
      document.querySelector("[data-mei-app-id]") ||
      document.querySelector("[data-app-id]") ||
      document.querySelector("[data-app]");
    if (!(host instanceof HTMLElement)) return "";
    return String(
      host.dataset.meiAppId || host.dataset.appId || host.dataset.app || "",
    ).trim();
  }

  function readBootstrapMeta(name) {
    const el = document.querySelector(`meta[name="${name}"]`);
    return el ? String(el.content || "").trim() : "";
  }

  function isBootstrapRevisionOnly() {
    return readBootstrapMeta("mei-bootstrap-inlined") === "0";
  }

  function resolveActivationSceneId(detail) {
    return String(
      detail?.scope || detail?.sceneId || detail?.boardSceneId || detail?.pageSceneId || "",
    ).trim();
  }

  function dispatchScopeActivation(detail = {}) {
    const sceneId = resolveActivationSceneId(detail);
    const appId = String(detail?.appId || resolveBootstrapAppId() || "").trim();
    if (!sceneId) return false;
    try {
      window.dispatchEvent(
        new CustomEvent("meilang:scope-activation", {
          detail: {
            ...detail,
            scope: sceneId,
            sceneId: String(detail?.sceneId || sceneId).trim() || sceneId,
            appId,
            source: String(detail?.source || "runtime").trim() || "runtime",
          },
        }),
      );
      return true;
    } catch (_) {
      return false;
    }
  }

  const inflightScopes = new Set();

  async function hydrateBootstrapForActivatedScope(event) {
    const detail = event?.detail && typeof event.detail === "object" ? event.detail : {};
    const sceneId = resolveActivationSceneId(detail);
    const appId = String(detail.appId || resolveBootstrapAppId() || "").trim();
    if (!appId) return;
    const currentScope = String(global.__mei?.bootstrap_scope || "").trim();
    const currentAppId = String(global.__mei?.bootstrap_app_id || "").trim();
    if (
      sceneId &&
      global.__meiBootstrapPayloadReady &&
      currentScope === sceneId &&
      (!currentAppId || currentAppId === appId)
    ) {
      return;
    }
    const inflightKey = `${appId}:${sceneId}`;
    if (!sceneId || inflightScopes.has(inflightKey)) return;
    inflightScopes.add(inflightKey);
    try {
      await ensureBootstrapSeeded({ appId, sceneId }, {});
    } catch (_) {
      /* allow next activation to retry */
    } finally {
      inflightScopes.delete(inflightKey);
    }
  }

  boot.ensureSceneBootstrapPayload = ensureSceneBootstrapPayload;
  boot.ensureBootstrapSeeded = ensureBootstrapSeeded;
  boot.seedBootstrapRuntimeCache = seedBootstrapRuntimeCache;
  boot.fetchJitEvalPack = fetchJitEvalPack;
  boot.applyBootstrapPayload = applyBootstrapPayload;
  boot.tryRestoreBootstrapFromLocalStorage = tryRestoreBootstrapFromLocalStorage;
  boot.isBootstrapRevisionOnly = isBootstrapRevisionOnly;
  boot.readBootstrapMeta = readBootstrapMeta;
  boot.dispatchScopeActivation = dispatchScopeActivation;
  boot.applyLayoutBudgetManifestProjection = applyLayoutBudgetManifestProjection;
  global.addEventListener("meilang:scope-activation", hydrateBootstrapForActivatedScope);

  if (global.__meiBootstrapFromLocalStorage && global.__meiBootstrapPayloadReady) {
    seedBootstrapRuntimeCache();
  }
})(typeof window !== "undefined" ? window : globalThis);
