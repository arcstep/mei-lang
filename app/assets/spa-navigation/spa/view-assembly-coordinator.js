/**
 * ViewAssemblyCoordinator: single entry for cold start / surface switch / SPA nav assembly.
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

  function updateSceneManifestRefs(manifest, surface) {
    if (!manifest?.layers) return;
    globalThis.__mei = globalThis.__mei || {};
    const prev = globalThis.__mei.scene_manifest_refs || {};
    globalThis.__mei.scene_manifest_refs = {
      ...prev,
      ...manifest,
      layers: { ...(prev.layers || {}), ...manifest.layers },
      compose_defaults: {
        ...(prev.compose_defaults || {}),
        ...(manifest.compose_defaults || {}),
        route_mode:
          surface ||
          manifest.compose_defaults?.route_mode ||
          prev.compose_defaults?.route_mode,
      },
    };
    delete globalThis.__mei.scene_manifest_refs_stale;
  }

  async function phasePanel(ctx, generation) {
    if (isStale(generation)) return;
    const surface = ctx.surface || ctx.mode || "app";
    if (typeof boot.switchSurfacePanel === "function") {
      boot.switchSurfacePanel(surface);
    }
    if (typeof boot.syncTopbarActiveState === "function") {
      boot.syncTopbarActiveState(surface);
    }
    tracePhase("panel", generation, { surface });
  }

  async function phaseChrome(ctx, generation) {
    if (isStale(generation)) return;
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

  async function phasePreview(ctx, generation, options, signal) {
    if (isStale(generation, signal)) return null;
    const opts = options || {};
    const negotiateOpts = {
      silent: true,
      surfaceSwitch: opts.kind === "surface_switch",
      forceRematerialize: opts.kind === "surface_switch" || opts.forceRematerialize === true,
      previousSurface: opts.previousSurface || "",
      skipComplete: true,
      signal,
      generation,
    };
    if (opts.kind === "surface_switch" && typeof boot.clearSurfaceRuntimeWarmedForApp === "function") {
      boot.clearSurfaceRuntimeWarmedForApp(ctx);
    }
    let result = null;
    if (opts.kind === "cold_start" && typeof boot.tryCacheFirstViewRestore === "function") {
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
        tracePhase("preview", generation, { ok: true, source: outcome.source });
        return { outcome, assemble: outcome.viewRevision?.assemble || { ok: true, layers }, layers };
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
      if (manifest) {
        updateSceneManifestRefs(manifest, ctx.surface || ctx.mode);
      }
    }
    tracePhase("preview", generation, { ok: !!result?.assemble?.ok });
    return result;
  }

  async function phaseRuntime(ctx, generation, previewResult, options, signal) {
    if (isStale(generation, signal)) return;
    const opts = options || {};
    const layers = previewResult?.assemble?.layers || previewResult?.layers;
    const cachedOnly = Boolean(previewResult?.response?.cached_only);
    const assembleLocal =
      previewResult?.outcome === (boot.ViewRevisionOutcome?.ASSEMBLE_LOCAL || "assemble_local");
    const warmOnly =
      opts.kind !== "surface_switch" &&
      opts.kind !== "spa_nav" &&
      (cachedOnly || assembleLocal);
    if (typeof boot.completeMaterializedSurface === "function") {
      await boot.completeMaterializedSurface(ctx, {
        layers,
        ssrPreview: previewResult?.assemble?.source === "ssr_preview",
        warmOnly: warmOnly && !opts.forceRuntimeWake,
        forceRuntimeWake: opts.kind === "surface_switch" || opts.kind === "spa_nav",
        skipTree: true,
        generation,
        signal,
      });
    } else if (typeof boot.wakeRevisionFirstShellRuntime === "function") {
      await boot.wakeRevisionFirstShellRuntime(ctx, {
        forceRuntimeWake: opts.kind === "surface_switch",
      });
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
    const ctx = parseIntent(intentLike);
    if (!ctx) return { ok: false, reason: "no_context", generation };

    const opts = { ...(options || {}), ...(intentLike || {}) };
    const started = performance.now();

    await phasePanel(ctx, generation);
    await phaseChrome(ctx, generation);
    await phaseStructureTree(ctx, generation, null, signal);

    const previewResult = await phasePreview(ctx, generation, opts, signal);
    const layers = previewResult?.assemble?.layers || previewResult?.layers;
    await phaseStructureTree(ctx, generation, layers, signal);

    if (previewResult?.assemble?.ok === false) {
      tracePhase("failed", generation, { ms: Math.round(performance.now() - started) });
      return { ok: false, generation, preview: previewResult };
    }

    await phaseRuntime(ctx, generation, previewResult, opts, signal);

    tracePhase("complete", generation, { ms: Math.round(performance.now() - started) });
    return { ok: true, generation, preview: previewResult };
  }

  function assemble(intentLike, options) {
    const opts = options || {};
    if (opts.debounce === false || opts.kind === "cold_start") {
      cancel();
      activeController = new AbortController();
      const generation = assemblyGeneration;
      return assembleInternal(intentLike, {
        ...opts,
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
