// @ts-nocheck — closes IIFE opened in preamble.js; valid only after bundle concat.
  bootstrapInitialLoadProgress();
  tagExistingBodyScripts();
  installSceneProjectionHost();
  applyDrilldownContextFromQuery();
  applySceneProjectionContextFromStorage();

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
      void navigateInternal(target.url, false);
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
          void navigateInternal(window.location.href, true);
        });
        return;
      }
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
