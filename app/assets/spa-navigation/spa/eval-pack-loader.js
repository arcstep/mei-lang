/**
 * Unified Scope Eval Pack loader (E9 v2 main contract).
 * Sources: eval_pack_inline | eval_pack_api | eval_pack_local | jit
 */
(function initEvalPackLoader(global) {
  "use strict";

  const boot = (global.__meiLangBoot = global.__meiLangBoot || {});
  const SCENE_EVAL_PACK_API = "/api/host/scene-eval-pack";
  const LEGACY_BOOTSTRAP_API = "/api/host/scene-bootstrap";
  const BOOTSTRAP_ARTIFACT_LS_PREFIX = "mei:scene-bootstrap:v1:";
  const NO_CLIENT_BOOTSTRAP_REVISION = "__no_client_bootstrap__";

  function evalPackUnifiedEnabled() {
    if (global.__mei?.eval_pack_unified === true) return true;
    try {
      return new URL(global.location.href).searchParams.get("eval_pack") === "unified";
    } catch (_) {
      return false;
    }
  }

  function bootstrapArtifactStorageKey(appId, sceneId, revision) {
    return `${appId}:${sceneId}:${revision || ""}`;
  }

  function normalizeEvalPackPayload(raw) {
    if (!raw || typeof raw !== "object") return null;
    if (raw.status && (raw.clientRevision || raw.client_revision)) {
      return {
        clientRevision: raw.clientRevision || raw.client_revision,
        bootstrapScope: raw.bootstrapScope || raw.bootstrap_scope || raw.scope,
        targetFile: raw.targetFile || raw.target_file || "",
        compileEpoch: raw.compileEpoch || raw.compile_epoch || "",
        dataGeneration: raw.dataGeneration || raw.data_generation || "",
        appId: raw.appId || raw.app_id || "",
        metrics: Array.isArray(raw.metrics) ? raw.metrics : [],
        bootstrapScopes: Array.isArray(raw.bootstrapScopes)
          ? raw.bootstrapScopes
          : Array.isArray(raw.bootstrap_scopes)
            ? raw.bootstrap_scopes
            : [],
        layoutBudgetManifest: raw.layoutBudgetManifest || raw.layout_budget_manifest || null,
        evalPackStatus: raw.status,
        neighborHops: raw.neighborHops ?? raw.neighbor_hops ?? null,
        evalLayerRefs: raw.evalLayerRefs || raw.eval_layer_refs || [],
      };
    }
    return raw;
  }

  function markNoClientBootstrapPack(reason) {
    global.__meiBootstrapNoClientPack = 1;
    global.__meiEvalPackMissReason =
      global.__meiEvalPackMissReason || reason || "no_client_bootstrap";
  }

  function clearNoClientBootstrapPack() {
    delete global.__meiBootstrapNoClientPack;
  }

  function applyEvalPackPayload(payload, options) {
    const opts = options || {};
    const normalized = normalizeEvalPackPayload(payload);
    if (!normalized) return false;
    global.__mei = global.__mei || {};
    if (normalized.clientRevision) global.__mei.client_revision = normalized.clientRevision;
    if (normalized.bootstrapScope) global.__mei.bootstrap_scope = normalized.bootstrapScope;
    if (normalized.targetFile) global.__mei.bootstrap_target_file = normalized.targetFile;
    if (normalized.compileEpoch) {
      global.__mei.bootstrap_compile_epoch = normalized.compileEpoch;
      global.__mei.compile_epoch = normalized.compileEpoch;
    }
    if (normalized.dataGeneration) {
      global.__mei.bootstrap_data_generation = normalized.dataGeneration;
      global.__mei.data_generation = normalized.dataGeneration;
    }
    if (normalized.appId) global.__mei.bootstrap_app_id = normalized.appId;
    if (Array.isArray(normalized.metrics)) global.__mei.bootstrap_metrics = normalized.metrics;
    if (Array.isArray(normalized.bootstrapScopes)) {
      global.__mei.bootstrap_scopes = normalized.bootstrapScopes;
    }
    if (normalized.layoutBudgetManifest) {
      const incoming = normalized.layoutBudgetManifest;
      const existing = global.__mei.layout_budget_manifest;
      // Prefer runtime.plans once applied: merge so bootstrap only fills gaps and
      // never drops Content-host grids (issue_body) that older local caches omit.
      if (
        global.__mei.__layout_budget_source === "runtime.plans" &&
        existing?.entries &&
        typeof existing.entries === "object" &&
        incoming?.entries &&
        typeof incoming.entries === "object"
      ) {
        global.__mei.layout_budget_manifest = {
          revision: existing.revision || incoming.revision,
          entries: { ...incoming.entries, ...existing.entries },
        };
      } else {
        global.__mei.layout_budget_manifest = incoming;
        if (!global.__mei.__layout_budget_source) {
          global.__mei.__layout_budget_source = "eval_pack";
        }
      }
      if (typeof boot.applyLayoutBudgetManifestProjection === "function") {
        boot.applyLayoutBudgetManifestProjection();
      } else if (global.MeiProjectionDepth?.applyLayoutBudgetManifest) {
        global.MeiProjectionDepth.applyLayoutBudgetManifest();
      }
    }
    if (Array.isArray(normalized.evalLayerRefs) && normalized.evalLayerRefs.length > 0) {
      global.__mei.eval_layer_refs = normalized.evalLayerRefs;
    }
    const revision = String(normalized.clientRevision || "").trim();
    const metricCount = Array.isArray(normalized.metrics) ? normalized.metrics.length : 0;
    if (revision === NO_CLIENT_BOOTSTRAP_REVISION || metricCount === 0) {
      markNoClientBootstrapPack(
        revision === NO_CLIENT_BOOTSTRAP_REVISION
          ? "no_client_bootstrap"
          : "empty_eval_pack_metrics",
      );
    } else {
      clearNoClientBootstrapPack();
    }
    global.__meiBootstrapPayloadReady = 1;
    if (opts.source) {
      global.__meiEvalPackSource = opts.source;
    }
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

  function resolveBootstrapRevision(revision) {
    const fromArg = String(revision?.client_revision || revision?.clientRevision || "").trim();
    if (fromArg) return fromArg;
    const el = document.querySelector('meta[name="mei-bootstrap-client-revision"]');
    const fromMeta = el ? String(el.content || "").trim() : "";
    if (fromMeta) return fromMeta;
    return String(global.__mei?.client_revision || "").trim();
  }

  async function fetchEvalPackFromApi(ctx, options) {
    const opts = options || {};
    const appId = ctx?.appId;
    const sceneId = ctx?.sceneId || "home";
    if (!appId) throw new Error("eval pack requires appId");
    const params = new URLSearchParams({
      app: appId,
      scene: sceneId,
      scope: sceneId,
      pack: "unified",
    });
    const clientRevision = resolveBootstrapRevision(opts.revision || {});
    if (clientRevision) params.set("client_revision", clientRevision);
    if (opts.fingerprint) params.set("fingerprint", String(opts.fingerprint));
    if (opts.neighborHops != null) params.set("neighbor_hops", String(opts.neighborHops));
    const endpoint = evalPackUnifiedEnabled() ? SCENE_EVAL_PACK_API : LEGACY_BOOTSTRAP_API;
    const response = await fetch(`${endpoint}?${params.toString()}`, {
      credentials: "same-origin",
      headers: { Accept: "application/json" },
    });
    if (!response.ok) {
      throw new Error(`eval pack failed: ${response.status}`);
    }
    const payload = await response.json();
    const normalized = normalizeEvalPackPayload(payload) || payload;
    applyEvalPackPayload(normalized, { source: "eval_pack_api" });
    global.__meiEvalPackFromApi = 1;
    if (clientRevision || normalized?.clientRevision) {
      writeLocalBootstrapArtifact(
        appId,
        sceneId,
        clientRevision || normalized.clientRevision,
        normalized,
      );
    }
    return normalized;
  }

  async function ensureEvalPackPayload(ctx, revision, options) {
    const opts = options || {};
    const appId = ctx?.appId;
    const sceneId = ctx?.sceneId;
    const clientRevision = resolveBootstrapRevision(revision);
    if (!appId || !sceneId) return null;
    if (clientRevision === NO_CLIENT_BOOTSTRAP_REVISION) {
      markNoClientBootstrapPack("no_client_bootstrap");
      global.__mei = global.__mei || {};
      global.__mei.client_revision = NO_CLIENT_BOOTSTRAP_REVISION;
      if (!Array.isArray(global.__mei.bootstrap_metrics)) {
        global.__mei.bootstrap_metrics = [];
      }
      global.__meiBootstrapPayloadReady = 1;
      try {
        document.dispatchEvent(new CustomEvent("mei-bootstrap-ready"));
      } catch (_) {}
      return global.__mei || null;
    }
    const currentScope = String(global.__mei?.bootstrap_scope || "").trim();
    const currentAppId = String(global.__mei?.bootstrap_app_id || "").trim();
    if (
      global.__meiBootstrapPayloadReady &&
      currentScope === sceneId &&
      (!currentAppId || currentAppId === appId) &&
      !opts.force
    ) {
      return global.__mei;
    }
    const inline = document.getElementById("mei-client-bootstrap");
    if (inline && inline.textContent) {
      try {
        const payload = JSON.parse(inline.textContent || "{}");
        applyEvalPackPayload(payload, { source: "eval_pack_inline" });
        if (clientRevision) {
          writeLocalBootstrapArtifact(appId, sceneId, clientRevision, payload);
        }
        return payload;
      } catch (_) {}
    }
    if (clientRevision) {
      const cached = readLocalBootstrapArtifact(appId, sceneId, clientRevision);
      if (cached) {
        applyEvalPackPayload(cached, { source: "eval_pack_local" });
        global.__meiBootstrapFromLocalStorage = 1;
        return cached;
      }
    }
    return fetchEvalPackFromApi(ctx, { revision, fingerprint: opts.fingerprint, neighborHops: opts.neighborHops });
  }

  function seedEvalPackRuntimeCache() {
    const sourceMeta = global.__meiBootstrapFromLocalStorage
      ? "eval_pack_local"
      : global.__meiEvalPackFromApi
        ? "eval_pack_api"
        : global.__meiEvalPackSource || "eval_pack_inline";
    if (boot.evalStore?.seedPack) {
      return boot.evalStore.seedPack(global.__mei, { source: sourceMeta });
    }
    if (typeof seedFromBootstrap !== "function") {
      return 0;
    }
    const count = seedFromBootstrap(global.__mei);
    if (count > 0) {
      global.__meiBootstrapSeeded = true;
      global.__meiBootstrapSeedCount = count;
      delete global.__meiBootstrapSeedError;
      global.__meiEvalPackSource = sourceMeta;
    }
    return count;
  }

  async function ensureEvalPackSeeded(ctx, revision, options) {
    const opts = options || {};
    const payload = await ensureEvalPackPayload(ctx, revision || {}, options);
    const count = seedEvalPackRuntimeCache();
    // Neighbor scope warmup must not compete with cold-start critical path.
    if (opts.prefetchNeighbors !== false) {
      const appId = ctx?.appId;
      const run = () => {
        void prefetchNeighborEvalPacks(appId, payload);
      };
      if (typeof global.requestIdleCallback === "function") {
        global.requestIdleCallback(run, { timeout: 2500 });
      } else {
        global.setTimeout(run, 0);
      }
    }
    return count;
  }

  async function prefetchNeighborEvalPacks(appId, payload) {
    const scopes = Array.isArray(payload?.bootstrapScopes) ? payload.bootstrapScopes : [];
    if (!appId || scopes.length === 0) return;
    for (const entry of scopes) {
      const neighborScope = String(entry?.bootstrapScope || entry?.bootstrap_scope || "").trim();
      const neighborRevision = String(entry?.clientRevision || entry?.client_revision || "").trim();
      if (!neighborScope) continue;
      const currentScope = String(global.__mei?.bootstrap_scope || "").trim();
      if (neighborScope === currentScope) continue;
      try {
        await ensureEvalPackSeeded(
          { appId, sceneId: neighborScope },
          { client_revision: neighborRevision },
          { prefetchNeighbors: false },
        );
      } catch (_) {
        /* neighbor warmup is best-effort */
      }
    }
  }

  async function fetchJitEvalPack(ctx, { fingerprint = "", neighborHops = null } = {}) {
    const payload = await fetchEvalPackFromApi(ctx, {
      fingerprint,
      neighborHops,
      revision: {},
    });
    global.__meiEvalPackSource = fingerprint ? "jit" : "eval_pack_api";
    return seedEvalPackRuntimeCache();
  }

  boot.evalPackLoader = {
    unifiedEnabled: evalPackUnifiedEnabled,
    applyEvalPackPayload,
    ensureEvalPackPayload,
    ensureEvalPackSeeded,
    seedEvalPackRuntimeCache,
    fetchJitEvalPack,
    fetchEvalPackFromApi,
    normalizeEvalPackPayload,
  };
  boot.applyEvalPackPayload = applyEvalPackPayload;
  boot.ensureEvalPackPayload = ensureEvalPackPayload;
  boot.ensureEvalPackSeeded = ensureEvalPackSeeded;
  boot.fetchEvalPackFromApi = fetchEvalPackFromApi;
})(typeof window !== "undefined" ? window : globalThis);
