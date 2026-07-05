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

  function resolveCacheContext(ctxLike) {
    if (ctxLike) return ctxLike;
    if (typeof boot.parseViewContext === "function") {
      const viewCtx = boot.parseViewContext(global.location.href);
      if (viewCtx) return viewCtx;
    }
    if (typeof boot.parseAccessSceneContext === "function") {
      const accessCtx = boot.parseAccessSceneContext(global.location.href);
      if (accessCtx) return accessCtx;
    }
    return null;
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
      revisionSkippedNetwork:
        !!global.__meiRevisionSkippedNetwork || !!global.__meiBuildRevisionSkippedNetwork,
      bootstrapFromLocalStorage: !!global.__meiBootstrapFromLocalStorage,
    };
  }

  async function inspect(ctxLike) {
    const hostBoot = global.__meiLangBoot || globalThis.__meiLangBoot || boot;
    const ctx = resolveCacheContext(ctxLike);
    const surface = String(ctx?.surface || ctx?.mode || "unknown").trim().toLowerCase() || "unknown";
    const shellKey =
      typeof boot.snapshotStorageKey === "function" && ctx ? boot.snapshotStorageKey(ctx) : null;
    const revisionKey =
      typeof boot.sceneRevisionCacheKey === "function" && ctx
        ? boot.sceneRevisionCacheKey(ctx)
        : null;
    const cachedRevision =
      ctx && typeof boot.readCachedSceneRevision === "function"
        ? boot.readCachedSceneRevision(ctx)
        : null;
    const ssrRevision =
      typeof boot.readSsrEmbeddedSceneRevision === "function"
        ? boot.readSsrEmbeddedSceneRevision()
        : null;
    const report = {
      url: global.location.href,
      ctx,
      surface,
      thin_shell:
        (typeof isRevisionFirstShellPage === "function" && isRevisionFirstShellPage()) ||
        globalThis.__mei?.thin_shell === true,
      artifact_hits: globalThis.__mei?.artifact_hits || boot.lastArtifactHits || null,
      keys: { shell: shellKey, revision: revisionKey },
      flags: readFlags(),
      ssrRevision,
      cachedRevision,
      snapshot: null,
      bootApi: {
        inspectSceneClientCache: typeof hostBoot.inspectSceneClientCache === "function",
        viewRevisionClient: typeof hostBoot.viewRevisionClient?.fetchViewRevision === "function",
        layerArtifactCache: typeof hostBoot.layerArtifactCache?.listHoldings === "function",
      },
      viewRevisionOutcome: hostBoot.lastViewRevisionOutcome || null,
      events: (global.__meiCacheDiag?.events || []).slice(-12),
    };
    trace("inspect", report);
    return report;
  }

  async function inspectCrossSurfaceCache() {
    const hostBoot = global.__meiLangBoot || globalThis.__meiLangBoot || boot;
    const viewCtx =
      typeof hostBoot.parseViewContext === "function"
        ? hostBoot.parseViewContext(global.location.href)
        : null;
    const appId =
      viewCtx?.app_id || viewCtx?.appId || document.body?.getAttribute("data-app-id") || "";
    const sceneId =
      viewCtx?.scene_id || viewCtx?.sceneId || document.body?.getAttribute("data-scene-id") || "home";
    const holdings =
      typeof hostBoot.layerStore?.listHoldings === "function"
        ? await hostBoot.layerStore.listHoldings(appId, sceneId)
        : typeof hostBoot.layerArtifactCache?.listHoldings === "function"
          ? await hostBoot.layerArtifactCache.listHoldings(appId, sceneId)
          : [];
    const mapResources = performance
      .getEntriesByType("resource")
      .filter((entry) => /tilejson|maplibre|\/gis\//i.test(entry.name))
      .map((entry) => ({
        name: entry.name.slice(-72),
        ms: Math.round(entry.duration),
        start: Math.round(entry.startTime),
      }));
    const sidebarScroll = document.querySelector("aside .sidebar-scroll");
    const report = {
      url: global.location.href,
      appId,
      sceneId,
      surface: viewCtx?.surface || "unknown",
      layerHoldings: holdings,
      lastViewRevisionOutcome: hostBoot.lastViewRevisionOutcome || null,
      flags: readFlags(),
      treeUi: {
        hasShell: !!document.querySelector(".build-tree-shell"),
        duplicateChevrons: document.querySelectorAll(".build-tree-summary > .build-tree-kind")
          .length,
        treeNodes: document.querySelectorAll(".build-tree-node").length,
        sidebarScroll: sidebarScroll
          ? {
              clientHeight: sidebarScroll.clientHeight,
              scrollHeight: sidebarScroll.scrollHeight,
              canScroll: sidebarScroll.scrollHeight > sidebarScroll.clientHeight + 1,
            }
          : null,
      },
      mapResources,
      mapNote:
        "地图为 MapLibre WebGL 客户端组件，不走 SSR 页面缓存；瓦片/tilejson 在整页刷新后重新拉取。",
    };
    trace("inspect-cross-surface", report);
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
        url.includes("/api/host/view-revision") ||
        url.includes("/api/host/scene-bootstrap") ||
        url.includes("/api/host/scene-fragment") ||
        url.includes("/api/host/layer-batch") ||
        url.includes("/api/host/scene-manifest")
      ) {
        trace("fetch", { url, method: init?.method || "GET" });
      }
      return nativeFetch(input, init);
    };
  }

  function publishApi() {
    const api = {
      inspect,
      inspectCrossSurfaceCache,
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
    boot.inspectCrossSurfaceCache = inspectCrossSurfaceCache;
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
