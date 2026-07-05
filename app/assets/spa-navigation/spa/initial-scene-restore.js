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

  async function ensureThinShellSceneRuntime() {
    const runtimeReady =
      window.__meiDatasetRuntime &&
      typeof window.__meiDatasetRuntime.prefetchVisiblePanelMetrics === "function";
    if (runtimeReady) return true;
    const bundleNode = document.querySelector('script[data-mei-scene-bundle="true"][src]');
    const bundleSrc = bundleNode?.getAttribute("src") || "";
    const moduleScripts = bundleSrc
      ? [bundleSrc]
      : Array.from(document.querySelectorAll('script[type="module"][src^="/workspace-components/"]'))
          .map((node) => node.getAttribute("src") || "")
          .filter(Boolean);
    if (!moduleScripts.length || typeof boot.syncPreviewWorkspaceScripts !== "function") {
      return false;
    }
    try {
      return await boot.syncPreviewWorkspaceScripts(moduleScripts, null);
    } catch (error) {
      console.warn("[spa-navigation] thin shell scene runtime sync skipped", error);
      return false;
    }
  }

  function extractLayerDocument(layerValue) {
    if (!layerValue) return null;
    if (typeof layerValue === "string") {
      try {
        return JSON.parse(layerValue);
      } catch (_) {
        return null;
      }
    }
    if (Array.isArray(layerValue.nodes) || layerValue.schema_version) {
      return layerValue;
    }
    if (layerValue.document) return layerValue.document;
    return layerValue;
  }

  function workspaceStructureTreeReady() {
    const nav =
      document.querySelector("aside nav.build-reachability-tree") ||
      document.querySelector("nav.build-reachability-tree");
    return !!nav?.querySelector(".build-tree-list .build-tree-node");
  }

  async function ensureWorkspaceStructureTree(ctx, layersFromAssemble) {
    const surface = ctx?.surface || ctx?.mode || "";
    if (typeof boot.isWorkspaceComposeSurface === "function" && !boot.isWorkspaceComposeSurface(surface)) {
      return false;
    }
    if (workspaceStructureTreeReady()) return true;
    const appId = ctx?.app_id || ctx?.appId || "";
    const sceneId = ctx?.scene_id || ctx?.sceneId || "home";
    if (!appId) return false;

    let structure = extractLayerDocument(layersFromAssemble?.["structure.full"]);
    const manifest = globalThis.__mei?.scene_manifest_refs;
    const ref = manifest?.layers?.["structure.full"];
    const holding = ref
      ? {
          name: "structure.full",
          artifact_id: ref.artifact_id || ref.artifactId,
          content_hash: ref.content_hash || ref.contentHash,
        }
      : null;

    if (!structure?.nodes?.length && holding && boot.layerStore?.takeLayerByRef) {
      structure = extractLayerDocument(boot.layerStore.takeLayerByRef(holding));
    }
    if (!structure?.nodes?.length && holding?.artifact_id && boot.layerArtifactCache?.getLayer) {
      const cached = await boot.layerArtifactCache.getLayer(holding.artifact_id);
      if (cached && (!holding.content_hash || cached.content_hash === holding.content_hash)) {
        structure = extractLayerDocument(cached.bytes);
        if (structure?.nodes?.length && boot.layerStore?.putLayerByRef) {
          await boot.layerStore.putLayerByRef(appId, sceneId, holding, cached.bytes, manifest);
        }
      }
    }
    if (!structure?.nodes?.length && boot.sceneManifestLoader?.fetchLayerBatch) {
      try {
        const batch = await boot.sceneManifestLoader.fetchLayerBatch(
          appId,
          sceneId,
          ["structure.full"],
          boot.sceneManifestLoader.readShellAxes?.() || {},
        );
        structure = extractLayerDocument(batch?.layers?.["structure.full"]);
      } catch (_) {}
    }
    if (structure?.nodes?.length && typeof boot.renderStructureTree === "function") {
      return boot.renderStructureTree(structure, {
        appId,
        sceneId,
        surface: surface || "layout",
        activeNode: ctx?.node || "",
      });
    }
    return false;
  }

  async function wakeRevisionFirstShellRuntime(ctx, options = {}) {
    if (!ctx) return;
    const surface = ctx.surface || ctx.mode || "app";
    const ssrPreview = options.ssrPreview === true;
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
      if (ssrPreview) {
        return;
      }
      if (typeof publishManagePreviewFromDoc === "function") {
        publishManagePreviewFromDoc(document, { resetRuntimeQueryCache: true, pulsePreviewUpdated: true });
      }
      if (typeof boot.mountManagePreviewBoard === "function") {
        await boot.mountManagePreviewBoard(document);
      }
      return;
    }
    await ensureThinShellSceneRuntime();
    if (typeof boot.dispatchScopeActivation === "function") {
      boot.dispatchScopeActivation({
        scope: ctx.sceneId || ctx.scene_id || "home",
        sceneId: ctx.sceneId || ctx.scene_id || "home",
        appId: ctx.appId || ctx.app_id || "",
        source: "revision-first-cold-start",
      });
    }
    if (ssrPreview) {
      if (typeof boot.scheduleFrameViewportRelayout === "function") {
        boot.scheduleFrameViewportRelayout();
      }
      if (typeof dispatchPanelMetricPrefetch === "function") {
        dispatchPanelMetricPrefetch();
      }
      return;
    }
    if (typeof wakeRuntimeAfterSceneBundleLoaded === "function") {
      wakeRuntimeAfterSceneBundleLoaded();
    }
  }

  async function finishRevisionFirstColdStart(ctx, outcome) {
    let resolved = outcome || { restored: false };
    const surface = ctx?.surface || ctx?.mode || "app";
    const composeRoot =
      typeof boot.resolveComposeRoot === "function"
        ? boot.resolveComposeRoot(surface)
        : document.querySelector(".shell");
    const ssrPreviewReady =
      typeof boot.hasMaterializedPreview === "function" &&
      boot.hasMaterializedPreview(composeRoot);
    if (ssrPreviewReady) {
      if (typeof boot.hideThinShellFallback === "function") {
        boot.hideThinShellFallback();
      }
      if (typeof boot.hydrateManifestLayerHoldings === "function") {
        boot.hydrateManifestLayerHoldings();
      }
      const negotiated = await boot.negotiateAndAssemble?.(ctx, { silent: true });
      await ensureWorkspaceStructureTree(ctx, negotiated?.assemble?.layers);
      await wakeRevisionFirstShellRuntime(ctx, { ssrPreview: true });
      return { ...resolved, restored: true, source: "ssr_preview" };
    }
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
      if (typeof boot.hideThinShellFallback === "function") {
        boot.hideThinShellFallback();
      }
      await wakeRevisionFirstShellRuntime(ctx);
      return resolved;
    }
    if (typeof boot.bootstrapThinShellComposition === "function") {
      const ok = await boot.bootstrapThinShellComposition();
      if (ok) {
        if (typeof boot.hideThinShellFallback === "function") {
          boot.hideThinShellFallback();
        }
        await wakeRevisionFirstShellRuntime(ctx);
        return { ...resolved, restored: true, source: "thin-bootstrap" };
      }
    }
    await wakeRevisionFirstShellRuntime(ctx);
    const scopeCount = document.querySelectorAll("[data-preview-scope]").length;
    if (scopeCount === 0 && typeof boot.showThinShellFallback === "function") {
      boot.showThinShellFallback("场景内容暂时无法加载，请刷新重试或检查网络。");
    } else if (typeof boot.hideThinShellFallback === "function") {
      boot.hideThinShellFallback();
    }
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
      await ensureWorkspaceStructureTree(vrCtx, result.assemble?.layers);
      if (typeof boot.rememberViewRevision === "function" && result.response) {
        boot.rememberViewRevision(viewCtx, result.response);
      }
      const ssrPreview =
        result.assemble?.source === "ssr_preview" ||
        (typeof boot.hasMaterializedPreview === "function" &&
          boot.hasMaterializedPreview(
            typeof boot.resolveComposeRoot === "function"
              ? boot.resolveComposeRoot(surface)
              : document.querySelector(".shell"),
          ));
      await wakeRevisionFirstShellRuntime(viewCtx, { ssrPreview });
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
    if (ctx) {
      const surface = ctx.surface || ctx.mode || "app";
      const composeRoot =
        typeof boot.resolveComposeRoot === "function"
          ? boot.resolveComposeRoot(surface)
          : document.querySelector(".shell");
      if (
        typeof boot.hasMaterializedPreview === "function" &&
        boot.hasMaterializedPreview(composeRoot)
      ) {
        if (typeof boot.hydrateManifestLayerHoldings === "function") {
          boot.hydrateManifestLayerHoldings();
        }
        const negotiated = await boot.negotiateAndAssemble?.(ctx, { silent: true });
        await ensureWorkspaceStructureTree(ctx, negotiated?.assemble?.layers);
        await wakeRevisionFirstShellRuntime(ctx, { ssrPreview: true });
        return {
          restored: true,
          doc: document,
          revision: negotiated?.response || null,
          source: "ssr_preview",
        };
      }
    }
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
      if (typeof boot.hideThinShellFallback === "function") {
        boot.hideThinShellFallback();
      }
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
  boot.ensureWorkspaceStructureTree = ensureWorkspaceStructureTree;
  boot.wakeRevisionFirstShellRuntime = wakeRevisionFirstShellRuntime;
  boot.fetchSceneFragment = fetchSceneFragment;
  boot.tryRestoreSceneShellFromFragment = tryRestoreSceneShellFromFragment;
  boot.negotiateAndAssemble = negotiateAndAssemble;
  boot.tryCacheFirstViewRestore = tryCacheFirstViewRestore;
  boot.tryCacheFirstSceneAccess = tryCacheFirstSceneAccess;
