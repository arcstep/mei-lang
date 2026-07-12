// @ts-nocheck — closes IIFE opened in preamble.js; valid only after bundle concat.
  bootstrapInitialLoadProgress();
  if (typeof boot.installClientCommandWrappers === "function") {
    boot.installClientCommandWrappers();
  }
  tagExistingBodyScripts();
  installSceneProjectionHost();
  if (typeof boot.watchTopbarChromeInjection === "function") {
    boot.watchTopbarChromeInjection();
  }
  void (async () => {
    if (typeof boot.ensureSceneDrilldownContext === "function") {
      try {
        const ctx =
          typeof boot.parseViewContext === "function"
            ? boot.parseViewContext(window.location.href)
            : null;
        await boot.ensureSceneDrilldownContext(ctx || {});
      } catch (error) {
        console.warn("[spa-navigation] drilldown context load skipped", error);
      }
    }
    applyDrilldownContextFromQuery();
    applySceneProjectionContextFromStorage();
    if (typeof boot.hostCapabilitiesReady === "function") {
      try {
        await boot.hostCapabilitiesReady({ timeoutMs: 5000 });
      } catch (error) {
        console.warn("[spa-navigation] host capabilities wait skipped", error);
      }
    }
    if (typeof boot.hydrateManifestLayerHoldings === "function") {
      boot.hydrateManifestLayerHoldings();
    }
    if (typeof boot.showThinShellFallback === "function") {
      boot.showThinShellFallback("正在加载场景内容…");
    }
    const isThinShell =
      typeof isRevisionFirstShellPage === "function"
        ? isRevisionFirstShellPage()
        : globalThis.__mei?.thin_shell === true;
    try {
      if (typeof boot.renderPipelineMark === "function") {
        boot.renderPipelineMark("cold_start:begin", { thinShell: isThinShell });
      }
      let outcome = { restored: false, source: "none" };
      if (boot.viewAssembly?.assemble && isThinShell && globalThis.__mei?.view_assembly_v2 !== false) {
        const result = await boot.viewAssembly.assemble(
          { kind: "cold_start" },
          { debounce: false },
        );
        outcome = {
          restored: !!result?.ok,
          doc: result?.ok ? document : null,
          source: result?.ok ? "coordinator" : "miss",
          viewRevision: result?.preview || null,
        };
      } else if (typeof boot.tryCacheFirstViewRestore === "function") {
        outcome = await boot.tryCacheFirstViewRestore(window.location.href, {
          replaceHistory: true,
          timeoutMs: 4000,
          coldStart: true,
          skipRemoteWhenValid: true,
        });
      }
      const ctx =
        typeof boot.parseViewContext === "function"
          ? boot.parseViewContext(window.location.href)
          : null;
      if (outcome.restored && outcome.doc && typeof runPostSpaWork === "function") {
        if (typeof boot.hideThinShellFallback === "function") {
          boot.hideThinShellFallback();
        }
        if (typeof boot.scheduleFrameViewportRelayout === "function") {
          boot.scheduleFrameViewportRelayout();
        }
        if (typeof boot.rememberViewRevision === "function" && ctx && outcome.revision) {
          boot.rememberViewRevision(ctx, outcome.revision);
        } else if (
          typeof boot.rememberViewRevision === "function" &&
          ctx &&
          outcome.source !== "coordinator" &&
          globalThis.__mei?.scene_manifest_refs
        ) {
          boot.rememberViewRevision(ctx, globalThis.__mei.scene_manifest_refs);
        }
        runPostSpaWork(
          outcome.doc,
          window.location.href,
          null,
          null,
          new URL(window.location.href),
          { skipViewAssembly: outcome.source === "coordinator" },
        );
      }
      if (
        ctx &&
        typeof boot.finishRevisionFirstColdStart === "function" &&
        isThinShell &&
        outcome.source !== "coordinator"
      ) {
        await boot.finishRevisionFirstColdStart(ctx, outcome);
      } else if (ctx && typeof boot.dispatchScopeActivation === "function" && !outcome.restored) {
        boot.dispatchScopeActivation({
          scope: ctx.scene_id || ctx.sceneId || "home",
          sceneId: ctx.scene_id || ctx.sceneId || "home",
          appId: ctx.app_id || ctx.appId || "",
          source: "revision-first-fallback",
        });
      }
      if (typeof boot.renderPipelineMark === "function") {
        boot.renderPipelineMark("cold_start:end", {
          restored: !!outcome.restored,
          source: outcome.source,
        });
      }
      // Coordinator path finishes materialize in phaseRuntime; let surface_ready /
      // spa_navigation_complete finalize so assembly/surface marks are not truncated.
      if (
        outcome.source !== "coordinator" &&
        typeof boot.renderPipelineFinalize === "function"
      ) {
        boot.renderPipelineFinalize({
          restored: !!outcome.restored,
          source: outcome.source,
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
      if (typeof boot.finishInitialLoadProgress === "function") {
        await boot.finishInitialLoadProgress();
      }
    } catch (error) {
      console.warn("[spa-navigation] view cold start skipped", error);
    }
  })();

  document.addEventListener(
    "click",
    async (event) => {
      if (typeof shouldDeferBuildTreeClick === "function" && shouldDeferBuildTreeClick(event)) {
        return;
      }
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
