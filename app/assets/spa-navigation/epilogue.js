// @ts-nocheck — closes IIFE opened in preamble.js; valid only after bundle concat.
  bootstrapInitialLoadProgress();
  tagExistingBodyScripts();
  installSceneProjectionHost();
  applyDrilldownContextFromQuery();
  applySceneProjectionContextFromStorage();
  void (async () => {
    const ctx =
      typeof boot.parseAccessSceneContext === "function"
        ? boot.parseAccessSceneContext(window.location.href)
        : null;
    if (!ctx) return;
    try {
      if (typeof boot.fetchSceneRevision !== "function") return;
      const revision = await boot.fetchSceneRevision(ctx, { timeoutMs: 4000 });
      if (typeof boot.ensureSceneBootstrapPayload === "function") {
        await boot.ensureSceneBootstrapPayload(ctx, revision);
      }
      const sceneCtx = ctx;
      if (sceneCtx?.appId && typeof window.__meiDatasetRuntime === "undefined") {
        /* runtime-query module may load later via component bundle */
      }
      if (typeof boot.saveCurrentSceneShellSnapshot === "function") {
        await boot.saveCurrentSceneShellSnapshot(ctx, revision, document);
      }
    } catch (error) {
      console.warn("[spa-navigation] initial scene cache bootstrap skipped", error);
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
      try {
        if (
          typeof globalThis.MeiBuildNavigation?.tryHandleBuildClick === "function" &&
          (await globalThis.MeiBuildNavigation.tryHandleBuildClick(event, target.url, false))
        ) {
          return;
        }
      } catch (err) {
        console.warn("[spa-navigation] build fast-nav failed; fallback to SPA", err);
      }
      void navigateInternal(target.url, false, { skipBuildNav: true });
    },
    true,
  );

  window.addEventListener("popstate", () => {
    closeDrilldownOverlay();
    if (shouldHandleUrl(window.location.href)) {
      const fromUrl =
        typeof globalThis.MeiBuildNavigation?.getLastUrl === "function"
          ? globalThis.MeiBuildNavigation.getLastUrl()
          : window.location.href;
      if (typeof globalThis.MeiBuildNavigation?.tryNavigateBuild === "function") {
        void globalThis.MeiBuildNavigation.tryNavigateBuild(
          fromUrl,
          window.location.href,
          { replaceHistory: true },
        ).then((result) => {
          if (result?.handled) {
            globalThis.MeiBuildNavigation.noteUrl(window.location.href);
            return;
          }
          void navigateInternal(window.location.href, true, { skipBuildNav: true });
        });
        return;
      }
      void navigateInternal(window.location.href, true, { skipBuildNav: true });
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
