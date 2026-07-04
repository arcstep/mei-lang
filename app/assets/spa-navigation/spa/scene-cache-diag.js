/**
 * Client cache diagnostics — enable with ?mei_cache_diag=1 or localStorage mei:cache-diag=1
 *
 * Console:
 *   await __meiCacheDiag.inspect()
 *   __meiCacheDiag.flags()
 *   __meiCacheDiag.events
 */
(function initSceneCacheDiag(global) {
  "use strict";

  const boot = (global.__meiLangBoot = global.__meiLangBoot || {});
  const LS_FLAG = "mei:cache-diag";
  const MAX_EVENTS = 48;

  function cacheDiagEnabled() {
    try {
      if (global.localStorage.getItem(LS_FLAG) === "1") return true;
      return new URL(global.location.href).searchParams.get("mei_cache_diag") === "1";
    } catch (_) {
      return false;
    }
  }

  function parseBuildCacheContext(urlLike) {
    try {
      const url = new URL(urlLike, global.location.href);
      const parts = url.pathname.split("/").filter(Boolean);
      if (parts[0] !== "apps" || parts[1] !== "build" || !parts[2]) return null;
      return {
        surface: "build",
        appId: decodeURIComponent(parts[2]),
        url: url.href,
        node: String(url.searchParams.get("node") || "").trim(),
        tab: String(url.searchParams.get("tab") || "").trim(),
        dataMode: String(url.searchParams.get("data_mode") || "").trim().toLowerCase(),
        reviewProjection: String(url.searchParams.get("review_projection") || "")
          .trim()
          .toLowerCase(),
      };
    } catch (_) {
      return null;
    }
  }

  function resolveCacheContext(ctxLike) {
    if (ctxLike) return ctxLike;
    if (typeof boot.parseAccessSceneContext === "function") {
      const accessCtx = boot.parseAccessSceneContext(global.location.href);
      if (accessCtx) return accessCtx;
    }
    return parseBuildCacheContext(global.location.href);
  }

  function trace(event, detail) {
    const diag = (global.__meiCacheDiag = global.__meiCacheDiag || { events: [] });
    const entry = {
      at: new Date().toISOString(),
      event: String(event || "unknown"),
      detail: detail || {},
    };
    diag.events.push(entry);
    if (diag.events.length > MAX_EVENTS) {
      diag.events.splice(0, diag.events.length - MAX_EVENTS);
    }
    if (cacheDiagEnabled()) {
      try {
        console.info("[mei-cache-diag]", entry.event, entry.detail);
      } catch (_) {}
    }
    return entry;
  }

  function readFlags() {
    return {
      shellRestoredFromCache: !!global.__meiShellRestoredFromCache,
      shellRestoredFromFragment: !!global.__meiShellRestoredFromFragment,
      buildPreviewRestoredFromCache: !!global.__meiBuildPreviewRestoredFromCache,
      bootstrapPayloadReady: !!global.__meiBootstrapPayloadReady,
      revisionSkippedNetwork: !!global.__meiRevisionSkippedNetwork,
      bootstrapFromLocalStorage: !!global.__meiBootstrapFromLocalStorage,
    };
  }

  async function inspect(ctxLike) {
    const hostBoot = global.__meiLangBoot || globalThis.__meiLangBoot || boot;
    const ctx = resolveCacheContext(ctxLike);
    const isBuild = ctx?.surface === "build";
    const shellKey =
      !isBuild && typeof boot.snapshotStorageKey === "function" && ctx
        ? boot.snapshotStorageKey(ctx)
        : null;
    const revisionKey =
      !isBuild && typeof boot.sceneRevisionCacheKey === "function" && ctx
        ? boot.sceneRevisionCacheKey(ctx)
        : isBuild && typeof hostBoot.buildFragmentRevisionCacheKey === "function"
          ? hostBoot.buildFragmentRevisionCacheKey(ctx.url)
          : isBuild && global.MeiBuildFragmentRevision?.buildFragmentRevisionCacheKey
            ? global.MeiBuildFragmentRevision.buildFragmentRevisionCacheKey(ctx.url)
            : null;
    let snapshot = null;
    if (!isBuild && ctx && typeof boot.loadSceneShellSnapshot === "function") {
      snapshot = await boot.loadSceneShellSnapshot(ctx);
    }
    const cachedRevision =
      !isBuild && ctx && typeof boot.readCachedSceneRevision === "function"
        ? boot.readCachedSceneRevision(ctx)
        : null;
    const ssrRevision =
      typeof boot.readSsrEmbeddedSceneRevision === "function"
        ? boot.readSsrEmbeddedSceneRevision()
        : null;
    const buildRevision =
      isBuild && ctx?.url
        ? hostBoot.readBuildFragmentRevision?.(ctx.url) ||
          global.MeiBuildFragmentRevision?.readBuildFragmentRevision?.(ctx.url) ||
          null
        : null;
    const buildFragment =
      isBuild && ctx?.url && buildRevision
        ? hostBoot.readBuildFragmentHtml?.(ctx.url, buildRevision) ||
          global.MeiBuildFragmentRevision?.readBuildFragmentHtml?.(ctx.url, buildRevision) ||
          null
        : null;
    const revisionMatch =
      snapshot?.revision &&
      cachedRevision &&
      typeof boot.revisionsMatch === "function"
        ? boot.revisionsMatch(snapshot.revision, cachedRevision)
        : false;
    const ssrMatch =
      snapshot?.revision &&
      ssrRevision &&
      typeof boot.revisionsMatch === "function"
        ? boot.revisionsMatch(snapshot.revision, ssrRevision)
        : false;
    const report = {
      url: global.location.href,
      ctx,
      surface: isBuild ? "build" : ctx ? "access" : "unknown",
      keys: { shell: shellKey, revision: revisionKey },
      flags: readFlags(),
      ssrRevision,
      cachedRevision,
      buildRevision,
      buildFragment: buildFragment
        ? {
            previewHtmlBytes: String(buildFragment.preview_html || "").length,
            node: buildFragment.node || "",
            revision: buildFragment.revision || buildRevision,
          }
        : null,
      snapshot: snapshot
        ? {
            key: snapshot.key,
            savedAtMs: snapshot.savedAtMs,
            revision: snapshot.revision,
            shellHtmlBytes: String(snapshot.shellHtml || "").length,
          }
        : null,
      revisionMatch,
      ssrMatch,
      bootApi: {
        inspectSceneClientCache: typeof hostBoot.inspectSceneClientCache === "function",
        fetchBuildFragmentRevision: typeof hostBoot.fetchBuildFragmentRevision === "function",
        meiBuildFragmentRevision: !!global.MeiBuildFragmentRevision?.fetchBuildFragmentRevision,
        tryRestoreBuildPreviewFromCache:
          typeof global.MeiBuildNavigation?.tryRestoreBuildPreviewFromCache === "function",
      },
      events: (global.__meiCacheDiag?.events || []).slice(-12),
    };
    trace("inspect", report);
    return report;
  }

  function installFetchTap() {
    if (global.__meiCacheDiagFetchTap) return;
    global.__meiCacheDiagFetchTap = true;
    const nativeFetch = global.fetch.bind(global);
    global.fetch = function meiCacheDiagFetch(input, init) {
      const url = String(input?.url || input || "");
      if (
        url.includes("/api/host/scene-revision") ||
        url.includes("/api/host/scene-bootstrap") ||
        url.includes("/api/host/scene-fragment") ||
        url.includes("/api/build/fragment-revision") ||
        url.includes("/api/build/workspace-fragment")
      ) {
        trace("fetch", { url, method: init?.method || "GET" });
      }
      return nativeFetch(input, init);
    };
  }

  function publishApi() {
    const api = {
      inspect,
      trace,
      flags: readFlags,
      enabled: cacheDiagEnabled,
      get events() {
        return global.__meiCacheDiag?.events || [];
      },
    };
    global.__meiCacheDiag = Object.assign(global.__meiCacheDiag || { events: [] }, api);
    boot.cacheDiagEnabled = cacheDiagEnabled;
    boot.cacheDiagTrace = trace;
    boot.inspectSceneClientCache = inspect;
    boot.parseBuildCacheContext = parseBuildCacheContext;
  }

  publishApi();

  if (cacheDiagEnabled()) {
    installFetchTap();
    trace("diag-enabled", { url: global.location.href });
    global.addEventListener("load", () => {
      void inspect();
    });
  }
})(window);
