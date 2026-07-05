// @ts-nocheck — closes IIFE opened in preamble.js; valid only after bundle concat.
  bootstrapInitialLoadProgress();
  if (typeof boot.installClientCommandWrappers === "function") {
    boot.installClientCommandWrappers();
  }
  tagExistingBodyScripts();
  installSceneProjectionHost();
  applyDrilldownContextFromQuery();
  applySceneProjectionContextFromStorage();

  void (async () => {
    if (typeof boot.tryCacheFirstViewRestore !== "function") return;
    const ctx =
      typeof boot.parseViewContext === "function"
        ? boot.parseViewContext(window.location.href)
        : null;
    try {
      const outcome = await boot.tryCacheFirstViewRestore(window.location.href, {
        replaceHistory: true,
        timeoutMs: 4000,
        coldStart: true,
        skipRemoteWhenValid: true,
      });
      if (outcome.restored && outcome.doc && typeof runPostSpaWork === "function") {
        if (typeof boot.hideThinShellFallback === "function") {
          boot.hideThinShellFallback();
        }
        runPostSpaWork(
          outcome.doc,
          window.location.href,
          null,
          null,
          new URL(window.location.href),
        );
      } else if (
        ctx &&
        typeof boot.finishRevisionFirstColdStart === "function" &&
        (typeof isRevisionFirstShellPage === "function"
          ? isRevisionFirstShellPage()
          : globalThis.__mei?.thin_shell === true)
      ) {
        await boot.finishRevisionFirstColdStart(ctx, outcome);
      } else if (ctx && typeof boot.dispatchScopeActivation === "function") {
        boot.dispatchScopeActivation({
          scope: ctx.scene_id || ctx.sceneId || "home",
          sceneId: ctx.scene_id || ctx.sceneId || "home",
          appId: ctx.app_id || ctx.appId || "",
          source: "revision-first-fallback",
        });
      }
      if (typeof boot.cacheDiagTrace === "function") {
        boot.cacheDiagTrace("view-cold-start", {
          restored: !!outcome.restored,
          source: outcome.source,
        });
      }
      if (typeof boot.inspectSceneClientCache === "function" && boot.cacheDiagEnabled?.()) {
        void boot.inspectSceneClientCache(ctx);
      }
    } catch (error) {
      console.warn("[spa-navigation] view cold start skipped", error);
    }
  })();

  document.addEventListener(
    "click",
    async (event) => {
      if (event.defaultPrevented) return;
      if (event.button !== 0) return;
      if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
      if (shouldBypassSpaClick(event)) {
        if (shouldAbortRuntimeForBypassNavigation(event)) {
          requestRuntimeAbort("full_navigation_bypass");
        }
        return;
      }
      const target = resolveClickTarget(event);
      if (!target) return;
      if (target.download) return;
      if (target.target && target.target !== "_self") return;
      if (!shouldHandleUrl(target.url)) return;
      if (isSameLocation(target.url)) {
        event.preventDefault();
        return;
      }
      event.preventDefault();
      void navigateInternal(target.url, false);
    },
    true,
  );

  window.addEventListener("popstate", () => {
    closeDrilldownOverlay();
    if (shouldHandleUrl(window.location.href)) {
      void navigateInternal(window.location.href, true);
    }
  });

  if (typeof globalThis.__meiBuildCopyContextInit === "function") {
    globalThis.__meiBuildCopyContextInit();
  }
  if (typeof globalThis.__meiBuildExecPanelInit === "function") {
    globalThis.__meiBuildExecPanelInit();
  }
  if (typeof globalThis.__meiBuildProjectionPreviewInit === "function") {
    globalThis.__meiBuildProjectionPreviewInit();
  }
})();
