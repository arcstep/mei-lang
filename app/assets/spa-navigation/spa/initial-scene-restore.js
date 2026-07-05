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
    if (!opts.htmlFallback) params.set("format", "manifest");
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
    if (fragment?.manifest && boot.sceneManifestLoader && boot.viewCompositor) {
      const structure =
        fragment.manifest?.layers?.["structure.full"]?.content_hash != null
          ? (
              await boot.sceneManifestLoader.fetchLayerBatch(
                ctx.appId,
                ctx.sceneId,
                ["structure.full"],
                boot.sceneManifestLoader.readShellAxes(),
              )
            )?.layers?.["structure.full"]
          : null;
      if (structure) {
        const projection =
          ctx.reviewProjection ||
          fragment.compose_defaults?.review_projection ||
          "live_full";
        const root = document.querySelector(".shell");
        if (!root?.querySelector("[data-preview-scope], [data-mei-ui-role]")) {
          return null;
        }
        boot.viewCompositor.composePreview(root, structure, projection, null, null);
        window.__meiShellRestoredFromManifest = 1;
        return document;
      }
    }
    if (!fragment?.shellHtml) return null;
    const normalizedRevision =
      typeof boot.normalizeRevision === "function"
        ? boot.normalizeRevision(revision)
        : revision;
    if (
      fragment.revisionDigest &&
      normalizedRevision?.revision_digest &&
      fragment.revisionDigest !== normalizedRevision.revision_digest
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
    if (typeof boot.cacheDiagTrace === "function") {
      boot.cacheDiagTrace("shell-restored", { source: "fragment", url: url || null });
    }
    if (typeof boot.buildDocFromSceneShellSnapshot === "function") {
      return boot.buildDocFromSceneShellSnapshot(snapshot);
    }
    return new DOMParser().parseFromString(
      `<!DOCTYPE html><html><head><title>${snapshot.title || ""}</title></head><body><div class="shell">${snapshot.shellHtml}</div></body></html>`,
      "text/html",
    );
  }

  async function tryViewRevisionAssemble(ctx) {
    if (!boot.viewRevisionClient?.negotiateWithLocalMiss) return null;
    if (boot.viewRevisionClient.isEnabled && !boot.viewRevisionClient.isEnabled()) {
      return null;
    }
    const vrCtx = {
      app_id: ctx.appId,
      scene_id: ctx.sceneId,
      surface: ctx.mode || "app",
      data_mode: ctx.dataMode,
      review_projection: ctx.reviewProjection,
      chrome: ctx.chrome,
    };
    try {
      const result = await boot.viewRevisionClient.negotiateWithLocalMiss(vrCtx);
      if (result?.assemble?.ok) {
        const outcome =
          result.outcome === (boot.ViewRevisionOutcome?.REFETCH || "refetch")
            ? "refetch"
            : (boot.ViewRevisionOutcome?.ASSEMBLE_LOCAL || "assemble_local");
        if (typeof boot.cacheDiagTrace === "function") {
          boot.cacheDiagTrace("view-revision-outcome", {
            outcome,
          });
        }
        return {
          restored: true,
          doc: document,
          revision: result.response,
          source: outcome,
          viewRevision: result,
        };
      }
      if (result?.outcome === (boot.ViewRevisionOutcome?.LOCAL_MISS || "local_miss")) {
        if (typeof boot.cacheDiagTrace === "function") {
          boot.cacheDiagTrace("view-revision-outcome", { outcome: "local_miss" });
        }
        return {
          restored: false,
          doc: null,
          revision: result.response,
          source: "local_miss",
          viewRevision: result,
        };
      }
    } catch (error) {
      console.warn("[spa-navigation] view-revision assemble skipped", error);
    }
    return null;
  }

  async function tryCacheFirstSceneAccess(ctx, options) {
    const opts = options || {};
    if (!ctx) {
      return { restored: false, doc: null, revision: null, source: "none" };
    }

    const prefetchedFragment = globalThis.__mei?.prefetched_scene_fragment;
    if (
      prefetchedFragment?.shellHtml &&
      typeof boot.restoreSceneShellSnapshot === "function"
    ) {
      const restored = boot.restoreSceneShellSnapshot(
        {
          shellHtml: prefetchedFragment.shellHtml,
          title: prefetchedFragment.title || document.title,
          bodyClassName: document.body.className,
          headScripts: prefetchedFragment.headScripts || {},
        },
        opts.url || window.location.href,
        !!opts.replaceHistory,
      );
      if (restored) {
        return {
          restored: true,
          doc: document,
          revision: null,
          source: "prefetched_fragment",
        };
      }
    }

    const viewRevisionOutcome = await tryViewRevisionAssemble(ctx);
    if (viewRevisionOutcome?.restored) {
      if (typeof boot.ensureSceneBootstrapPayload === "function") {
        await boot.ensureSceneBootstrapPayload(ctx, viewRevisionOutcome.revision);
      }
      return viewRevisionOutcome;
    }

    if (!ctx || typeof boot.fetchSceneRevision !== "function") {
      return { restored: false, doc: null, revision: null, source: "none" };
    }
    const timeoutMs =
      opts.timeoutMs ||
      (typeof SPA_FETCH_TIMEOUT_MS !== "undefined" ? SPA_FETCH_TIMEOUT_MS : 30000);
    const coldStart = opts.coldStart === true;
    const revision = await boot.fetchSceneRevision(ctx, {
      timeoutMs,
      skipRemoteWhenValid: opts.skipRemoteWhenValid === true,
      preloadSnapshotRevision: true,
    });
    if (
      opts.navigationId != null &&
      typeof currentNavigationId !== "undefined" &&
      opts.navigationId !== currentNavigationId
    ) {
      return { restored: false, doc: null, revision, source: "superseded" };
    }

    if (typeof boot.tryRestoreSceneShellFromCache === "function") {
      const restoredDoc = await boot.tryRestoreSceneShellFromCache(
        ctx,
        revision,
        opts.url || window.location.href,
        !!opts.replaceHistory,
      );
      if (restoredDoc) {
        if (typeof boot.ensureSceneBootstrapPayload === "function") {
          await boot.ensureSceneBootstrapPayload(ctx, revision);
        }
        if (typeof boot.cacheDiagTrace === "function") {
          boot.cacheDiagTrace("cache-first-outcome", { source: "snapshot", coldStart });
        }
        return { restored: true, doc: restoredDoc, revision, source: "snapshot" };
      }
    }

    if (!coldStart && opts.allowFragment !== false) {
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
          if (typeof boot.cacheDiagTrace === "function") {
            boot.cacheDiagTrace("cache-first-outcome", { source: "fragment", coldStart });
          }
          return { restored: true, doc: fragmentDoc, revision, source: "fragment" };
        }
      } catch (error) {
        console.warn("[spa-navigation] scene fragment restore skipped", error);
      }
    }

    if (viewRevisionOutcome?.source === "local_miss") {
      return viewRevisionOutcome;
    }

    if (typeof boot.ensureSceneBootstrapPayload === "function") {
      await boot.ensureSceneBootstrapPayload(ctx, revision);
    }
    if (typeof boot.cacheDiagTrace === "function") {
      boot.cacheDiagTrace("cache-first-outcome", {
        source: coldStart ? "cold-miss" : "miss",
        coldStart,
      });
    }
    return { restored: false, doc: null, revision, source: coldStart ? "cold-miss" : "miss" };
  }

  boot.fetchSceneFragment = fetchSceneFragment;
  boot.tryRestoreSceneShellFromFragment = tryRestoreSceneShellFromFragment;
  boot.tryCacheFirstSceneAccess = tryCacheFirstSceneAccess;
