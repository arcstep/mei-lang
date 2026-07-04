  const SCENE_REVISION_API = "/api/host/scene-revision";
  const SCENE_REVISION_STORE_KEY = "mei-scene-revisions";
  const SCENE_REVISION_LS_KEY = "mei:scene-revisions:v1";

  function readSceneRevisionStore() {
    try {
      const raw = sessionStorage.getItem(SCENE_REVISION_STORE_KEY);
      if (raw) return JSON.parse(raw);
    } catch (_) {}
    try {
      const raw = localStorage.getItem(SCENE_REVISION_LS_KEY);
      return raw ? JSON.parse(raw) : {};
    } catch (_) {
      return {};
    }
  }

  function writeSceneRevisionStore(store) {
    const payload = JSON.stringify(store || {});
    try {
      sessionStorage.setItem(SCENE_REVISION_STORE_KEY, payload);
    } catch (_) {}
    try {
      localStorage.setItem(SCENE_REVISION_LS_KEY, payload);
    } catch (_) {}
  }

  function parseAccessSceneContext(urlLike) {
    try {
      const url = new URL(urlLike, window.location.href);
      const match = url.pathname.match(
        /^\/apps\/(?:app|access|run|presentation|slides|copilot)\/([^/]+)\/scene\/([^/]+)/,
      );
      if (!match) return null;
      const mode = url.pathname.split("/")[2] || "app";
      return {
        appId: decodeURIComponent(match[1]),
        sceneId: decodeURIComponent(match[2]),
        mode,
        chrome: String(url.searchParams.get("chrome") || "").trim().toLowerCase(),
        dataMode: String(url.searchParams.get("data_mode") || "").trim().toLowerCase(),
        reviewProjection: String(url.searchParams.get("review_projection") || "")
          .trim()
          .toLowerCase(),
        url: url.href,
      };
    } catch (_) {
      return null;
    }
  }

  function sceneRevisionCacheKey(ctx) {
    return boot.surfaceRevisionKey({
      surface: ctx.mode || "app",
      app_id: ctx.appId,
      scene_id: ctx.sceneId,
      data_mode: (() => {
        try {
          return String(
            new URL(ctx.url || window.location.href).searchParams.get("data_mode") || "",
          )
            .trim()
            .toLowerCase();
        } catch (_) {
          return "";
        }
      })(),
      review_projection: (() => {
        try {
          return String(
            new URL(ctx.url || window.location.href).searchParams.get("review_projection") || "",
          )
            .trim()
            .toLowerCase();
        } catch (_) {
          return "";
        }
      })(),
      chrome: (() => {
        try {
          return String(new URL(ctx.url || window.location.href).searchParams.get("chrome") || "")
            .trim()
            .toLowerCase();
        } catch (_) {
          return "";
        }
      })(),
    });
  }

  function readCachedSceneRevision(ctx) {
    const key = sceneRevisionCacheKey(ctx);
    if (!key) return null;
    const store = readSceneRevisionStore();
    const revision = store[key] || null;
    return typeof boot.normalizeRevision === "function"
      ? boot.normalizeRevision(revision)
      : revision;
  }

  function rememberSceneRevision(ctx, revision) {
    const key = sceneRevisionCacheKey(ctx);
    if (!key || !revision) return;
    const normalized =
      typeof boot.normalizeRevision === "function"
        ? boot.normalizeRevision(revision)
        : revision;
    const store = readSceneRevisionStore();
    store[key] = normalized;
    if (typeof boot.pruneRevisionStore === "function") {
      boot.pruneRevisionStore(store, key, 48);
    }
    writeSceneRevisionStore(store);
    if (typeof boot.cacheDiagTrace === "function") {
      boot.cacheDiagTrace("revision-remembered", {
        key,
        revision_digest: normalized.revision_digest,
      });
    }
  }

  function readSsrEmbeddedSceneRevision() {
    const digest = String(
      document.querySelector('meta[name="mei-scene-revision-digest"]')?.getAttribute("content") ||
        "",
    ).trim();
    if (!digest) return null;
    const cacheKey = String(
      document.querySelector('meta[name="mei-scene-cache-key"]')?.getAttribute("content") || "",
    ).trim();
    const clientRevision = String(window.__mei?.client_revision || "").trim();
    const revision = {
      revision_digest: digest,
      cache_key: cacheKey || undefined,
    };
    if (clientRevision) {
      revision.client_revision = clientRevision;
    }
    return typeof boot.normalizeRevision === "function"
      ? boot.normalizeRevision(revision)
      : revision;
  }

  function resolveRevisionWithoutNetwork(ctx, snapshotRevision) {
    const cached = readCachedSceneRevision(ctx);
    const ssr = readSsrEmbeddedSceneRevision();
    const candidates = [cached, snapshotRevision, ssr].filter(Boolean);
    if (typeof boot.cacheDiagTrace === "function") {
      boot.cacheDiagTrace("revision-local-candidates", {
        cached: !!cached,
        snapshotRevision: !!snapshotRevision,
        ssr: !!ssr,
      });
    }
    if (ssr) {
      for (const candidate of candidates) {
        if (
          candidate &&
          typeof boot.revisionsMatch === "function" &&
          boot.revisionsMatch(candidate, ssr)
        ) {
          window.__meiRevisionSkippedNetwork = 1;
          if (typeof boot.cacheDiagTrace === "function") {
            boot.cacheDiagTrace("revision-skip-network", {
              reason: "ssr-digest-match",
              revision_digest: ssr.revision_digest,
            });
          }
          return typeof boot.normalizeRevision === "function"
            ? boot.normalizeRevision(candidate)
            : candidate;
        }
      }
    }
    if (cached?.revision_digest) {
      window.__meiRevisionSkippedNetwork = 1;
      if (typeof boot.cacheDiagTrace === "function") {
        boot.cacheDiagTrace("revision-skip-network", {
          reason: "cached-revision",
          revision_digest: cached.revision_digest,
        });
      }
      return cached;
    }
    return null;
  }

  async function fetchSceneRevision(ctx, options) {
    const opts = options || {};
    let snapshotRevision = null;
    if (opts.preloadSnapshotRevision && typeof boot.loadSceneShellSnapshot === "function") {
      const snapshot = await boot.loadSceneShellSnapshot(ctx);
      snapshotRevision = snapshot?.revision || null;
    }
    if (opts.skipRemoteWhenValid) {
      const local = resolveRevisionWithoutNetwork(ctx, snapshotRevision);
      if (local) return local;
    }
    const params = new URLSearchParams({
      app: ctx.appId,
      scene: ctx.sceneId,
      mode: ctx.mode || "app",
    });
    try {
      const url = new URL(ctx.url || window.location.href);
      const dataMode = String(url.searchParams.get("data_mode") || "").trim();
      const reviewProjection = String(url.searchParams.get("review_projection") || "").trim();
      const chrome = String(url.searchParams.get("chrome") || "").trim();
      if (dataMode) params.set("data_mode", dataMode);
      if (reviewProjection) params.set("review_projection", reviewProjection);
      if (chrome) params.set("chrome", chrome);
    } catch (_) {}
    const controller = opts.signal ? null : new AbortController();
    const signal = opts.signal || controller?.signal;
    const timer =
      controller && Number.isFinite(opts.timeoutMs)
        ? setTimeout(() => controller.abort(), opts.timeoutMs)
        : null;
    try {
      if (typeof boot.cacheDiagTrace === "function") {
        boot.cacheDiagTrace("revision-fetch-network", { app: ctx.appId, scene: ctx.sceneId });
      }
      const response = await fetch(`${SCENE_REVISION_API}?${params.toString()}`, {
        credentials: "same-origin",
        headers: { Accept: "application/json" },
        signal,
      });
      if (!response.ok) {
        throw new Error(`scene revision failed: ${response.status}`);
      }
      const revision = await response.json();
      rememberSceneRevision(ctx, revision);
      return typeof boot.normalizeRevision === "function"
        ? boot.normalizeRevision(revision)
        : revision;
    } finally {
      if (timer) clearTimeout(timer);
    }
  }

  boot.parseAccessSceneContext = parseAccessSceneContext;
  boot.fetchSceneRevision = fetchSceneRevision;
  boot.sceneRevisionCacheKey = sceneRevisionCacheKey;
  boot.readCachedSceneRevision = readCachedSceneRevision;
  boot.rememberSceneRevision = rememberSceneRevision;
  boot.readSsrEmbeddedSceneRevision = readSsrEmbeddedSceneRevision;
  boot.resolveRevisionWithoutNetwork = resolveRevisionWithoutNetwork;
