  async function bootstrapThinShellComposition() {
    if (globalThis.__mei?.thin_shell !== true) return false;
    if (!boot.sceneManifestLoader?.ensureAccessComposeLayers || !boot.viewCompositor?.composePreview) {
      return false;
    }
    const ctx =
      typeof boot.parseAccessSceneContext === "function"
        ? boot.parseAccessSceneContext(global.location.href)
        : null;
    if (!ctx?.appId || !ctx?.sceneId) return false;
    const shell = global.document?.querySelector?.(".shell");
    if (!(shell instanceof HTMLElement)) return false;
    try {
      const { structure, theme, overlay } = await boot.sceneManifestLoader.ensureAccessComposeLayers(
        ctx.appId,
        ctx.sceneId,
        "app",
      );
      if (!structure) return false;
      const projection =
        ctx.reviewProjection ||
        String(shell.getAttribute("data-review-projection") || "").trim() ||
        "live_full";
      boot.viewCompositor.composePreview(shell, structure, projection, theme, overlay);
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
        isBuildWorkspacePathname(window.location.pathname)
      ) {
        return;
      }
      const sceneCtx =
        typeof boot.parseAccessSceneContext === "function"
          ? boot.parseAccessSceneContext(window.location.href)
          : null;
      if (!sceneCtx?.sceneId) return;

      const run = () => {
        void bootstrapThinShellComposition().then(() => {
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
