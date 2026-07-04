  const SCENE_BOOTSTRAP_API = "/api/host/scene-bootstrap";
  const BOOTSTRAP_ARTIFACT_LS_PREFIX = "mei:scene-bootstrap:v1:";
  const NO_CLIENT_BOOTSTRAP_REVISION = "__no_client_bootstrap__";

  function bootstrapArtifactStorageKey(appId, sceneId, revision) {
    return `${appId}:${sceneId}:${revision || ""}`;
  }

  function applyBootstrapPayload(payload) {
    if (!payload || typeof payload !== "object") return false;
    window.__mei = window.__mei || {};
    if (payload.clientRevision) window.__mei.client_revision = payload.clientRevision;
    if (payload.bootstrapScope) window.__mei.bootstrap_scope = payload.bootstrapScope;
    if (payload.targetFile) window.__mei.bootstrap_target_file = payload.targetFile;
    if (payload.compileEpoch) window.__mei.bootstrap_compile_epoch = payload.compileEpoch;
    if (payload.dataGeneration) window.__mei.bootstrap_data_generation = payload.dataGeneration;
    if (payload.appId) window.__mei.bootstrap_app_id = payload.appId;
    if (Array.isArray(payload.metrics)) window.__mei.bootstrap_metrics = payload.metrics;
    if (Array.isArray(payload.bootstrapScopes)) {
      window.__mei.bootstrap_scopes = payload.bootstrapScopes;
    }
    if (payload.layoutBudgetManifest) {
      window.__mei.layout_budget_manifest = payload.layoutBudgetManifest;
    }
    window.__meiBootstrapPayloadReady = 1;
    try {
      document.dispatchEvent(new CustomEvent("mei-bootstrap-ready"));
    } catch (_) {}
    return true;
  }

  function readLocalBootstrapArtifact(appId, sceneId, revision) {
    try {
      const raw = localStorage.getItem(
        `${BOOTSTRAP_ARTIFACT_LS_PREFIX}${bootstrapArtifactStorageKey(appId, sceneId, revision)}`,
      );
      if (!raw) return null;
      return JSON.parse(raw);
    } catch (_) {
      return null;
    }
  }

  function writeLocalBootstrapArtifact(appId, sceneId, revision, payload) {
    try {
      localStorage.setItem(
        `${BOOTSTRAP_ARTIFACT_LS_PREFIX}${bootstrapArtifactStorageKey(appId, sceneId, revision)}`,
        JSON.stringify(payload),
      );
      return true;
    } catch (_) {
      return false;
    }
  }

  function resolveBootstrapAppId() {
    const mei = window.__mei || {};
    const direct = String(
      window.__meiRuntimeAppId || mei.bootstrap_app_id || mei.app_id || "",
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

  async function ensureSceneBootstrapPayload(ctx, revision) {
    const appId = ctx?.appId;
    const sceneId = ctx?.sceneId;
    const clientRevision = revision?.client_revision;
    if (!appId || !sceneId) return null;
    if (clientRevision === NO_CLIENT_BOOTSTRAP_REVISION) {
      window.__meiBootstrapPayloadReady = 1;
      return window.__mei || null;
    }
    const currentScope = String(window.__mei?.bootstrap_scope || "").trim();
    const currentAppId = String(window.__mei?.bootstrap_app_id || "").trim();
    if (
      window.__meiBootstrapPayloadReady &&
      currentScope === sceneId &&
      (!currentAppId || currentAppId === appId)
    ) {
      return window.__mei;
    }
    const inline = document.getElementById("mei-client-bootstrap");
    if (inline && inline.textContent) {
      try {
        const payload = JSON.parse(inline.textContent || "{}");
        applyBootstrapPayload(payload);
        if (clientRevision) {
          writeLocalBootstrapArtifact(appId, sceneId, clientRevision, payload);
        }
        return payload;
      } catch (_) {}
    }
    if (clientRevision) {
      const cached = readLocalBootstrapArtifact(appId, sceneId, clientRevision);
      if (cached) {
        applyBootstrapPayload(cached);
        return cached;
      }
    }
    const params = new URLSearchParams({ app: appId, scene: sceneId });
    const response = await fetch(`${SCENE_BOOTSTRAP_API}?${params.toString()}`, {
      credentials: "same-origin",
      headers: { Accept: "application/json" },
    });
    if (!response.ok) {
      throw new Error(`scene bootstrap failed: ${response.status}`);
    }
    const payload = await response.json();
    applyBootstrapPayload(payload);
    if (clientRevision || payload?.clientRevision) {
      writeLocalBootstrapArtifact(
        appId,
        sceneId,
        clientRevision || payload.clientRevision,
        payload,
      );
    }
    return payload;
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
    const currentScope = String(window.__mei?.bootstrap_scope || "").trim();
    const currentAppId = String(window.__mei?.bootstrap_app_id || "").trim();
    if (
      sceneId &&
      window.__meiBootstrapPayloadReady &&
      currentScope === sceneId &&
      (!currentAppId || currentAppId === appId)
    ) {
      return;
    }
    const inflightKey = `${appId}:${sceneId}`;
    if (!sceneId || inflightScopes.has(inflightKey)) return;
    inflightScopes.add(inflightKey);
    try {
      await ensureSceneBootstrapPayload({ appId, sceneId }, {});
    } catch (_) {
      /* allow next activation to retry */
    } finally {
      inflightScopes.delete(inflightKey);
    }
  }

  boot.ensureSceneBootstrapPayload = ensureSceneBootstrapPayload;
  boot.applyBootstrapPayload = applyBootstrapPayload;
  boot.dispatchScopeActivation = dispatchScopeActivation;
  window.addEventListener("meilang:scope-activation", hydrateBootstrapForActivatedScope);
