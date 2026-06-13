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
        publishManagePreviewFromDoc(doc, { resetRuntimeQueryCache: false });
        if (nextUrl.pathname.startsWith("/apps/manage/")) {
          if (typeof boot.installManageTabs === "function") {
            boot.installManageTabs();
          }
          if (typeof boot.mountSourceTreeControls === "function") {
            boot.mountSourceTreeControls();
          }
          syncManageTabFromUrl(url);
          pulseManagePreview(extractManagePanelContext(document.querySelector("#meilang-author-panel")), {
            resetRuntimeQueryCache: false,
          });
        }
        installSceneProjectionHost();
        applyDrilldownContextFromQuery();
        applySceneProjectionContextFromStorage();
      } catch (err) {
        console.warn("[spa-navigation] post-spa work failed", err);
      }
    })();
  }

  function preserveManageWorkspaceFromUrls(currentUrl, nextUrl) {
    return shouldPreserveManageWorkspace(currentUrl, nextUrl);
  }

