/**
 * Unified surface runtime wake: workspace manage preview + app compose activation.
 */
(function initEnsureSurfaceRuntime(global) {
  "use strict";

  const boot = (global.__meiLangBoot = global.__meiLangBoot || {});
  const doc = global.document;

  async function composeAppPreviewIfNeeded(ctx, layers, force) {
    const surface = ctx?.surface || ctx?.mode || "app";
    if (surface !== "app") return false;
    if (typeof boot.restoreAppPreviewSnapshot === "function") {
      boot.restoreAppPreviewSnapshot();
    }
    const root =
      typeof boot.resolveComposeRoot === "function"
        ? boot.resolveComposeRoot(surface)
        : doc?.getElementById?.("mei-compose-root");
    if (!(root instanceof HTMLElement)) return false;
    const materialized =
      typeof boot.hasMaterializedPreview === "function" && boot.hasMaterializedPreview(root);
    if (materialized && !force) return true;
    if (!boot.viewCompositor?.composeFromLayers || !layers) return false;
    const composeAxes =
      typeof boot.viewRevisionClient?.buildComposeRequest === "function"
        ? boot.viewRevisionClient.buildComposeRequest(ctx)
        : boot.composeDefaultsForSurface?.(ctx) || {};
    return boot.viewCompositor.composeFromLayers(root, layers, {
      ...composeAxes,
      forceRematerialize: force === true,
    });
  }

  async function ensureSurfaceRuntime(ctx, options) {
    const opts = options || {};
    const force = opts.force === true || opts.forceRuntimeWake === true;
    const layers = opts.layers || null;
    const surface = ctx?.surface || ctx?.mode || "app";

    if (surface === "app") {
      await composeAppPreviewIfNeeded(ctx, layers, force);
    }

    if (typeof boot.wakeRevisionFirstShellRuntime === "function") {
      await boot.wakeRevisionFirstShellRuntime(ctx, {
        ssrPreview: opts.ssrPreview !== false,
        warmOnly: !force && opts.warmOnly === true,
        forceRuntimeWake: force,
      });
      return;
    }

    if (
      typeof boot.isWorkspaceComposeSurface === "function" &&
      boot.isWorkspaceComposeSurface(surface)
    ) {
      if (typeof boot.restoreWorkspacePreviewSnapshot === "function") {
        boot.restoreWorkspacePreviewSnapshot();
      }
      if (typeof boot.applyHostChromeFromManifestRefs === "function") {
        boot.applyHostChromeFromManifestRefs();
      }
      if (typeof boot.mountManagePreviewBoard === "function") {
        await boot.mountManagePreviewBoard(doc);
      }
      if (typeof globalThis.MeiBuildTreePersist?.refresh === "function") {
        globalThis.MeiBuildTreePersist.refresh();
      }
      return;
    }

    if (typeof boot.ensureThinShellSceneRuntime === "function") {
      await boot.ensureThinShellSceneRuntime();
    }
    if (typeof boot.dispatchScopeActivation === "function") {
      boot.dispatchScopeActivation({
        scope: ctx.sceneId || ctx.scene_id || "home",
        sceneId: ctx.sceneId || ctx.scene_id || "home",
        appId: ctx.appId || ctx.app_id || "",
        source: force ? "surface-switch-runtime" : "revision-first-cold-start",
      });
    }
  }

  boot.ensureSurfaceRuntime = ensureSurfaceRuntime;
})(typeof window !== "undefined" ? window : globalThis);
