/**
 * ViewAssemblyCoordinator: single cold_start pipeline for F5 and surface tab switches.
 */
(function initViewAssemblyCoordinator(global) {
  "use strict";

  const boot = (global.__meiLangBoot = global.__meiLangBoot || {});
  const DEBOUNCE_MS = 50;

  let assemblyGeneration = 0;
  let activeController = null;
  let debounceTimer = null;
  let pendingIntent = null;
  const layerResidentWaiters = new Map();

  function isEnabled() {
    return globalThis.__mei?.view_assembly_v2 !== false;
  }

  function parseIntent(intentLike) {
    if (intentLike?.app_id || intentLike?.appId) {
      return typeof boot.parseViewContext === "function"
        ? boot.parseViewContext(intentLike.url || global.location.href)
        : intentLike;
    }
    if (typeof boot.parseViewContext === "function") {
      return boot.parseViewContext(global.location.href);
    }
    return null;
  }

  function isSurfaceSwitch(opts) {
    const payload = opts || {};
    return (
      payload.surfaceSwitch === true ||
      payload.kind === "surface_switch" ||
      payload.kind === "spa_nav"
    );
  }

  function normalizeAssemblyOpts(intentLike, options) {
    const intent = intentLike || {};
    const optionsIn = options || {};
    const surfaceSwitch = isSurfaceSwitch({ ...optionsIn, ...intent });
    return {
      ...optionsIn,
      ...intent,
      kind: "cold_start",
      surfaceSwitch,
      omit_digests: surfaceSwitch || optionsIn.omit_digests === true || intent.omit_digests === true,
      forceRuntimeWake: surfaceSwitch || optionsIn.forceRuntimeWake === true,
      previousSurface: intent.previousSurface || optionsIn.previousSurface || "",
    };
  }

  function isStale(generation, signal) {
    if (generation !== assemblyGeneration) return true;
    if (signal?.aborted) return true;
    return false;
  }

  function tracePhase(phase, generation, extra) {
    if (typeof boot.cacheDiagTrace === "function") {
      boot.cacheDiagTrace("assembly-phase", { phase, generation, ...(extra || {}) });
    }
    try {
      global.document?.dispatchEvent(
        new CustomEvent("mei:assembly-phase", {
          detail: { phase, generation, ...(extra || {}) },
        }),
      );
    } catch (_) {}
  }

  function cancel() {
    assemblyGeneration += 1;
    if (activeController) {
      try {
        activeController.abort();
      } catch (_) {}
      activeController = null;
    }
    if (debounceTimer) {
      clearTimeout(debounceTimer);
      debounceTimer = null;
    }
    pendingIntent = null;
    try {
      global.document?.dispatchEvent(
        new CustomEvent("mei:abort-runtime-queries", {
          detail: { reason: "view_assembly_cancel", clearCaches: false },
        }),
      );
    } catch (_) {}
    return assemblyGeneration;
  }

  function onLayerResident(layerName, callback) {
    const key = String(layerName || "").trim();
    if (!key || typeof callback !== "function") return () => {};
    const list = layerResidentWaiters.get(key) || [];
    list.push(callback);
    layerResidentWaiters.set(key, list);
    return () => {
      const cur = layerResidentWaiters.get(key) || [];
      layerResidentWaiters.set(
        key,
        cur.filter((fn) => fn !== callback),
      );
    };
  }

  function notifyLayerResident(layerName, layers, generation) {
    const key = String(layerName || "").trim();
    if (!key || !layers?.[key]) return;
    const waiters = layerResidentWaiters.get(key) || [];
    for (const fn of waiters) {
      try {
        fn(layers, generation);
      } catch (error) {
        console.warn("[view-assembly] layer resident callback failed", error);
      }
    }
  }

  function syncManifestRefs(manifest, ctx) {
    if (!manifest || typeof manifest !== "object") return;
    if (typeof boot.applySceneManifestRefs === "function") {
      boot.applySceneManifestRefs(manifest, ctx);
      return;
    }
    if (!manifest.layers) return;
    globalThis.__mei = globalThis.__mei || {};
    const prev = globalThis.__mei.scene_manifest_refs || {};
    globalThis.__mei.scene_manifest_refs = {
      ...prev,
      ...manifest,
      layers: { ...(prev.layers || {}), ...manifest.layers },
    };
    delete globalThis.__mei.scene_manifest_refs_stale;
  }

  function syncManifestForSurface(ctx) {
    if (typeof boot.applySceneManifestRefs !== "function") return;
    const refs = globalThis.__mei?.scene_manifest_refs;
    syncManifestRefs(
      {
        layers: refs?.layers || {},
        revision_digest: refs?.revision_digest,
        surface_revision_digest: refs?.surface_revision_digest,
      },
      ctx,
    );
  }

  async function waitForSurfaceMaterialized(ctx, generation, signal) {
    if (typeof boot.isSurfaceMaterialized !== "function") return true;
    let ready = boot.isSurfaceMaterialized(ctx);
    const surface = ctx.surface || ctx.mode || "app";
    const isWorkspace =
      typeof boot.isWorkspaceComposeSurface === "function" &&
      boot.isWorkspaceComposeSurface(surface);
    if (!ready && isWorkspace) {
      const deadline = performance.now() + 3000;
      while (!ready && performance.now() < deadline && !isStale(generation, signal)) {
        await new Promise((resolve) => setTimeout(resolve, 100));
        ready = boot.isSurfaceMaterialized(ctx);
      }
    }
    if (!ready && isWorkspace) {
      ready = boot.isSurfaceMaterialized(ctx, { relaxTree: true });
    }
    return ready;
  }

  async function phasePanel(ctx, generation, opts) {
    if (isStale(generation)) return;
    const surface = ctx.surface || ctx.mode || "app";
    if (typeof boot.switchSurfacePanel === "function") {
      boot.switchSurfacePanel(surface, { skipPreviewRestore: isSurfaceSwitch(opts) });
    }
    if (typeof boot.syncTopbarActiveState === "function") {
      boot.syncTopbarActiveState(surface);
    }
    syncManifestForSurface(ctx);
    if (isSurfaceSwitch(opts) && typeof boot.clearSurfaceRuntimeWarmedForApp === "function") {
      boot.clearSurfaceRuntimeWarmedForApp(ctx);
    }
    tracePhase("panel", generation, { surface, surfaceSwitch: isSurfaceSwitch(opts) });
  }

  async function phaseChrome(ctx, generation) {
    if (isStale(generation)) return;
    if (typeof boot.ensureSceneDrilldownContext === "function") {
      try {
        await boot.ensureSceneDrilldownContext(ctx || {});
      } catch (error) {
        console.warn("[view-assembly] drilldown context load skipped", error);
      }
    }
    if (typeof boot.ensureViewShellLayout === "function") {
      boot.ensureViewShellLayout();
    }
    if (typeof boot.applyHostChromeFromManifestRefs === "function") {
      boot.applyHostChromeFromManifestRefs();
    }
    tracePhase("chrome", generation);
  }

  async function phaseStructureTree(ctx, generation, layers, signal) {
    if (isStale(generation, signal)) return;
    const surface = ctx.surface || ctx.mode || "app";
    if (typeof boot.isWorkspaceComposeSurface === "function" && !boot.isWorkspaceComposeSurface(surface)) {
      return;
    }
    if (typeof boot.ensureWorkspaceStructureTree === "function") {
      await boot.ensureWorkspaceStructureTree(ctx, layers || null, { generation, signal });
    }
    tracePhase("structure_tree", generation);
    if (typeof boot.isWorkspaceComposeSurface === "function" && boot.isWorkspaceComposeSurface(surface)) {
      if (typeof boot.hideThinShellFallback === "function") {
        boot.hideThinShellFallback();
      }
    }
  }

  async function phasePreview(ctx, generation, opts, signal) {
    if (isStale(generation, signal)) return null;
    const surfaceSwitch = isSurfaceSwitch(opts);
    const negotiateOpts = {
      silent: true,
      surfaceSwitch,
      forceRematerialize: surfaceSwitch || opts.forceRematerialize === true,
      omit_digests: opts.omit_digests === true,
      previousSurface: opts.previousSurface || "",
      skipComplete: true,
      signal,
      generation,
    };
    let result = null;
    if (typeof boot.tryCacheFirstViewRestore === "function" && !surfaceSwitch) {
      const outcome = await boot.tryCacheFirstViewRestore(ctx.url || global.location.href, {
        coldStart: true,
        skipRemoteWhenValid: true,
        timeoutMs: 4000,
        signal,
        viaCoordinator: true,
        generation,
      });
      if (outcome?.restored) {
        const layers = outcome.viewRevision?.assemble?.layers || outcome.layers || null;
        if (layers) notifyLayerResident("structure.full", layers, generation);
        syncManifestForSurface(ctx);
        if (!isStale(generation, signal) && typeof boot.applyHostChromeFromManifestRefs === "function") {
          boot.applyHostChromeFromManifestRefs();
        }
        const surfaceReady =
          typeof boot.isSurfaceMaterialized === "function"
            ? boot.isSurfaceMaterialized(ctx)
            : (typeof boot.hostChromeReady === "function" ? boot.hostChromeReady() : true) &&
              (typeof boot.isSsrShellPlaceholder === "function"
                ? !boot.isSsrShellPlaceholder(ctx)
                : true);
        if (surfaceReady) {
          tracePhase("preview", generation, { ok: true, source: outcome.source, cacheFirst: true });
          return {
            outcome,
            assemble: { ok: true, ...(outcome.viewRevision?.assemble || {}), layers },
            layers,
          };
        }
        negotiateOpts.forceRematerialize = true;
        negotiateOpts.omit_digests = true;
      }
    }
    if (typeof boot.negotiateAndAssemble === "function") {
      result = await boot.negotiateAndAssemble(
        { ...ctx, url: ctx.url || global.location.href },
        negotiateOpts,
      );
    }
    if (result?.assemble?.layers) {
      notifyLayerResident("structure.full", result.assemble.layers, generation);
      const manifest =
        result.response?.manifest ||
        result.plan?.manifest ||
        result.response?.assembly_plan?.manifest;
      syncManifestRefs(manifest || { layers: result.assemble.layers }, ctx);
    }
    if (!isStale(generation, signal) && typeof boot.applyHostChromeFromManifestRefs === "function") {
      boot.applyHostChromeFromManifestRefs();
    }
    tracePhase("preview", generation, { ok: !!result?.assemble?.ok });
    return result;
  }

  async function retryPreviewForSurfaceReady(ctx, generation, opts, signal) {
    if (!boot.negotiateAndAssemble) return null;
    return boot.negotiateAndAssemble(
      { ...ctx, url: ctx.url || global.location.href },
      {
        silent: true,
        surfaceSwitch: isSurfaceSwitch(opts),
        forceRematerialize: true,
        omit_digests: true,
        skipComplete: true,
        signal,
        generation,
      },
    );
  }

  async function phaseVerify(ctx, generation, signal) {
    if (isStale(generation, signal)) return { ok: false };
    const ready = await waitForSurfaceMaterialized(ctx, generation, signal);
    tracePhase("verify", generation, { ready });
    return { ok: ready };
  }

  async function phaseRuntime(ctx, generation, previewResult, opts, signal) {
    if (isStale(generation, signal)) return;
    const layers = previewResult?.assemble?.layers || previewResult?.layers;
    const surfaceSwitch = isSurfaceSwitch(opts);
    const cachedOnly = Boolean(previewResult?.response?.cached_only);
    const assembleLocal =
      previewResult?.outcome === (boot.ViewRevisionOutcome?.ASSEMBLE_LOCAL || "assemble_local");
    const warmOnly = !surfaceSwitch && (cachedOnly || assembleLocal);
    if (typeof boot.ensureSurfaceRuntime === "function") {
      await boot.ensureSurfaceRuntime(ctx, {
        force: surfaceSwitch,
        layers,
        forceRuntimeWake: surfaceSwitch,
        warmOnly,
        ssrPreview: previewResult?.assemble?.source === "ssr_preview",
      });
    } else if (typeof boot.completeMaterializedSurface === "function") {
      await boot.completeMaterializedSurface(ctx, {
        layers,
        ssrPreview: previewResult?.assemble?.source === "ssr_preview",
        warmOnly,
        forceRuntimeWake: surfaceSwitch,
        skipTree: true,
        generation,
        signal,
      });
    } else if (typeof boot.wakeRevisionFirstShellRuntime === "function") {
      await boot.wakeRevisionFirstShellRuntime(ctx, {
        forceRuntimeWake: surfaceSwitch,
      });
    }
    const surface = ctx.surface || ctx.mode || "app";
    if (typeof boot.captureSurfacePreviewSnapshot === "function") {
      boot.captureSurfacePreviewSnapshot(surface);
    }
    tracePhase("runtime", generation);
    try {
      global.document?.dispatchEvent(new CustomEvent("mei:spa-navigation-complete"));
    } catch (_) {}
  }

  async function assembleInternal(intentLike, options) {
    if (!isEnabled()) {
      return { ok: false, reason: "disabled" };
    }
    await boot.hostCapabilitiesReady?.({ timeoutMs: 5000 });
    const generation = options?.generation ?? cancel();
    const signal = options?.signal || null;
    const opts = normalizeAssemblyOpts(intentLike, options);
    const ctx = parseIntent(opts);
    if (!ctx) return { ok: false, reason: "no_context", generation };

    const started = performance.now();

    await phasePanel(ctx, generation, opts);
    await phaseStructureTree(ctx, generation, null, signal);

    let previewResult = await phasePreview(ctx, generation, opts, signal);
    let layers = previewResult?.assemble?.layers || previewResult?.layers;

    if (
      previewResult?.assemble?.ok === true &&
      typeof boot.isSurfaceMaterialized === "function" &&
      !boot.isSurfaceMaterialized(ctx) &&
      !opts._surfaceReadyRetried
    ) {
      const retry = await retryPreviewForSurfaceReady(ctx, generation, opts, signal);
      if (retry?.assemble?.layers) {
        notifyLayerResident("structure.full", retry.assemble.layers, generation);
        const manifest =
          retry.response?.manifest || retry.plan?.manifest || retry.response?.assembly_plan?.manifest;
        syncManifestRefs(manifest || { layers: retry.assemble.layers }, ctx);
        previewResult = retry;
        layers = retry.assemble.layers;
      }
    }

    await phaseStructureTree(ctx, generation, layers, signal);

    const verify = await phaseVerify(ctx, generation, signal);
    if (previewResult?.assemble?.ok === true && !verify.ok) {
      if (typeof boot.isSurfaceMaterialized === "function" && boot.isSurfaceMaterialized(ctx, { relaxTree: true })) {
        tracePhase("verify", generation, { ready: true, relaxed: true });
      } else {
        previewResult = {
          ...previewResult,
          assemble: {
            ...(previewResult.assemble || {}),
            ok: false,
            reason: "surface_not_materialized",
          },
        };
      }
    }

    await phaseChrome(ctx, generation);

    if (previewResult?.assemble?.ok !== true) {
      tracePhase("failed", generation, { ms: Math.round(performance.now() - started) });
      return { ok: false, generation, preview: previewResult };
    }

    await phaseRuntime(ctx, generation, previewResult, opts, signal);

    tracePhase("complete", generation, { ms: Math.round(performance.now() - started) });
    return { ok: true, generation, preview: previewResult };
  }

  function assemble(intentLike, options) {
    const opts = normalizeAssemblyOpts(intentLike, options);
    if (opts.debounce === false || opts.kind === "cold_start" || opts.surfaceSwitch) {
      cancel();
      activeController = new AbortController();
      const generation = assemblyGeneration;
      return assembleInternal(intentLike, {
        ...options,
        generation,
        signal: activeController.signal,
      });
    }
    pendingIntent = { intentLike, options: opts };
    if (debounceTimer) clearTimeout(debounceTimer);
    return new Promise((resolve) => {
      debounceTimer = setTimeout(() => {
        debounceTimer = null;
        const pending = pendingIntent;
        pendingIntent = null;
        if (!pending) {
          resolve({ ok: false, reason: "debounce_cancelled" });
          return;
        }
        cancel();
        activeController = new AbortController();
        const generation = assemblyGeneration;
        resolve(
          assembleInternal(pending.intentLike, {
            ...pending.options,
            generation,
            signal: activeController.signal,
          }),
        );
      }, DEBOUNCE_MS);
    });
  }

  boot.viewAssembly = {
    assemble,
    cancel,
    onLayerResident,
    getState: () => ({
      generation: assemblyGeneration,
      enabled: isEnabled(),
    }),
  };
})(typeof window !== "undefined" ? window : globalThis);
