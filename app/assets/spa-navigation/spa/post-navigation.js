  function applySceneProjectionDepth(doc) {
    if (typeof isAppSurfaceRoute === "function" && isAppSurfaceRoute(window.location.pathname)) {
      return;
    }
    if (
      (typeof isWorkspaceSurfaceUrl === "function" &&
        isWorkspaceSurfaceUrl(window.location.href)) ||
      (typeof isWorkspaceSurfaceRoute === "function" &&
        isWorkspaceSurfaceRoute(window.location.pathname))
    ) {
      return;
    }
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

  function ssrPreviewMaterialized(doc) {
    const root =
      doc?.querySelector?.("#mei-compose-root, .preview-pane-scroll, .shell") ||
      document.querySelector("#mei-compose-root, .preview-pane-scroll, .shell");
    if (boot.previewMaterializer?.isSsrInjectedPreviewRoot) {
      return boot.previewMaterializer.isSsrInjectedPreviewRoot(root);
    }
    return (
      typeof boot.hasMaterializedPreview === "function" &&
      boot.hasMaterializedPreview(root) &&
      !boot.previewMaterializer?.isClientLayerMaterialized?.(root)
    );
  }

  function runPostSpaWork(doc, url, navigationId, currentUrl, nextUrl, workOpts) {
    void (async () => {
      try {
        const postOpts = workOpts || {};
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
        const sceneCtx =
          typeof boot.parseViewContext === "function"
            ? boot.parseViewContext(url)
            : typeof boot.parseAccessSceneContext === "function"
              ? boot.parseAccessSceneContext(url)
              : null;
        const workspaceSurface =
          (typeof isWorkspaceSurfaceUrl === "function" && isWorkspaceSurfaceUrl(url)) ||
          (typeof isWorkspaceSurfaceRoute === "function" &&
            isWorkspaceSurfaceRoute(nextUrl.pathname));
        if (
          !postOpts.skipViewAssembly &&
          workspaceSurface &&
          sceneCtx &&
          boot.viewAssembly?.assemble &&
          globalThis.__mei?.view_assembly_v2 !== false
        ) {
          await boot.viewAssembly.assemble(
            { kind: "spa_nav", ...sceneCtx, url },
            { debounce: false },
          );
        } else if (
          typeof boot.bootstrapThinShellComposition === "function" &&
          !ssrPreviewMaterialized(doc) &&
          (typeof isRevisionFirstShellPage === "function"
            ? isRevisionFirstShellPage(nextUrl.pathname)
            : globalThis.__mei?.thin_shell === true ||
              doc?.documentElement?.innerHTML?.includes("thin_shell=true"))
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
        const runManagePreview =
          (typeof isWorkspaceSurfaceUrl === "function" && isWorkspaceSurfaceUrl(url)) ||
          (typeof isWorkspaceSurfaceUrl === "function" && isWorkspaceSurfaceUrl(nextUrl.href)) ||
          shouldRunBuildPreviewRuntimeForUrl(nextUrl.href);
        if (runManagePreview) {
          const skipWake = ssrPreviewMaterialized(doc);
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
        if (sceneCtx && typeof boot.syncTopbarActiveState === "function") {
          boot.syncTopbarActiveState(sceneCtx.surface || sceneCtx.mode || "app");
        }
        if (sceneCtx && typeof boot.syncAppTabActiveState === "function") {
          boot.syncAppTabActiveState(sceneCtx.appId || sceneCtx.app_id);
        }
        if (typeof boot.fixTopbarHrefsFromPageContext === "function") {
          boot.fixTopbarHrefsFromPageContext();
        }
        if (sceneCtx && typeof boot.rememberViewRevision === "function") {
          try {
            const vrCtx =
              typeof boot.parseViewContext === "function"
                ? boot.parseViewContext(window.location.href)
                : sceneCtx;
            const stored = boot.readViewRevision?.(vrCtx);
            if (stored) {
              boot.rememberViewRevision(vrCtx, stored);
            }
          } catch (error) {
            console.warn("[spa-navigation] post-spa revision remember skipped", error);
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

