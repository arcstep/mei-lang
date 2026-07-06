/**
 * Unified surface runtime wake: workspace manage preview + app compose activation.
 */
(function initEnsureSurfaceRuntime(global) {
  "use strict";

  const boot = (global.__meiLangBoot = global.__meiLangBoot || {});
  const doc = global.document;

  async function tryHydratePlaceholderPreview(ctx, root, force, layers, options) {
    if (!(root instanceof HTMLElement)) return false;
    const placeholder = root.getAttribute("data-mei-compose-placeholder") === "1";
    const ssrReady = boot.previewMaterializer?.isSsrInjectedPreviewRoot?.(root) === true;
    if (!placeholder && !force) return false;
    if (ssrReady && !placeholder) return true;
    if (typeof boot.previewMaterializer?.materializePlaceholderPreview === "function") {
      try {
        const composeAxes =
          typeof boot.viewRevisionClient?.buildComposeRequest === "function"
            ? boot.viewRevisionClient.buildComposeRequest(ctx)
            : boot.composeDefaultsForSurface?.(ctx) || {};
        const result = await boot.previewMaterializer.materializePlaceholderPreview(
          ctx,
          root,
          layers,
          {
            ...(options || {}),
            composeAxes,
            forceRematerialize: force === true || placeholder,
          },
        );
        if (result?.ok) {
          return result.source === "fragment" ? "fragment" : result.source || true;
        }
      } catch (error) {
        console.warn("[spa-navigation] preview materialize skipped", error);
      }
    }
    if (typeof boot.previewMaterializer?.hydratePlaceholderFromFragment !== "function") {
      return false;
    }
    try {
      const hydrated = await boot.previewMaterializer.hydratePlaceholderFromFragment(ctx, root);
      return hydrated ? "fragment" : false;
    } catch (error) {
      console.warn("[spa-navigation] preview fragment hydrate skipped", error);
      return false;
    }
  }

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
    const placeholder = root.getAttribute("data-mei-compose-placeholder") === "1";
    const materialized =
      typeof boot.hasMaterializedPreview === "function" && boot.hasMaterializedPreview(root);
    if (materialized && !force && !placeholder) return true;
    if (placeholder || force) {
      const hydrated = await tryHydratePlaceholderPreview(ctx, root, force, layers);
      if (hydrated) {
        if (typeof boot.stashAppPreviewSnapshot === "function") {
          boot.stashAppPreviewSnapshot();
        }
        return hydrated === "fragment" ? "fragment" : hydrated;
      }
    }
    if (!boot.viewCompositor?.composeFromLayers || !layers) return false;
    const composeAxes =
      typeof boot.viewRevisionClient?.buildComposeRequest === "function"
        ? boot.viewRevisionClient.buildComposeRequest(ctx)
        : boot.composeDefaultsForSurface?.(ctx) || {};
    return boot.viewCompositor.composeFromLayers(root, layers, {
      ...composeAxes,
      forceRematerialize: force === true || placeholder,
    });
  }

  async function composeWorkspacePreviewIfNeeded(ctx, layers, force) {
    const surface = ctx?.surface || ctx?.mode || "app";
    if (
      typeof boot.isWorkspaceComposeSurface !== "function" ||
      !boot.isWorkspaceComposeSurface(surface)
    ) {
      return false;
    }
    const root =
      typeof boot.resolveComposeRoot === "function"
        ? boot.resolveComposeRoot(surface)
        : doc?.querySelector?.("#mei-surface-workspace .preview-pane-scroll");
    if (!(root instanceof HTMLElement)) return false;
    const placeholder = root.getAttribute("data-mei-compose-placeholder") === "1";
    const materialized =
      typeof boot.hasMaterializedPreview === "function" && boot.hasMaterializedPreview(root);
    if (materialized && !force && !placeholder) return true;
    if (placeholder || force) {
      const hydrated = await tryHydratePlaceholderPreview(ctx, root, force, layers);
      if (hydrated) {
        if (typeof boot.stashWorkspacePreviewSnapshot === "function") {
          boot.stashWorkspacePreviewSnapshot();
        }
        return hydrated === "fragment" ? "fragment" : hydrated;
      }
    }
    if (!boot.viewCompositor?.composeFromLayers || !layers) return false;
    const composeAxes =
      typeof boot.viewRevisionClient?.buildComposeRequest === "function"
        ? boot.viewRevisionClient.buildComposeRequest(ctx)
        : boot.composeDefaultsForSurface?.(ctx) || {};
    return boot.viewCompositor.composeFromLayers(root, layers, {
      ...composeAxes,
      forceRematerialize: force === true || placeholder,
    });
  }

  async function ensureSurfaceRuntime(ctx, options) {
    const opts = options || {};
    const force = opts.force === true || opts.forceRuntimeWake === true;
    const layers = opts.layers || null;
    const surface = ctx?.surface || ctx?.mode || "app";
    let hydratedFromFragment = false;

    if (surface === "app") {
      const composed = await composeAppPreviewIfNeeded(ctx, layers, force);
      hydratedFromFragment = composed === "fragment";
    } else if (
      typeof boot.isWorkspaceComposeSurface === "function" &&
      boot.isWorkspaceComposeSurface(surface)
    ) {
      const composed = await composeWorkspacePreviewIfNeeded(ctx, layers, force);
      hydratedFromFragment = composed === "fragment";
    }

    if (typeof boot.wakeRevisionFirstShellRuntime === "function") {
      await boot.wakeRevisionFirstShellRuntime(ctx, {
        ssrPreview: opts.ssrPreview === true || hydratedFromFragment,
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
