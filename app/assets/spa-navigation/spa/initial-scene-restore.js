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
    params.set("format", "manifest");
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
        const root =
          typeof boot.resolveComposeRoot === "function"
            ? boot.resolveComposeRoot(ctx.surface || ctx.mode || "app")
            : document.querySelector(".shell");
        if (!root?.querySelector("[data-preview-scope], [data-mei-ui-role]")) {
          return null;
        }
        boot.viewCompositor.composePreview(root, structure, projection, null, null);
        window.__meiShellRestoredFromManifest = 1;
        return document;
      }
    }
    return null;
  }

  async function wakeRevisionFirstShellRuntime(ctx) {
    if (!ctx) return;
    const surface = ctx.surface || ctx.mode || "app";
    if (typeof boot.isWorkspaceComposeSurface === "function" && boot.isWorkspaceComposeSurface(surface)) {
      if (typeof boot.installManageTabs === "function") {
        boot.installManageTabs();
      }
      if (typeof globalThis.MeiBuildTreePersist?.refresh === "function") {
        globalThis.MeiBuildTreePersist.refresh();
      }
      if (typeof globalThis.MeiBuildInspectHighlight?.refresh === "function") {
        globalThis.MeiBuildInspectHighlight.refresh();
      }
      if (typeof publishManagePreviewFromDoc === "function") {
        publishManagePreviewFromDoc(document, { resetRuntimeQueryCache: true, pulsePreviewUpdated: true });
      }
      if (typeof boot.mountManagePreviewBoard === "function") {
        await boot.mountManagePreviewBoard(document);
      }
      return;
    }
    if (typeof boot.dispatchScopeActivation === "function") {
      boot.dispatchScopeActivation({
        scope: ctx.sceneId || ctx.scene_id || "home",
        sceneId: ctx.sceneId || ctx.scene_id || "home",
        appId: ctx.appId || ctx.app_id || "",
        source: "revision-first-cold-start",
      });
    }
    if (typeof wakeRuntimeAfterSceneBundleLoaded === "function") {
      wakeRuntimeAfterSceneBundleLoaded();
    }
  }

  async function finishRevisionFirstColdStart(ctx, outcome) {
    let resolved = outcome || { restored: false };
    if (!resolved?.restored && boot.negotiateAndAssemble) {
      const retry = await boot.negotiateAndAssemble(ctx, { silent: true });
      if (retry?.assemble?.ok) {
        resolved = {
          restored: true,
          doc: document,
          revision: retry.response,
          source: retry.outcome || "assemble_local",
        };
      }
    }
    if (resolved?.restored) {
      await wakeRevisionFirstShellRuntime(ctx);
      return resolved;
    }
    if (typeof boot.bootstrapThinShellComposition === "function") {
      await boot.bootstrapThinShellComposition();
    }
    await wakeRevisionFirstShellRuntime(ctx);
    return resolved;
  }

  async function negotiateAndAssemble(ctx, options) {
    const opts = options || {};
    const viewCtx =
      typeof boot.parseViewContext === "function"
        ? boot.parseViewContext(ctx?.url || window.location.href)
        : ctx;
    if (!viewCtx || !boot.viewRevisionClient?.negotiateWithLocalMiss) {
      return null;
    }
    const vrCtx = {
      app_id: viewCtx.app_id || viewCtx.appId,
      scene_id: viewCtx.scene_id || viewCtx.sceneId,
      surface: viewCtx.surface || viewCtx.mode || "app",
      node: viewCtx.node || "",
      data_mode: viewCtx.data_mode || viewCtx.dataMode || "",
      review_projection: viewCtx.review_projection || viewCtx.reviewProjection || "",
      chrome: viewCtx.chrome || "",
      tab: viewCtx.tab || "",
      focus: viewCtx.focus || "",
      scope: viewCtx.scope || "",
    };
    try {
      const result = await boot.viewRevisionClient.negotiateWithLocalMiss(vrCtx);
      if (!result?.assemble?.ok) {
        return result;
      }
      const surface = vrCtx.surface || "app";
      if (surface === "layout" || surface === "prototype" || surface === "build") {
        if (typeof boot.installManageTabs === "function") {
          boot.installManageTabs();
        }
        if (typeof globalThis.MeiBuildTreePersist?.refresh === "function") {
          globalThis.MeiBuildTreePersist.refresh();
        }
      }
      if (
        typeof boot.renderStructureTree === "function" &&
        result.assemble?.layers?.["structure.full"]
      ) {
        boot.renderStructureTree(result.assemble.layers["structure.full"], {
          appId: vrCtx.app_id,
          sceneId: vrCtx.scene_id,
          surface,
          activeNode: vrCtx.node || "",
        });
      }
      if (typeof boot.rememberViewRevision === "function" && result.response) {
        boot.rememberViewRevision(viewCtx, result.response);
      }
      await wakeRevisionFirstShellRuntime(viewCtx);
      return result;
    } catch (error) {
      if (!opts.silent) {
        console.warn("[spa-navigation] negotiateAndAssemble skipped", error);
      }
      return null;
    }
  }

  async function tryCacheFirstViewRestore(urlLike, options) {
    const opts = options || {};
    const ctx =
      typeof boot.parseViewContext === "function"
        ? boot.parseViewContext(urlLike || window.location.href)
        : typeof boot.parseAccessSceneContext === "function"
          ? boot.parseAccessSceneContext(urlLike || window.location.href)
          : null;
    if (!ctx) {
      return { restored: false, doc: null, revision: null, source: "none" };
    }
    const surface = ctx.surface || ctx.mode || "app";
    const isWorkspace =
      surface === "build" ||
      surface === "layout" ||
      surface === "prototype" ||
      (typeof isBuildWorkspacePathname === "function" &&
        isBuildWorkspacePathname(new URL(urlLike || window.location.href).pathname));
    if (isWorkspace && typeof globalThis.MeiBuildNavigation?.tryRestoreBuildPreviewFromCache === "function") {
      try {
        const buildOutcome = await globalThis.MeiBuildNavigation.tryRestoreBuildPreviewFromCache(
          urlLike || window.location.href,
          {
            timeoutMs: opts.timeoutMs || 4000,
            coldStart: opts.coldStart !== false,
            skipRemoteWhenValid: opts.skipRemoteWhenValid !== false,
          },
        );
        if (buildOutcome?.restored) {
          return {
            restored: true,
            doc: document,
            revision: buildOutcome.revision,
            source: buildOutcome.source || "build-cache",
          };
        }
      } catch (error) {
        console.warn("[spa-navigation] build cache restore skipped", error);
      }
    }
    const negotiated = await negotiateAndAssemble(ctx, opts);
    if (negotiated?.assemble?.ok) {
      const outcome =
        negotiated.outcome === (boot.ViewRevisionOutcome?.REFETCH || "refetch")
          ? "refetch"
          : (boot.ViewRevisionOutcome?.ASSEMBLE_LOCAL || "assemble_local");
      return {
        restored: true,
        doc: document,
        revision: negotiated.response,
        source: outcome,
        viewRevision: negotiated,
      };
    }
    if (typeof boot.tryCacheFirstSceneAccess === "function") {
      return boot.tryCacheFirstSceneAccess(ctx, {
        ...opts,
        url: urlLike || window.location.href,
      });
    }
    return {
      restored: false,
      doc: null,
      revision: negotiated?.response || null,
      source: negotiated?.outcome || "miss",
    };
  }

  async function tryViewRevisionAssemble(ctx) {
    if (!boot.viewRevisionClient?.negotiateWithLocalMiss) return null;
    if (boot.viewRevisionClient.isEnabled && !boot.viewRevisionClient.isEnabled()) {
      return null;
    }
    const vrCtx = {
      app_id: ctx.appId || ctx.app_id,
      scene_id: ctx.sceneId || ctx.scene_id,
      surface: ctx.mode || ctx.surface || "app",
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

    const viewRevisionOutcome = await tryViewRevisionAssemble(ctx);
    if (viewRevisionOutcome?.restored) {
      if (typeof boot.ensureSceneBootstrapPayload === "function") {
        await boot.ensureSceneBootstrapPayload(ctx, viewRevisionOutcome.revision);
      }
      return viewRevisionOutcome;
    }

    if (viewRevisionOutcome?.source === "local_miss") {
      return viewRevisionOutcome;
    }

    if (typeof boot.fetchSceneRevision === "function") {
      const revision = await boot.fetchSceneRevision(ctx, {
        timeoutMs: opts.timeoutMs || 30000,
        skipRemoteWhenValid: opts.skipRemoteWhenValid === true,
        preloadSnapshotRevision: false,
      });
      if (typeof boot.ensureSceneBootstrapPayload === "function") {
        await boot.ensureSceneBootstrapPayload(ctx, revision);
      }
      return {
        restored: false,
        doc: null,
        revision,
        source: viewRevisionOutcome?.source || "miss",
      };
    }

    return { restored: false, doc: null, revision: null, source: "none" };
  }

  boot.finishRevisionFirstColdStart = finishRevisionFirstColdStart;
  boot.wakeRevisionFirstShellRuntime = wakeRevisionFirstShellRuntime;
  boot.fetchSceneFragment = fetchSceneFragment;
  boot.tryRestoreSceneShellFromFragment = tryRestoreSceneShellFromFragment;
  boot.negotiateAndAssemble = negotiateAndAssemble;
  boot.tryCacheFirstViewRestore = tryCacheFirstViewRestore;
  boot.tryCacheFirstSceneAccess = tryCacheFirstSceneAccess;
