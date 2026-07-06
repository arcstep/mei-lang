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

  function workspaceStructureTreeReady(generation) {
    if (typeof boot.workspaceStructureTreeReady === "function") {
      return boot.workspaceStructureTreeReady(generation);
    }
    const nav =
      document.querySelector("aside .build-reachability-tree") ||
      document.querySelector(".build-tree-shell .build-reachability-tree") ||
      document.querySelector("nav.build-reachability-tree");
    return !!nav?.querySelector(".build-tree-list .build-tree-node");
  }

  async function ensureWorkspaceStructureTree(ctx, layersFromAssemble, options) {
    const opts = options || {};
    const generation = opts.generation;
    const surface = ctx?.surface || ctx?.mode || "";
    if (typeof boot.isWorkspaceComposeSurface === "function" && !boot.isWorkspaceComposeSurface(surface)) {
      return false;
    }
    if (workspaceStructureTreeReady(generation)) return true;
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
          { signal: opts.signal },
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
        generation,
      });
    }
    if (boot.viewAssembly?.onLayerResident) {
      boot.viewAssembly.onLayerResident("structure.full", async (residentLayers, residentGen) => {
        if (residentGen != null && generation != null && residentGen !== generation) return;
        if (workspaceStructureTreeReady(generation)) return;
        await ensureWorkspaceStructureTree(ctx, residentLayers, { generation, signal: opts.signal });
      });
    }
    return false;
  }

  function clearSurfaceRuntimeWarmedForApp(ctx) {
    const vrCtx = vrCtxFromViewCtx(ctx);
    const prefix = [vrCtx.app_id, vrCtx.scene_id].filter(Boolean).join(":");
    if (!prefix || !global.__meiSurfaceRuntimeWarmed) return;
    for (const key of [...global.__meiSurfaceRuntimeWarmed]) {
      if (key === prefix || key.startsWith(`${prefix}:`)) {
        global.__meiSurfaceRuntimeWarmed.delete(key);
      }
    }
  }

  async function wakeRevisionFirstShellRuntime(ctx, options = {}) {
    if (!ctx) return;
    const surface = ctx.surface || ctx.mode || "app";
    const ssrPreview = options.ssrPreview === true;
    const warmOnly =
      options.forceRuntimeWake === true
        ? false
        : options.warmOnly === true ||
          (options.forceRuntimeWake !== true && isSurfaceRuntimeWarmed(ctx));
    if (typeof boot.isWorkspaceComposeSurface === "function" && boot.isWorkspaceComposeSurface(surface)) {
      if (typeof boot.installManageTabs === "function") {
        boot.installManageTabs();
      }
      if (typeof globalThis.MeiBuildTreePersist?.refresh === "function") {
        globalThis.MeiBuildTreePersist.refresh();
      }
      if (!warmOnly) {
        if (typeof boot.restoreWorkspacePreviewSnapshot === "function") {
          boot.restoreWorkspacePreviewSnapshot();
        }
        if (typeof globalThis.MeiBuildInspectHighlight?.refresh === "function") {
          globalThis.MeiBuildInspectHighlight.refresh();
        }
        if (typeof publishManagePreviewFromDoc === "function") {
          publishManagePreviewFromDoc(document, {
            resetRuntimeQueryCache: options.forceRuntimeWake === true || !ssrPreview,
            pulsePreviewUpdated: true,
          });
        }
        if (typeof boot.syncPreviewWorkspaceScripts === "function") {
          const scripts = Array.from(
            document.querySelectorAll('script[type="module"][src^="/workspace-components/"]'),
          )
            .map((node) => node.getAttribute("src") || "")
            .filter(Boolean);
          if (scripts.length) {
            try {
              await boot.syncPreviewWorkspaceScripts(scripts, null);
            } catch (_) {}
          }
        }
        if (typeof boot.mountManagePreviewBoard === "function") {
          await boot.mountManagePreviewBoard(document);
        }
        markSurfaceRuntimeWarmed(ctx);
      }
      return;
    }
    if (!warmOnly) {
      await ensureThinShellSceneRuntime();
      markSurfaceRuntimeWarmed(ctx);
    }
    if (typeof boot.dispatchScopeActivation === "function") {
      boot.dispatchScopeActivation({
        scope: ctx.sceneId || ctx.scene_id || "home",
        sceneId: ctx.sceneId || ctx.scene_id || "home",
        appId: ctx.appId || ctx.app_id || "",
        source: warmOnly ? "revision-first-warm-skip" : "revision-first-cold-start",
      });
    }
    if (typeof boot.scheduleFrameViewportRelayout === "function") {
      boot.scheduleFrameViewportRelayout();
    }
    if (!warmOnly && typeof dispatchPanelMetricPrefetch === "function") {
      dispatchPanelMetricPrefetch();
    }
    const isWorkspace =
      typeof boot.isWorkspaceComposeSurface === "function" &&
      boot.isWorkspaceComposeSurface(surface);
    if (
      typeof wakeRuntimeAfterSceneBundleLoaded === "function" &&
      (!warmOnly || !isWorkspace)
    ) {
      wakeRuntimeAfterSceneBundleLoaded();
    }
  }

  function isSsrInjectedPreviewRoot(root) {
    if (boot.previewMaterializer?.isSsrInjectedPreviewRoot) {
      return boot.previewMaterializer.isSsrInjectedPreviewRoot(root);
    }
    if (boot.previewMaterializer?.isClientLayerMaterialized?.(root)) {
      return false;
    }
    return (
      typeof boot.hasMaterializedPreview === "function" && boot.hasMaterializedPreview(root)
    );
  }

  function vrCtxFromViewCtx(ctx) {
    const resolved =
      typeof boot.resolveComposeKeyCtx === "function"
        ? boot.resolveComposeKeyCtx(ctx)
        : ctx || {};
    return {
      app_id: resolved.app_id || resolved.appId,
      scene_id: resolved.scene_id || resolved.sceneId,
      surface: resolved.surface || resolved.mode || "app",
      node: resolved.node || "",
      data_mode: resolved.data_mode || resolved.dataMode || "",
      review_projection: resolved.review_projection || resolved.reviewProjection || "",
      chrome: resolved.chrome || "",
      tab: resolved.tab || "",
      focus: resolved.focus || "",
      scope: resolved.scope || "",
    };
  }

  function rememberSurfaceFromManifestRefs(ctx) {
    const refs = globalThis.__mei?.scene_manifest_refs;
    if (!refs?.layers || typeof boot.rememberViewRevision !== "function") return;
    const vrCtx = vrCtxFromViewCtx(ctx);
    const routeMode = String(refs.compose_defaults?.route_mode || "")
      .trim()
      .toLowerCase();
    const surface = String(vrCtx.surface || "app").trim().toLowerCase();
    if (routeMode && routeMode !== surface) return;
    boot.rememberViewRevision(vrCtx, {
      manifest_revision_digest: refs.revision_digest || refs.manifest_revision_digest,
      surface_revision_digest: refs.surface_revision_digest,
      manifest: refs,
    });
  }

  function surfaceRuntimeKey(ctx) {
    const vrCtx = vrCtxFromViewCtx(ctx);
    return [
      vrCtx.app_id,
      vrCtx.scene_id,
      vrCtx.surface,
    ]
      .filter(Boolean)
      .join(":");
  }

  function markSurfaceRuntimeWarmed(ctx) {
    const key = surfaceRuntimeKey(ctx);
    if (!key) return;
    const warmed =
      global.__meiSurfaceRuntimeWarmed ||
      (global.__meiSurfaceRuntimeWarmed = new Set());
    warmed.add(key);
  }

  function isSurfaceRuntimeWarmed(ctx) {
    const key = surfaceRuntimeKey(ctx);
    if (!key) return false;
    return Boolean(global.__meiSurfaceRuntimeWarmed?.has(key));
  }

  async function completeMaterializedSurface(ctx, options) {
    const opts = options || {};
    if (!opts.skipHydrate && typeof boot.hydrateManifestLayerHoldings === "function") {
      boot.hydrateManifestLayerHoldings();
    }
    if (!opts.skipRemember) rememberSurfaceFromManifestRefs(ctx);
    if (!opts.skipTree) {
      await ensureWorkspaceStructureTree(ctx, opts.layers || null, {
        generation: opts.generation,
        signal: opts.signal,
      });
    }
    if (typeof boot.hideThinShellFallback === "function" && !opts.skipHideFallback) {
      boot.hideThinShellFallback();
    }
    if (opts.skipRuntimeWake) return;
    await wakeRevisionFirstShellRuntime(ctx, {
      ssrPreview: opts.ssrPreview !== false,
      warmOnly: opts.warmOnly === true,
      forceRuntimeWake: opts.forceRuntimeWake === true,
    });
  }

  async function finishRevisionFirstColdStart(ctx, outcome) {
    const freshCtx =
      typeof boot.parseViewContext === "function"
        ? boot.parseViewContext(window.location.href)
        : ctx;
    if (boot.viewAssembly?.assemble && globalThis.__mei?.view_assembly_v2 !== false) {
      const resolved = outcome || { restored: false };
      if (
        resolved?.restored &&
        (resolved.source === "ssr_preview" || resolved.source === "client_cache" || resolved.source === "coordinator")
      ) {
        return resolved;
      }
      const result = await boot.viewAssembly.assemble(
        { kind: "cold_start", ...(freshCtx || {}) },
        { debounce: false },
      );
      if (result?.ok) {
        return { ...resolved, restored: true, source: "coordinator" };
      }
    }
    // Legacy fallback when view_assembly_v2 is disabled or coordinator assemble failed.
    let resolved = outcome || { restored: false };
    const surface = ctx?.surface || ctx?.mode || "app";
    const composeRoot =
      typeof boot.resolveComposeRoot === "function"
        ? boot.resolveComposeRoot(surface)
        : document.querySelector(".shell");
    const ssrPreviewReady = isSsrInjectedPreviewRoot(composeRoot);
    if (
      resolved?.restored &&
      (resolved.source === "ssr_preview" || resolved.source === "client_cache")
    ) {
      if (typeof boot.hideThinShellFallback === "function") {
        boot.hideThinShellFallback();
      }
      return resolved;
    }
    if (ssrPreviewReady) {
      if (typeof boot.hideThinShellFallback === "function") {
        boot.hideThinShellFallback();
      }
      await completeMaterializedSurface(ctx, { ssrPreview: true });
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
      const missing =
        resolved?.viewRevision?.assemble?.missing ||
        boot.lastViewRevisionOutcome === (boot.ViewRevisionOutcome?.LOCAL_MISS || "local_miss")
          ? ["view-revision local_miss"]
          : [];
      const detail = missing.length ? ` 缺失层: ${missing.join(", ")}` : "";
      boot.showThinShellFallback(`场景内容暂时无法加载，请检查 layer 组装。${detail}`);
    } else if (typeof boot.hideThinShellFallback === "function") {
      boot.hideThinShellFallback();
    }
    return resolved;
  }

  /**
   * @deprecated Prefer boot.viewAssembly.assemble; kept for __mei.view_assembly_v2 rollback.
   */
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
      const result = await boot.viewRevisionClient.negotiateWithLocalMiss(vrCtx, opts);
      if (!result?.assemble?.ok) {
        return result;
      }
      const surface = vrCtx.surface || "app";
      if (opts.surfaceSwitch) {
        clearSurfaceRuntimeWarmedForApp(vrCtx);
      }
      if (!opts.skipComplete) {
        if (typeof boot.switchSurfacePanel === "function") {
          boot.switchSurfacePanel(surface);
        } else if (surface === "layout" || surface === "prototype") {
          if (typeof boot.installManageTabs === "function") {
            boot.installManageTabs();
          }
          if (typeof globalThis.MeiBuildTreePersist?.refresh === "function") {
            globalThis.MeiBuildTreePersist.refresh();
          }
        }
        if (typeof boot.syncTopbarActiveState === "function") {
          boot.syncTopbarActiveState(surface);
        }
      }
      if (typeof boot.rememberViewRevision === "function" && result.response) {
        const rememberPayload = {
          ...result.response,
          manifest:
            result.response.manifest ||
            result.plan?.manifest ||
            result.response.assembly_plan?.manifest ||
            null,
        };
        boot.rememberViewRevision(vrCtx, rememberPayload);
      }
      const cachedOnly = Boolean(result.response?.cached_only);
      const assembleLocal =
        result.outcome === (boot.ViewRevisionOutcome?.ASSEMBLE_LOCAL || "assemble_local");
      if (!opts.skipComplete) {
        await completeMaterializedSurface(viewCtx, {
          layers: result.assemble?.layers,
          ssrPreview: result.assemble?.source === "ssr_preview",
          warmOnly: (cachedOnly || assembleLocal) && !opts.surfaceSwitch,
          forceRuntimeWake: opts.surfaceSwitch === true,
          generation: opts.generation,
          signal: opts.signal,
        });
      }
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
    const skipComplete = opts.viaCoordinator === true || opts.skipComplete === true;
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
      if (isSsrInjectedPreviewRoot(composeRoot)) {
        if (boot.hostChromeReady?.()) {
          if (typeof boot.hideThinShellFallback === "function") {
            boot.hideThinShellFallback();
          }
          const vrCtx = vrCtxFromViewCtx(ctx);
          const cachedOnly = await boot.viewRevisionClient?.tryClientOnlyAssemble?.(vrCtx);
          if (cachedOnly?.ok) {
            if (!skipComplete) {
              await completeMaterializedSurface(ctx, {
                layers: cachedOnly.layers,
                ssrPreview: true,
                warmOnly: true,
                generation: opts.generation,
              });
            }
            return {
              restored: true,
              doc: document,
              revision: boot.readViewRevision?.(vrCtx) || null,
              source: "client_cache",
              viewRevision: { assemble: cachedOnly, layers: cachedOnly.layers },
            };
          }
          if (!skipComplete) {
            await completeMaterializedSurface(ctx, { ssrPreview: true, warmOnly: true, generation: opts.generation });
          }
          return {
            restored: true,
            doc: document,
            revision: globalThis.__mei?.scene_manifest_refs || null,
            source: "ssr_preview",
          };
        }
        /* chrome not ready: fall through to negotiateAndAssemble */
      }
    }
    if (!ctx) {
      return { restored: false, doc: null, revision: null, source: "none" };
    }
    const negotiated = await negotiateAndAssemble(ctx, {
      ...opts,
      skipComplete,
      generation: opts.generation,
      signal: opts.signal,
      omit_digests:
        opts.omit_digests === true ||
        (typeof boot.isSsrShellPlaceholder === "function" && boot.isSsrShellPlaceholder(ctx)),
      forceRematerialize:
        opts.forceRematerialize === true ||
        (typeof boot.isSsrShellPlaceholder === "function" && boot.isSsrShellPlaceholder(ctx)),
    });
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
      if (result?.assemble?.ok && result.assemble.materialized !== false) {
        const outcome =
          result.outcome === (boot.ViewRevisionOutcome?.REFETCH || "refetch")
            ? "refetch"
            : (boot.ViewRevisionOutcome?.ASSEMBLE_LOCAL || "assemble_local");
        if (typeof boot.cacheDiagTrace === "function") {
          boot.cacheDiagTrace("view-revision-outcome", {
            outcome,
          });
        }
        const viewCtx =
          typeof boot.parseViewContext === "function"
            ? boot.parseViewContext(ctx.url || window.location.href)
            : ctx;
        await completeMaterializedSurface(viewCtx || ctx, {
          layers: result.assemble?.layers,
          ssrPreview: result.assemble?.source === "ssr_preview",
          warmOnly: false,
          forceRuntimeWake: true,
        });
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
  boot.completeMaterializedSurface = completeMaterializedSurface;
  boot.clearSurfaceRuntimeWarmedForApp = clearSurfaceRuntimeWarmedForApp;
  boot.wakeRevisionFirstShellRuntime = wakeRevisionFirstShellRuntime;
  boot.fetchSceneFragment = fetchSceneFragment;
  boot.tryRestoreSceneShellFromFragment = tryRestoreSceneShellFromFragment;
  boot.negotiateAndAssemble = negotiateAndAssemble;
  boot.tryCacheFirstViewRestore = tryCacheFirstViewRestore;
  boot.tryCacheFirstSceneAccess = tryCacheFirstSceneAccess;
