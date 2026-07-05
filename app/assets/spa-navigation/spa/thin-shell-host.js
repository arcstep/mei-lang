/**
 * Thin-shell host frame helpers + diagnostic entrypoint.
 */
(function initThinShellHost(global) {
  "use strict";

  const boot = (global.__meiLangBoot = global.__meiLangBoot || {});

  function fallbackEl() {
    return global.document?.getElementById?.("mei-thin-shell-fallback");
  }

  function showThinShellFallback(message) {
    const el = fallbackEl();
    if (!(el instanceof HTMLElement)) return;
    if (message) el.textContent = String(message);
    el.hidden = false;
    el.classList.remove("hidden");
  }

  function hideThinShellFallback() {
    const el = fallbackEl();
    if (!(el instanceof HTMLElement)) return;
    el.hidden = true;
    el.classList.add("hidden");
  }

  function applyHostChromeFromManifestRefs() {
    const layers = globalThis.__mei?.scene_manifest_refs?.layers;
    if (!layers || typeof layers !== "object") return false;
    const shell =
      layers["shell.app"] ||
      layers["shell.layout"] ||
      layers["shell.prototype"] ||
      layers["shell.build"] ||
      null;
    if (!shell) return false;
    const root =
      typeof boot.resolveComposeRoot === "function"
        ? boot.resolveComposeRoot("app")
        : global.document?.getElementById?.("mei-compose-root");
    if (boot.viewCompositor?.applyShellLayer && root instanceof HTMLElement) {
      boot.viewCompositor.applyShellLayer(root, shell);
    } else {
      const doc = shell.document || shell;
      const top = String(doc?.topbar_html || "").trim();
      const bottom = String(doc?.statusbar_html || "").trim();
      const topSlot = global.document?.getElementById?.("mei-host-topbar-slot");
      const bottomSlot = global.document?.getElementById?.("mei-host-statusbar-slot");
      if (top && topSlot instanceof HTMLElement) topSlot.innerHTML = top;
      if (bottom && bottomSlot instanceof HTMLElement) bottomSlot.innerHTML = bottom;
    }
    if (typeof boot.refreshStatusBarChips === "function") {
      boot.refreshStatusBarChips();
    }
    return hostChromeSummary().topbar || hostChromeSummary().statusbar;
  }

  function scheduleEarlyHostChrome() {
    const run = () => {
      applyHostChromeFromManifestRefs();
    };
    if (global.document?.readyState === "loading") {
      global.document.addEventListener("DOMContentLoaded", run, { once: true });
    } else {
      run();
    }
  }

  scheduleEarlyHostChrome();

  function hostChromeSummary() {
    return {
      topbar: !!global.document?.querySelector?.(
        "#mei-host-topbar-slot .topbar-shell, #mei-host-topbar-slot .topbar, .topbar-shell",
      ),
      statusbar: !!global.document?.querySelector?.(
        "#mei-host-statusbar-slot .statusbar-shell, #mei-host-statusbar-slot .statusbar, .statusbar-shell",
      ),
      composeRoot: !!global.document?.getElementById?.("mei-compose-root"),
      previewScopes: global.document?.querySelectorAll?.("[data-preview-scope]")?.length || 0,
    };
  }

  function hasMaterializedPreview(root) {
    if (!(root instanceof HTMLElement)) return false;
    return !!root.querySelector(
      "[data-mei-frame-viewport], [data-mei-use-key], .preview-surface, .preview-viewport",
    );
  }

  function hydrateManifestLayerHoldings() {
    const manifest = globalThis.__mei?.scene_manifest_refs;
    if (manifest && boot.layerStore?.syncHoldingsFromManifest) {
      boot.layerStore.syncHoldingsFromManifest(manifest);
    }
  }

  hydrateManifestLayerHoldings();

  async function runThinShellDiagnostic() {
    const url = global.location.href;
    const ctx =
      typeof boot.parseViewContext === "function" ? boot.parseViewContext(url) : null;
    const report = {
      url,
      thin_shell: globalThis.__mei?.thin_shell === true,
      view_revision_enabled: globalThis.__mei?.view_revision_enabled !== false,
      manifest_layers: Object.keys(globalThis.__mei?.scene_manifest_refs?.layers || {}),
      host_chrome: hostChromeSummary(),
      boot_apis: {
        tryCacheFirstViewRestore: typeof boot.tryCacheFirstViewRestore === "function",
        negotiateAndAssemble: typeof boot.negotiateAndAssemble === "function",
        viewRevisionClient: !!boot.viewRevisionClient?.negotiateWithLocalMiss,
        layerStore: !!boot.layerStore?.putLayerByRef,
        viewCompositor: !!boot.viewCompositor?.composeFromLayers,
      },
      bootstrap_ready: globalThis.__meiBootstrapPayloadReady === 1,
      client_revision:
        globalThis.__mei?.scene_manifest_refs?.semantic_core?.client_revision || "",
      ctx,
    };

    if (ctx && boot.viewRevisionClient?.fetchViewRevision) {
      try {
        const vr = await boot.viewRevisionClient.fetchViewRevision({
          app_id: ctx.app_id || ctx.appId,
          scene_id: ctx.scene_id || ctx.sceneId,
          surface: ctx.surface || ctx.mode || "app",
        });
        report.view_revision = {
          status: vr.status || vr._headers?.status,
          changed_layers: vr.changed_layers || [],
          inline_layer_keys: Object.keys(vr.inline_layers || {}),
          shell_topbar_bytes: String(
            vr.inline_layers?.["shell.app"]?.document?.topbar_html ||
              vr.manifest?.layers?.["shell.app"]?.document?.topbar_html ||
              "",
          ).length,
          manifest_topbar_bytes: String(
            globalThis.__mei?.scene_manifest_refs?.layers?.["shell.app"]?.document
              ?.topbar_html || "",
          ).length,
          slot_topbar_html_bytes: String(
            global.document?.getElementById?.("mei-host-topbar-slot")?.innerHTML || "",
          ).length,
        };
      } catch (error) {
        report.view_revision_error = String(error?.message || error);
      }
    }

    if (ctx && boot.negotiateAndAssemble) {
      try {
        const assembled = await boot.negotiateAndAssemble(ctx, { silent: true });
        report.assemble = {
          ok: !!assembled?.assemble?.ok,
          outcome: assembled?.outcome,
          missing: assembled?.assemble?.missing || [],
        };
        report.host_chrome_after = hostChromeSummary();
        report.preview_scopes_after =
          global.document?.querySelectorAll?.("[data-preview-scope]")?.length || 0;
      } catch (error) {
        report.assemble_error = String(error?.message || error);
      }
    }

    console.table(report.host_chrome);
    console.log("[mei thin-shell diagnostic]", report);
    return report;
  }

  boot.showThinShellFallback = showThinShellFallback;
  boot.hideThinShellFallback = hideThinShellFallback;
  boot.applyHostChromeFromManifestRefs = applyHostChromeFromManifestRefs;
  boot.hasMaterializedPreview = hasMaterializedPreview;
  boot.hydrateManifestLayerHoldings = hydrateManifestLayerHoldings;
  boot.runThinShellDiagnostic = runThinShellDiagnostic;
  global.runMeiThinShellDiagnostic = runThinShellDiagnostic;
})(typeof window !== "undefined" ? window : globalThis);
