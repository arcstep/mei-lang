  function applySceneProjectionDepth(doc) {
    if (!globalThis.MeiProjectionDepth?.applyProjectionDepth) return;
    const root =
      doc.querySelector(".preview-pane-scroll") ||
      doc.querySelector(".shell.scene-shell") ||
      doc.querySelector(".shell") ||
      doc.body;
    if (root instanceof HTMLElement) {
      globalThis.MeiProjectionDepth.applyProjectionDepth(root);
    }
  }

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
        if (navigationId != null && navigationId !== currentNavigationId) return;
        if (!preserveManageWorkspaceFromUrls(currentUrl, nextUrl)) {
          const bundlesReady = await ensureHostBundlesFromDoc(
            doc,
            navigationId,
            currentUrl,
            nextUrl,
          );
          if (!bundlesReady || (navigationId != null && navigationId !== currentNavigationId)) return;
        }
        if (navigationId != null && navigationId !== currentNavigationId) return;
        if (
          typeof boot.bootstrapThinShellComposition === "function" &&
          (globalThis.__mei?.thin_shell === true || doc?.documentElement?.innerHTML?.includes("thin_shell=true"))
        ) {
          await boot.bootstrapThinShellComposition();
        }
        if (navigationId != null && navigationId !== currentNavigationId) return;
        await syncMissingWorkspaceModulesOnly(doc, navigationId);
        if (navigationId != null && navigationId !== currentNavigationId) return;
        if (isBuildWorkspacePathname(nextUrl.pathname)) {
          stabilizeBuildPreviewRuntime();
          if (typeof globalThis.__meiBuildCopyContextInit === "function") {
            globalThis.__meiBuildCopyContextInit();
          }
          if (typeof boot.installManageTabs === "function") {
            boot.installManageTabs();
          }
          if (nextUrl.pathname.startsWith("/apps/manage/")) {
            if (typeof boot.mountSourceTreeControls === "function") {
              boot.mountSourceTreeControls();
            }
          }
          syncManageTabFromUrl(url);
        }
        if (shouldRunBuildPreviewRuntimeForUrl(nextUrl.href)) {
          const skipWake =
            typeof globalThis.MeiBuildNavigation?.shouldSkipPreviewRuntimeWake === "function" &&
            globalThis.MeiBuildNavigation.shouldSkipPreviewRuntimeWake(
              currentUrl?.href || window.location.href,
              nextUrl.href,
            );
          if (!skipWake) {
            publishManagePreviewFromDoc(doc, { resetRuntimeQueryCache: false });
          }
          installSceneProjectionHost();
          if (typeof boot.mountManagePreviewBoard === "function") {
            void boot.mountManagePreviewBoard(doc);
          }
        }
        applyDrilldownContextFromQuery();
        applySceneProjectionContextFromStorage();
        const sceneCtx =
          typeof boot.parseAccessSceneContext === "function"
            ? boot.parseAccessSceneContext(url)
            : null;
        if (sceneCtx && typeof boot.dispatchScopeActivation === "function") {
          boot.dispatchScopeActivation({
            scope: sceneCtx.sceneId,
            sceneId: sceneCtx.sceneId,
            appId: sceneCtx.appId,
            source: "spa-primary-nav",
            projection:
              nextUrl instanceof URL ? String(nextUrl.searchParams.get("mei_projection") || "") : "",
          });
        }
        if (sceneCtx && typeof boot.saveCurrentSceneShellSnapshot === "function") {
          try {
            const revision =
              typeof boot.fetchSceneRevision === "function"
                ? await boot.fetchSceneRevision(sceneCtx, { timeoutMs: SPA_FETCH_TIMEOUT_MS })
                : null;
            if (revision) {
              await boot.saveCurrentSceneShellSnapshot(sceneCtx, revision, doc);
            }
          } catch (error) {
            console.warn("[spa-navigation] post-spa shell snapshot save skipped", error);
          }
        }
        if (typeof boot.markLoadingPostSpaDone === "function") {
          boot.markLoadingPostSpaDone(navigationId);
        }
        applySceneProjectionDepth(doc);
        document.dispatchEvent(new CustomEvent("mei:spa-navigation-complete"));
      } catch (err) {
        console.warn("[spa-navigation] post-spa work failed", err);
      }
    })();
  }

  function preserveManageWorkspaceFromUrls(currentUrl, nextUrl) {
    return shouldPreserveManageWorkspace(currentUrl, nextUrl);
  }

