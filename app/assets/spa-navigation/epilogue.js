// @ts-nocheck — closes IIFE opened in preamble.js; valid only after bundle concat.
  bootstrapInitialLoadProgress();
  if (typeof boot.installClientCommandWrappers === "function") {
    boot.installClientCommandWrappers();
  }
  tagExistingBodyScripts();
  installSceneProjectionHost();
  applyDrilldownContextFromQuery();
  applySceneProjectionContextFromStorage();

  function scheduleAccessSceneSnapshotPersist(ctx, revision) {
    if (!ctx || !revision || typeof boot.saveCurrentSceneShellSnapshot !== "function") return;
    const run = () => {
      void boot.saveCurrentSceneShellSnapshot(ctx, revision, document);
    };
    window.addEventListener("pagehide", run, { once: true });
    window.addEventListener("visibilitychange", () => {
      if (document.visibilityState === "hidden") run();
    });
  }

  void (async () => {
    if (
      typeof isBuildWorkspacePathname === "function" &&
      isBuildWorkspacePathname(window.location.pathname) &&
      typeof globalThis.MeiBuildNavigation?.tryRestoreBuildPreviewFromCache === "function"
    ) {
      try {
        const prefetchedBuild = window.__mei?.prefetched_build_fragment;
        if (
          prefetchedBuild?.preview_html &&
          typeof globalThis.MeiBuildNavigation?.swapPreviewFragment === "function"
        ) {
          globalThis.MeiBuildNavigation.swapPreviewFragment(
            String(prefetchedBuild.preview_html || ""),
            String(prefetchedBuild.drilldown_script || ""),
          );
          if (
            Array.isArray(prefetchedBuild.workspace_scripts) &&
            typeof boot.syncPreviewWorkspaceScripts === "function"
          ) {
            await boot.syncPreviewWorkspaceScripts(prefetchedBuild.workspace_scripts);
          }
        }
        const buildOutcome = await globalThis.MeiBuildNavigation.tryRestoreBuildPreviewFromCache(
          window.location.href,
          { timeoutMs: 4000, coldStart: true, skipRemoteWhenValid: true },
        );
        if (typeof boot.cacheDiagTrace === "function") {
          boot.cacheDiagTrace("build-cold-start", buildOutcome || {});
        }
        if (
          !buildOutcome?.restored &&
          buildOutcome?.revision &&
          typeof globalThis.MeiBuildNavigation?.scheduleEagerBuildPreviewPersist === "function"
        ) {
          globalThis.MeiBuildNavigation.scheduleEagerBuildPreviewPersist(
            window.location.href,
            buildOutcome.revision,
          );
        } else if (
          !buildOutcome?.restored &&
          typeof globalThis.MeiBuildNavigation?.scheduleEagerBuildPreviewPersist === "function"
        ) {
          globalThis.MeiBuildNavigation.scheduleEagerBuildPreviewPersist(window.location.href);
        }
      } catch (error) {
        console.warn("[spa-navigation] initial build preview cache restore skipped", error);
      }
      if (typeof boot.inspectSceneClientCache === "function" && boot.cacheDiagEnabled?.()) {
        void boot.inspectSceneClientCache();
      }
      return;
    }
    const ctx =
      typeof boot.parseAccessSceneContext === "function"
        ? boot.parseAccessSceneContext(window.location.href)
        : null;
    if (!ctx || typeof boot.tryCacheFirstSceneAccess !== "function") return;
    try {
      const outcome = await boot.tryCacheFirstSceneAccess(ctx, {
        url: window.location.href,
        replaceHistory: true,
        timeoutMs: 4000,
        allowFragment: true,
        coldStart: true,
        skipRemoteWhenValid: true,
      });
      if (outcome.restored && outcome.doc && typeof runPostSpaWork === "function") {
        runPostSpaWork(outcome.doc, window.location.href, null, null, new URL(window.location.href));
      } else if (
        !outcome.restored &&
        outcome.source === "local_miss" &&
        typeof boot.bootstrapColdAccessSceneRuntime === "function"
      ) {
        boot.bootstrapColdAccessSceneRuntime();
      } else if (
        !outcome.restored &&
        outcome.source !== "local_miss" &&
        typeof boot.bootstrapColdAccessSceneRuntime === "function"
      ) {
        boot.bootstrapColdAccessSceneRuntime();
      }
      if (outcome.revision) {
        scheduleAccessSceneSnapshotPersist(ctx, outcome.revision);
        if (!outcome.restored) {
          await boot.saveCurrentSceneShellSnapshot(ctx, outcome.revision, document);
        }
      }
      if (typeof boot.cacheDiagTrace === "function") {
        boot.cacheDiagTrace("access-cold-start", {
          restored: !!outcome.restored,
          source: outcome.source,
          revision_digest: outcome.revision?.revision_digest,
        });
      }
      if (typeof boot.inspectSceneClientCache === "function" && boot.cacheDiagEnabled?.()) {
        void boot.inspectSceneClientCache(ctx);
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
