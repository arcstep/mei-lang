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
