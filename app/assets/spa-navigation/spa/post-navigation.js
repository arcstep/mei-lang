  function stabilizeBuildPreviewRuntime() {
    if (!isBuildWorkspacePathname(window.location.pathname)) return;
    document.body.classList.remove("access-drilldown-open", "access-scene-board-open");
    if (typeof closeDrilldownOverlay === "function") {
      try {
        closeDrilldownOverlay();
      } catch (_) {}
    }
    if (typeof boot.clearManageWorkspaceLoadingState === "function") {
      boot.clearManageWorkspaceLoadingState();
    }
    if (typeof globalThis.MeiBuildInspectHighlight?.refresh === "function") {
      globalThis.MeiBuildInspectHighlight.refresh();
    }
    if (typeof globalThis.MeiBuildTreePersist?.refresh === "function") {
      globalThis.MeiBuildTreePersist.refresh();
    }
  }

  function runPostSpaWork(doc, url, navigationId, currentUrl, nextUrl) {
    void (async () => {
      try {
        if (navigationId !== currentNavigationId) return;
        if (!preserveManageWorkspaceFromUrls(currentUrl, nextUrl)) {
          const bundlesReady = await ensureHostBundlesFromDoc(
            doc,
            navigationId,
            currentUrl,
            nextUrl,
          );
          if (!bundlesReady || navigationId !== currentNavigationId) return;
        }
        if (navigationId !== currentNavigationId) return;
        await syncMissingWorkspaceModulesOnly(doc, navigationId);
        if (navigationId !== currentNavigationId) return;
        if (isBuildWorkspacePathname(nextUrl.pathname)) {
          stabilizeBuildPreviewRuntime();
          if (typeof boot.installManageTabs === "function") {
            boot.installManageTabs();
          }
          if (nextUrl.pathname.startsWith("/apps/manage/")) {
            if (typeof boot.mountSourceTreeControls === "function") {
              boot.mountSourceTreeControls();
            }
          }
          syncManageTabFromUrl(url);
          if (typeof globalThis.MeiBuildTreePersist?.refresh === "function") {
            globalThis.MeiBuildTreePersist.refresh();
          }
        }
        if (shouldRunBuildPreviewRuntimeForUrl(nextUrl.href)) {
          publishManagePreviewFromDoc(doc, { resetRuntimeQueryCache: false });
          installSceneProjectionHost();
          if (typeof boot.mountManagePreviewBoard === "function") {
            void boot.mountManagePreviewBoard(doc);
          }
        }
        applyDrilldownContextFromQuery();
        applySceneProjectionContextFromStorage();
        if (typeof boot.markLoadingPostSpaDone === "function") {
          boot.markLoadingPostSpaDone(navigationId);
        }
      } catch (err) {
        console.warn("[spa-navigation] post-spa work failed", err);
      }
    })();
  }

  function preserveManageWorkspaceFromUrls(currentUrl, nextUrl) {
    return shouldPreserveManageWorkspace(currentUrl, nextUrl);
  }

