// @ts-nocheck — closes IIFE opened in preamble.js; valid only after bundle concat.
  bootstrapInitialLoadProgress();
  tagExistingBodyScripts();
  installSceneProjectionHost();
  applyDrilldownContextFromQuery();
  applySceneProjectionContextFromStorage();

  document.addEventListener(
    "click",
    (event) => {
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
})();
