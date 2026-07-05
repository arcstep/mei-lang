  async function bootstrapThinShellComposition() {
    if (typeof isRevisionFirstShellPage === "function" && !isRevisionFirstShellPage()) {
      return false;
    }
    const ctx =
      typeof boot.parseViewContext === "function"
        ? boot.parseViewContext(global.location.href)
        : typeof boot.parseAccessSceneContext === "function"
          ? boot.parseAccessSceneContext(global.location.href)
          : null;
    if (!ctx?.appId && !ctx?.app_id) return false;
    const appId = ctx.appId || ctx.app_id;
    const sceneId = ctx.sceneId || ctx.scene_id || "home";
    const surface = ctx.surface || ctx.mode || "app";
    const composeRoot =
      typeof boot.resolveComposeRoot === "function"
        ? boot.resolveComposeRoot(surface)
        : global.document?.querySelector?.(".shell");
    if (!(composeRoot instanceof HTMLElement)) return false;

    if (boot.viewRevisionClient?.negotiateWithLocalMiss) {
      try {
        const vrCtx = {
          app_id: appId,
          scene_id: sceneId,
          surface,
          node: ctx.node || "",
          data_mode: ctx.data_mode || ctx.dataMode || "",
          review_projection: ctx.review_projection || ctx.reviewProjection || "",
          chrome: ctx.chrome || "",
          tab: ctx.tab || "",
          focus: ctx.focus || "",
          scope: ctx.scope || "",
        };
        const result = await boot.viewRevisionClient.negotiateWithLocalMiss(vrCtx);
        if (result?.assemble?.ok) {
          return true;
        }
        if (result?.assemble?.missing?.length && typeof boot.showThinShellFallback === "function") {
          boot.showThinShellFallback(
            `场景层未就绪，缺失: ${result.assemble.missing.join(", ")}`,
          );
        }
      } catch (error) {
        console.warn("[spa-navigation] view-revision thin shell composition skipped", error);
      }
    }

    if (!boot.sceneManifestLoader?.ensureAccessComposeLayers || !boot.viewCompositor?.composeFromLayers) {
      return false;
    }
    try {
      const { layers, manifest } = await boot.sceneManifestLoader.ensureAccessComposeLayers(
        appId,
        sceneId,
        surface,
      );
      if (!layers?.["structure.full"]) return false;
      const projection =
        ctx.reviewProjection ||
        ctx.review_projection ||
        String(composeRoot.getAttribute("data-review-projection") || "").trim() ||
        manifest?.compose_defaults?.review_projection ||
        "live_full";
      const composed = boot.viewCompositor.composeFromLayers(composeRoot, layers, {
        review_projection: projection,
        route_mode: surface,
      });
      if (!composed && typeof boot.showThinShellFallback === "function") {
        boot.showThinShellFallback("场景结构层组装失败，请检查 view-revision / layer-batch。");
      }
      return composed;
    } catch (error) {
      console.warn("[spa-navigation] thin shell composition skipped", error);
      if (typeof boot.showThinShellFallback === "function") {
        boot.showThinShellFallback(
          `场景内容加载失败: ${String(error?.message || error)}`,
        );
      }
      return false;
    }
  }

  boot.bootstrapThinShellComposition = bootstrapThinShellComposition;

  function bootstrapColdAccessSceneRuntime() {
    try {
      if (
        typeof isBuildWorkspacePathname === "function" &&
        isBuildWorkspacePathname(window.location.pathname) &&
        typeof boot.finishRevisionFirstColdStart === "function"
      ) {
        const ctx =
          typeof boot.parseViewContext === "function"
            ? boot.parseViewContext(window.location.href)
            : null;
        if (ctx) {
          void boot.finishRevisionFirstColdStart(ctx, { restored: false });
          return;
        }
      }
      const sceneCtx =
        typeof boot.parseAccessSceneContext === "function"
          ? boot.parseAccessSceneContext(window.location.href)
          : null;
      if (!sceneCtx?.sceneId) return;

      const run = () => {
        void bootstrapThinShellComposition().then(async () => {
          if (typeof boot.wakeRevisionFirstShellRuntime === "function") {
            await boot.wakeRevisionFirstShellRuntime(sceneCtx);
          } else {
            if (typeof boot.dispatchScopeActivation === "function") {
              boot.dispatchScopeActivation({
                scope: sceneCtx.sceneId,
                sceneId: sceneCtx.sceneId,
                appId: sceneCtx.appId,
                source: "initial-load",
              });
            }
            if (typeof wakeRuntimeAfterSceneBundleLoaded === "function") {
              wakeRuntimeAfterSceneBundleLoaded();
            }
          }
          try {
            document.dispatchEvent(new CustomEvent("mei:spa-navigation-complete"));
          } catch (_) {}
        });
      };

      if (document.readyState === "loading") {
        document.addEventListener("DOMContentLoaded", run, { once: true });
      } else {
        run();
      }
    } catch (error) {
      console.warn("[spa-navigation] initial access bootstrap skipped", error);
    }
  }

  boot.bootstrapColdAccessSceneRuntime = bootstrapColdAccessSceneRuntime;
