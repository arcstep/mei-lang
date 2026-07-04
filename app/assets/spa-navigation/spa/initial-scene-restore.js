  const SCENE_FRAGMENT_API = "/api/host/scene-fragment";

  async function fetchSceneFragment(ctx, options) {
    const opts = options || {};
    const params = new URLSearchParams({
      app: ctx.appId,
      scene: ctx.sceneId,
    });
    if (ctx.dataMode) params.set("data_mode", ctx.dataMode);
    if (ctx.reviewProjection) params.set("review_projection", ctx.reviewProjection);
    if (ctx.chrome) params.set("chrome", ctx.chrome);
    const controller = opts.signal ? null : new AbortController();
    const signal = opts.signal || controller?.signal;
    const response = await fetch(`${SCENE_FRAGMENT_API}?${params.toString()}`, {
      credentials: "same-origin",
      headers: { Accept: "application/json", "x-mei-spa-nav": "1" },
      signal,
    });
    if (!response.ok) {
      throw new Error(`scene fragment failed: ${response.status}`);
    }
    return await response.json();
  }

  async function tryRestoreSceneShellFromFragment(ctx, revision, url, replaceHistory) {
    const fragment = await fetchSceneFragment(ctx);
    if (!fragment?.shellHtml) return null;
    if (
      fragment.revisionDigest &&
      revision?.revision_digest &&
      fragment.revisionDigest !== revision.revision_digest
    ) {
      return null;
    }
    const snapshot = {
      shellHtml: fragment.shellHtml,
      title: fragment.title || document.title,
      bodyClassName: document.body.className,
      headScripts:
        typeof boot.collectHeadJsonScripts === "function" ? boot.collectHeadJsonScripts() : {},
    };
    if (typeof boot.restoreSceneShellSnapshot !== "function") return null;
    const restored = boot.restoreSceneShellSnapshot(snapshot, url, replaceHistory);
    if (!restored) return null;
    window.__meiShellRestoredFromFragment = 1;
    if (typeof boot.buildDocFromSceneShellSnapshot === "function") {
      return boot.buildDocFromSceneShellSnapshot(snapshot);
    }
    return new DOMParser().parseFromString(
      `<!DOCTYPE html><html><head><title>${snapshot.title || ""}</title></head><body><div class="shell">${snapshot.shellHtml}</div></body></html>`,
      "text/html",
    );
  }

  async function tryCacheFirstSceneAccess(ctx, options) {
    const opts = options || {};
    if (!ctx || typeof boot.fetchSceneRevision !== "function") {
      return { restored: false, doc: null, revision: null, source: "none" };
    }
    const timeoutMs =
      opts.timeoutMs ||
      (typeof SPA_FETCH_TIMEOUT_MS !== "undefined" ? SPA_FETCH_TIMEOUT_MS : 30000);
    const revision = await boot.fetchSceneRevision(ctx, { timeoutMs });
    if (
      opts.navigationId != null &&
      typeof currentNavigationId !== "undefined" &&
      opts.navigationId !== currentNavigationId
    ) {
      return { restored: false, doc: null, revision, source: "superseded" };
    }

    // 冷启动仅拉 bootstrap 时保留 SSR shell，避免 IndexedDB 旧快照盖掉服务端新 HTML。
    if (opts.hydrateBootstrapOnly === true) {
      if (typeof boot.ensureSceneBootstrapPayload === "function") {
        await boot.ensureSceneBootstrapPayload(ctx, revision);
      }
      return { restored: false, doc: null, revision, source: "bootstrap-only" };
    }

    if (typeof boot.tryRestoreSceneShellFromCache === "function") {
      const restoredDoc = await boot.tryRestoreSceneShellFromCache(
        ctx,
        revision,
        opts.url || null,
        !!opts.replaceHistory,
      );
      if (restoredDoc) {
        if (typeof boot.ensureSceneBootstrapPayload === "function") {
          await boot.ensureSceneBootstrapPayload(ctx, revision);
        }
        return { restored: true, doc: restoredDoc, revision, source: "snapshot" };
      }
    }

    if (opts.allowFragment !== false) {
      try {
        const fragmentDoc = await tryRestoreSceneShellFromFragment(
          ctx,
          revision,
          opts.url || null,
          !!opts.replaceHistory,
        );
        if (fragmentDoc) {
          if (typeof boot.ensureSceneBootstrapPayload === "function") {
            await boot.ensureSceneBootstrapPayload(ctx, revision);
          }
          return { restored: true, doc: fragmentDoc, revision, source: "fragment" };
        }
      } catch (error) {
        console.warn("[spa-navigation] scene fragment restore skipped", error);
      }
    }

    if (opts.hydrateBootstrapOnly !== false && typeof boot.ensureSceneBootstrapPayload === "function") {
      await boot.ensureSceneBootstrapPayload(ctx, revision);
    }
    return { restored: false, doc: null, revision, source: "miss" };
  }

  boot.fetchSceneFragment = fetchSceneFragment;
  boot.tryRestoreSceneShellFromFragment = tryRestoreSceneShellFromFragment;
  boot.tryCacheFirstSceneAccess = tryCacheFirstSceneAccess;
