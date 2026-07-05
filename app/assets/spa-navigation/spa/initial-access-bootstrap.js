  async function bootstrapThinShellComposition() {
    if (typeof isRevisionFirstShellPage === "function" && !isRevisionFirstShellPage()) {
      return false;
    }
    const ctx =
      typeof boot.parseViewContext === "function"
        ? boot.parseViewContext(global.location.href)
        : typeof boot.parseAccessSceneContext === "function"
          ? boot.parseAccessSceneContext(global.location.href)
          : null;
    if (!ctx?.appId && !ctx?.app_id) return false;
    const appId = ctx.appId || ctx.app_id;
    const sceneId = ctx.sceneId || ctx.scene_id || "home";
    const surface = ctx.surface || ctx.mode || "app";
    const composeRoot =
      typeof boot.resolveComposeRoot === "function"
        ? boot.resolveComposeRoot(surface)
        : global.document?.querySelector?.(".shell");
    if (!(composeRoot instanceof HTMLElement)) return false;

    if (boot.viewRevisionClient?.negotiateWithLocalMiss) {
      try {
        const vrCtx = {
          app_id: appId,
          scene_id: sceneId,
          surface,
          node: ctx.node || "",
          data_mode: ctx.data_mode || ctx.dataMode || "",
          review_projection: ctx.review_projection || ctx.reviewProjection || "",
          chrome: ctx.chrome || "",
          tab: ctx.tab || "",
          focus: ctx.focus || "",
          scope: ctx.scope || "",
        };
        const result = await boot.viewRevisionClient.negotiateWithLocalMiss(vrCtx);
        if (result?.assemble?.ok) {
          return true;
        }
      } catch (error) {
        console.warn("[spa-navigation] view-revision thin shell composition skipped", error);
      }
    }

    if (!boot.sceneManifestLoader?.ensureAccessComposeLayers || !boot.viewCompositor?.composePreview) {
      return false;
    }
    try {
      const { structure, theme, overlay } = await boot.sceneManifestLoader.ensureAccessComposeLayers(
        appId,
        sceneId,
        surface,
      );
      if (!structure) return false;
      const projection =
        ctx.reviewProjection ||
        ctx.review_projection ||
        String(composeRoot.getAttribute("data-review-projection") || "").trim() ||
        "live_full";
      boot.viewCompositor.composePreview(composeRoot, structure, projection, theme, overlay);
      return true;
    } catch (error) {
      console.warn("[spa-navigation] thin shell composition skipped", error);
      return false;
    }
  }

  boot.bootstrapThinShellComposition = bootstrapThinShellComposition;

  function bootstrapColdAccessSceneRuntime() {
    try {
      if (
        typeof isBuildWorkspacePathname === "function" &&
        isBuildWorkspacePathname(window.location.pathname) &&
        typeof boot.finishRevisionFirstColdStart === "function"
      ) {
        const ctx =
          typeof boot.parseViewContext === "function"
            ? boot.parseViewContext(window.location.href)
            : null;
        if (ctx) {
          void boot.finishRevisionFirstColdStart(ctx, { restored: false });
          return;
        }
      }
      const sceneCtx =
        typeof boot.parseAccessSceneContext === "function"
          ? boot.parseAccessSceneContext(window.location.href)
          : null;
      if (!sceneCtx?.sceneId) return;

      const run = () => {
        void bootstrapThinShellComposition().then(async () => {
          if (typeof boot.wakeRevisionFirstShellRuntime === "function") {
            await boot.wakeRevisionFirstShellRuntime(sceneCtx);
          } else {
            if (typeof boot.dispatchScopeActivation === "function") {
              boot.dispatchScopeActivation({
                scope: sceneCtx.sceneId,
                sceneId: sceneCtx.sceneId,
                appId: sceneCtx.appId,
                source: "initial-load",
              });
            }
            if (typeof wakeRuntimeAfterSceneBundleLoaded === "function") {
              wakeRuntimeAfterSceneBundleLoaded();
            }
          }
          try {
            document.dispatchEvent(new CustomEvent("mei:spa-navigation-complete"));
          } catch (_) {}
        });
      };

      if (document.readyState === "loading") {
        document.addEventListener("DOMContentLoaded", run, { once: true });
      } else {
        run();
      }
    } catch (error) {
      console.warn("[spa-navigation] initial access bootstrap skipped", error);
    }
  }

  boot.bootstrapColdAccessSceneRuntime = bootstrapColdAccessSceneRuntime;
