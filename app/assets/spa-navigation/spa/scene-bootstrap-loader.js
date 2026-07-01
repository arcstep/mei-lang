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

  async function ensureSceneBootstrapPayload(ctx, revision) {
    const appId = ctx?.appId;
    const sceneId = ctx?.sceneId;
    const clientRevision = revision?.client_revision;
    if (!appId || !sceneId) return null;
    if (clientRevision === NO_CLIENT_BOOTSTRAP_REVISION) {
      window.__meiBootstrapPayloadReady = 1;
      return window.__mei || null;
    }
    if (window.__meiBootstrapSeeded && window.__meiBootstrapPayloadReady) {
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

  boot.ensureSceneBootstrapPayload = ensureSceneBootstrapPayload;
  boot.applyBootstrapPayload = applyBootstrapPayload;
