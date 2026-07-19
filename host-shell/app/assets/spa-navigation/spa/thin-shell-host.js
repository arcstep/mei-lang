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

  function shellDocFromManifestRefs(surface) {
    const layers = globalThis.__mei?.scene_manifest_refs?.layers;
    if (!layers || typeof layers !== "object") return null;
    const slug = String(surface || "app").trim().toLowerCase();
    const shell =
      layers[`shell.${slug}`] ||
      layers["shell.app"] ||
      layers["shell.layout"] ||
      layers["shell.prototype"] ||
      null;
    if (!shell) return null;
    return shell.document || shell;
  }

  /** chrome=none / body.chrome-none：只抑制顶栏；底栏仍由 shell layer 提供。 */
  function isHostChromeSuppressed(ctx) {
    const fromCtx = String(ctx?.chrome || "").trim().toLowerCase();
    if (fromCtx === "none") return true;
    try {
      const urlChrome = String(
        new URL(global.location.href).searchParams.get("chrome") || "",
      )
        .trim()
        .toLowerCase();
      if (urlChrome === "none") return true;
    } catch (_) {}
    const body = global.document?.body;
    return body instanceof HTMLElement && body.classList.contains("chrome-none");
  }

  function isSsrShellPlaceholder(ctx) {
    const surface = ctx?.surface || ctx?.mode || "app";
    const doc = shellDocFromManifestRefs(surface);
    if (isHostChromeSuppressed(ctx)) {
      return !String(doc?.statusbar_html || "").trim();
    }
    if (boot.viewCompositor?.isPlaceholderShellDoc) {
      return boot.viewCompositor.isPlaceholderShellDoc(doc);
    }
    const top = String(doc?.topbar_html || "").trim();
    if (!top) return true;
    return top.includes('class="mei-shell-topbar"') && top.length < 240;
  }

  function hostChromeReady(ctx) {
    const summary = hostChromeSummary();
    if (isHostChromeSuppressed(ctx)) return summary.statusbar;
    return summary.topbar || summary.statusbar;
  }

  function applyHostChromeFromManifestRefs() {
    const layers = globalThis.__mei?.scene_manifest_refs?.layers;
    if (!layers || typeof layers !== "object") return false;
    const ctx =
      typeof boot.parseViewContext === "function"
        ? boot.parseViewContext(global.location.href)
        : null;
    const surface = String(ctx?.surface || ctx?.mode || "app")
      .trim()
      .toLowerCase();
    const shell =
      layers[`shell.${surface}`] ||
      layers["shell.app"] ||
      layers["shell.layout"] ||
      layers["shell.prototype"] ||
      layers["shell.build"] ||
      null;
    if (!shell) return false;
    const shellDoc = shell.document || shell;
    const chromeCtx = ctx || { surface };
    if (isSsrShellPlaceholder(chromeCtx)) {
      if (typeof boot.cacheDiagTrace === "function") {
        boot.cacheDiagTrace("host-chrome-placeholder", {
          surface,
          topbar_len: String(shellDoc?.topbar_html || "").length,
        });
      }
    }
    if (isHostChromeSuppressed(chromeCtx)) {
      const bottom = String(shellDoc?.statusbar_html || "").trim();
      const topSlot = global.document?.getElementById?.("mei-host-topbar-slot");
      const bottomSlot = global.document?.getElementById?.("mei-host-statusbar-slot");
      if (topSlot instanceof HTMLElement) topSlot.innerHTML = "";
      if (bottom && bottomSlot instanceof HTMLElement) bottomSlot.innerHTML = bottom;
      if (typeof boot.refreshStatusBarChips === "function") {
        boot.refreshStatusBarChips();
      }
      if (typeof boot.refreshVisitHistoryPanel === "function") {
        boot.refreshVisitHistoryPanel();
      }
      try {
        global.document?.dispatchEvent?.(
          new CustomEvent("mei:shell-layer-applied", {
            detail: { source: "thin-shell-host", topbarSuppressed: true },
          }),
        );
      } catch (_error) {
        // ignore
      }
      return true;
    }
    const root =
      typeof boot.resolveComposeRoot === "function"
        ? boot.resolveComposeRoot(surface)
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
      try {
        global.document?.dispatchEvent?.(
          new CustomEvent("mei:shell-layer-applied", {
            detail: { source: "thin-shell-host" },
          }),
        );
      } catch (_error) {
        // ignore
      }
    }
    if (typeof boot.refreshStatusBarChips === "function") {
      boot.refreshStatusBarChips();
    }
    return hostChromeReady(chromeCtx);
  }

  function ensureViewShellLayout() {
    const body = global.document?.body;
    if (body instanceof HTMLElement) {
      body.classList.add("mei-view-shell-body", "min-h-screen", "flex", "flex-col", "overflow-hidden");
    }
    const viewHost = global.document?.getElementById?.("mei-view-host");
    if (viewHost instanceof HTMLElement) {
      viewHost.classList.add("relative", "flex-1", "min-h-0", "overflow-hidden");
      const fallback = global.document?.getElementById?.("mei-thin-shell-fallback");
      if (
        fallback instanceof HTMLElement &&
        fallback.parentElement !== viewHost &&
        viewHost.isConnected
      ) {
        viewHost.appendChild(fallback);
        fallback.classList.add("mei-view-loading-overlay");
      }
    }
    const topSlot = global.document?.getElementById?.("mei-host-topbar-slot");
    const bottomSlot = global.document?.getElementById?.("mei-host-statusbar-slot");
    if (topSlot instanceof HTMLElement) topSlot.classList.add("mei-host-chrome-slot", "shrink-0");
    if (bottomSlot instanceof HTMLElement) {
      bottomSlot.classList.add("mei-host-chrome-slot", "shrink-0", "mt-auto");
    }
  }

  function scheduleEarlyHostChrome() {
    const run = () => {
      ensureViewShellLayout();
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
        "#mei-host-topbar-slot .topbar-shell, #mei-host-topbar-slot sl-button[data-mei-app-view], #mei-host-topbar-slot .topbar, .topbar-shell",
      ),
      statusbar: !!global.document?.querySelector?.(
        "#mei-host-statusbar-slot .statusbar-shell, #mei-host-statusbar-slot .statusbar, .statusbar-shell",
      ),
      composeRoot: !!global.document?.getElementById?.("mei-compose-root"),
      previewScopes: global.document?.querySelectorAll?.("[data-preview-scope]")?.length || 0,
    };
  }

  function previewRenderSummary() {
    const ctx =
      typeof boot.parseViewContext === "function"
        ? boot.parseViewContext(global.location.href)
        : null;
    const surface = ctx?.surface || ctx?.mode || "app";
    const el =
      (typeof boot.resolveComposeRoot === "function"
        ? boot.resolveComposeRoot(surface)
        : null) ||
      global.document?.getElementById?.("mei-compose-root") ||
      global.document?.querySelector?.(".preview-pane-scroll");
    if (!(el instanceof HTMLElement)) {
      return {
        composeRootBytes: 0,
        componentHosts: 0,
        emptyHosts: 0,
        customElements: 0,
        dataProps: 0,
        ssrInjected: false,
        clientMaterialized: false,
      };
    }
    const hosts = Array.from(el.querySelectorAll(".component-host"));
    const emptyHosts = hosts.filter((host) => !host.firstElementChild).length;
    const customElements = hosts.filter((host) => host.firstElementChild).length;
    return {
      composeRootBytes: String(el.innerHTML || "").length,
      componentHosts: hosts.length,
      emptyHosts,
      customElements,
      dataProps: el.querySelectorAll("[data-props]").length,
      ssrInjected: !!boot.previewMaterializer?.isSsrInjectedPreviewRoot?.(el),
      clientMaterialized: !!boot.previewMaterializer?.isClientLayerMaterialized?.(el),
      sampleTags: hosts
        .map((host) => host.firstElementChild?.tagName?.toLowerCase() || "")
        .filter(Boolean)
        .slice(0, 6),
    };
  }

  function hasMaterializedPreview(root) {
    if (boot.previewMaterializer?.hasMaterializedPreview) {
      return boot.previewMaterializer.hasMaterializedPreview(root);
    }
    if (!(root instanceof HTMLElement)) return false;
    return !!root.querySelector(
      "[data-mei-frame-viewport], [data-mei-use-key], .preview-surface, .preview-viewport, .mei-structure-tree",
    );
  }

  function hydrateManifestLayerHoldings() {
    const manifest = globalThis.__mei?.scene_manifest_refs;
    if (!manifest || !boot.layerStore) return;
    const appId = String(manifest.app_id || manifest.appId || "").trim();
    const sceneId = String(manifest.scene_id || manifest.sceneId || "").trim();
    boot.layerStore.syncHoldingsFromManifest(manifest);
    if (!appId || !sceneId || !manifest.layers) return;
    for (const [name, value] of Object.entries(manifest.layers)) {
      if (!value || typeof value !== "object") continue;
      const document = value.document;
      if (!document) continue;
      const holding = {
        name,
        artifact_id: String(value.artifact_id || "").trim(),
        content_hash: String(value.content_hash || "").trim(),
      };
      if (!holding.artifact_id || !holding.content_hash) continue;
      void boot.layerStore.putLayerByRef(appId, sceneId, holding, document, manifest);
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
      preview_render: previewRenderSummary(),
      boot_apis: {
        tryCacheFirstViewRestore: typeof boot.tryCacheFirstViewRestore === "function",
        assembleViaViewRevision: typeof boot.assembleViaViewRevision === "function",
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

    if (ctx && boot.viewRevisionClient?.negotiateWithLocalMiss) {
      try {
        const vrCtx = {
          app_id: ctx.app_id || ctx.appId,
          scene_id: ctx.scene_id || ctx.sceneId,
          surface: ctx.surface || ctx.mode || "app",
          data_mode: ctx.data_mode || ctx.dataMode || "",
          review_projection: ctx.review_projection || ctx.reviewProjection || "",
          chrome: ctx.chrome || "",
        };
        const assembled = await boot.viewRevisionClient.negotiateWithLocalMiss(vrCtx, {
          silent: true,
        });
        report.assemble = {
          ok: !!assembled?.assemble?.ok,
          outcome: assembled?.outcome,
          missing: assembled?.assemble?.missing || [],
        };
        report.host_chrome_after = hostChromeSummary();
        report.preview_render_after = previewRenderSummary();
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
  boot.ensureViewShellLayout = ensureViewShellLayout;
  boot.hostChromeReady = hostChromeReady;
  boot.isHostChromeSuppressed = isHostChromeSuppressed;
  boot.isSsrShellPlaceholder = isSsrShellPlaceholder;
  boot.hasMaterializedPreview = hasMaterializedPreview;
  boot.hydrateManifestLayerHoldings = hydrateManifestLayerHoldings;
  boot.runThinShellDiagnostic = runThinShellDiagnostic;
  global.runMeiThinShellDiagnostic = runThinShellDiagnostic;
})(typeof window !== "undefined" ? window : globalThis);
