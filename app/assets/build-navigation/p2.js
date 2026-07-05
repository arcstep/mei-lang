        panel.hidden = slug !== tab;
      });
      return;
    }
    if (typeof boot.switchManageTab === "function") {
      boot.switchManageTab(tab, { updateUrl: false, emit: opts.emit !== false });
      return;
    }
    document.querySelectorAll("[data-manage-tab-panel]").forEach((panel) => {
      if (!(panel instanceof HTMLElement)) return;
      const slug = String(panel.getAttribute("data-manage-tab-panel") || "").trim().toLowerCase();
      panel.hidden = slug !== tab;
    });
  }

  function wakePreviewRuntime(scope, options) {
    const opts = options || {};
    if (opts.fromCache === true) {
      if (typeof boot.scheduleFrameViewportRelayout === "function") {
        try {
          boot.scheduleFrameViewportRelayout();
        } catch (_) {}
      }
      return;
    }
    const resetCache = opts.resetRuntimeQueryCache === true;
    if (typeof boot.scheduleFrameViewportRelayout === "function") {
      try {
        boot.scheduleFrameViewportRelayout();
      } catch (_) {}
    }
    if (typeof publishManagePreviewFromDoc === "function") {
      publishManagePreviewFromDoc(document, { resetRuntimeQueryCache: resetCache });
    }
    if (typeof boot.mountManagePreviewBoard === "function") {
      void boot.mountManagePreviewBoard(document);
    }
    const dispatchPreviewUpdated = (pass) => {
      try {
        global.dispatchEvent(
          new CustomEvent("meilang:preview-updated", {
            detail: {
              scope: scope || "build-nav",
              resetRuntimeQueryCache: pass === 0 ? resetCache : false,
            },
          }),
        );
      } catch (_) {}
    };
    dispatchPreviewUpdated(0);
    if (opts.pulsePreviewUpdated) {
      global.requestAnimationFrame(() => {
        if (typeof boot.scheduleFrameViewportRelayout === "function") {
          try {
            boot.scheduleFrameViewportRelayout();
          } catch (_) {}
        }
        global.requestAnimationFrame(() => {
          dispatchPreviewUpdated(1);
          if (typeof boot.mountManagePreviewBoard === "function") {
            void boot.mountManagePreviewBoard(document);
          }
        });
      });
    }
  }

  function runTier0PostNav(prevUrl) {
    global.__meiBuildNavPrevUrl = String(prevUrl || global.location.href);
    ensurePreviewTabVisible(global.location.href, null, { emit: false });
    const previewRoot =
      document.querySelector(".preview-pane-scroll") ||
      document.querySelector('[data-manage-tab-panel="preview"] .preview-pane-scroll');
    if (previewRoot instanceof HTMLElement && global.MeiProjectionDepth?.applyProjectionDepth) {
      try {
        const parsed = new URL(global.location.href);
        global.MeiProjectionDepth.applyProjectionDepth(previewRoot, {
          reviewProjection: parsed.searchParams.get("review_projection") || "",
        });
      } catch (_) {}
    }
    if (boot.viewCompositor?.clearComposeArtifacts && previewRoot instanceof HTMLElement) {
      boot.viewCompositor.clearComposeArtifacts(previewRoot);
    }
    document.body.classList.remove("access-drilldown-open", "access-scene-board-open");
    if (typeof closeDrilldownOverlay === "function") {
      try {
        closeDrilldownOverlay();
      } catch (_) {}
    }
    if (typeof global.MeiBuildInspectHighlight?.refresh === "function") {
      global.MeiBuildInspectHighlight.refresh({ scope: "build-inspect" });
    }
    if (typeof global.MeiBuildTreePersist?.refresh === "function") {
      global.MeiBuildTreePersist.refresh();
    }
    if (typeof boot.activateManagePreviewBoardPool === "function") {
      boot.activateManagePreviewBoardPool(document);
    }
    const prevNode = nodeIdFromUrl(prevUrl || global.location.href);
    const nextNode = nodeIdFromUrl(global.location.href);
    if (boardExportChanged(prevNode, nextNode)) {
      wakePreviewRuntime("build-nav-board-export");
      return;
    }
    if (/^(?:ui-scope|scene-panel|scene-block):/i.test(nextNode)) {
      return;
    }
    if (isPackCatalogNodeId(nextNode) && nodeIdChanged(prevUrl, nextNode)) {
      wakePreviewRuntime("build-nav-catalog-node", {
        resetRuntimeQueryCache: true,
        pulsePreviewUpdated: true,
      });
    }
  }

  function shouldSkipPreviewRuntimeWake(prevUrl, nextUrl) {
    if (isSameSceneStructureNav(prevUrl, nextUrl)) return true;
    if (classifyBuildNavTier(prevUrl, nextUrl) !== "client") return false;
    const prevNode = nodeIdFromUrl(prevUrl);
    const nextNode = nodeIdFromUrl(nextUrl);
    if (boardExportChanged(prevNode, nextNode)) return false;
    return true;
  }

  function shouldWakePreviewRuntime(prevUrl, nextUrl) {
    return !shouldSkipPreviewRuntimeWake(prevUrl, nextUrl);
  }

  function applyBootstrapScripts(drilldownScript) {
    const raw = String(drilldownScript || "").trim();
    if (!raw) return;
    const scriptTpl = document.createElement("template");
    scriptTpl.innerHTML = raw;
    ["mei-scene-drilldown-context", "mei-host-runtime-capabilities"].forEach((id) => {
      const next = scriptTpl.content.querySelector(`#${CSS.escape(id)}`);
      if (!(next instanceof HTMLScriptElement)) return;
      const existing = document.getElementById(id);
      if (existing instanceof HTMLScriptElement) {
        existing.textContent = next.textContent || "";
        return;
      }
      document.body.appendChild(next.cloneNode(true));
    });
    try {
      delete global.__meiSceneDrilldownContext;
      delete global.__meiHostRuntimeCapabilities;
    } catch (_) {}
  }

  function swapPreviewFragment(previewHtml, drilldownScript) {
    const panel = document.querySelector('[data-manage-tab-panel="preview"]');
    if (!panel) return false;
    const html = String(previewHtml || "").trim();
    if (!html) return false;
    const tpl = document.createElement("template");
    tpl.innerHTML = html;
    const nextScroll = tpl.content.querySelector(".preview-pane-scroll");
    const scroll = panel.querySelector(".preview-pane-scroll");
    if (nextScroll instanceof HTMLElement && scroll instanceof HTMLElement) {
      scroll.replaceWith(document.importNode(nextScroll, true));
    } else if (scroll instanceof HTMLElement) {
      scroll.innerHTML = html;
    } else {
      panel.innerHTML = html;
    }
    const nextBar = tpl.content.querySelector("#build-inspect-bar");
    const curBar = panel.querySelector("#build-inspect-bar");
    if (nextBar instanceof HTMLElement && curBar instanceof HTMLElement) {
      curBar.replaceWith(document.importNode(nextBar, true));
    }
    applyBootstrapScripts(drilldownScript);
    const previewScroll = panel.querySelector(".preview-pane-scroll");
    if (previewScroll instanceof HTMLElement && global.MeiProjectionDepth?.applyProjectionDepth) {
      try {
        const parsed = new URL(global.location.href);
        global.MeiProjectionDepth.applyProjectionDepth(previewScroll, {
          reviewProjection: parsed.searchParams.get("review_projection") || "",
        });
      } catch (_) {}
    }
    return true;
  }

  function resolveBuildNode(url) {
    const host = hostBoot();
    const fn =
      host.resolveBuildFragmentNode || global.MeiBuildFragmentRevision?.resolveBuildFragmentNode;
    if (typeof fn === "function") {
      const resolved = fn.call(host, url);
      if (resolved) return resolved;
    }
    return nodeIdFromUrl(url);
  }

  function viewRevisionCtxFromUrl(url) {
    const parsed = new URL(url, global.location.href);
    const appId =
      typeof appIdFromAppsPathname === "function"
        ? appIdFromAppsPathname(parsed.pathname)
        : parsed.pathname.split("/").filter(Boolean)[2] || "";
    const node = resolveBuildNode(url);
    const surface =
      typeof workspaceSurfaceSlugFromAppsPathname === "function"
        ? workspaceSurfaceSlugFromAppsPathname(parsed.pathname) || "build"
        : "build";
    return {
      app_id: appId,
      scene_id: parsed.searchParams.get("scene") || "home",
      surface,
      node,
      data_mode: parsed.searchParams.get("data_mode") || "",
      review_projection: parsed.searchParams.get("review_projection") || "",
      focus: parsed.searchParams.get("focus") || "",
      scope: parsed.searchParams.get("scope") || "",
      tab: parsed.searchParams.get("tab") || "preview",
    };
  }

  async function tryBuildViewRevisionAssemble(url) {
    const host = hostBoot();
    if (!host.viewRevisionClient?.negotiateWithLocalMiss) return null;
    if (host.viewRevisionClient.isEnabled && !host.viewRevisionClient.isEnabled()) {
      return null;
    }
    try {
      const ctx = viewRevisionCtxFromUrl(url);
      const result = await host.viewRevisionClient.negotiateWithLocalMiss(ctx);
      if (result?.assemble?.ok) {
        const outcome =
          result.outcome === (host.ViewRevisionOutcome?.REFETCH || "refetch")
            ? "refetch"
            : (host.ViewRevisionOutcome?.ASSEMBLE_LOCAL || "assemble_local");
        ensurePreviewTabVisible(url, null, { emit: false });
        wakePreviewRuntime("build-view-revision", { fromCache: true });
        global.__meiBuildPreviewRestoredFromCache = 1;
        if (typeof host.cacheDiagTrace === "function") {
          host.cacheDiagTrace("view-revision-outcome", {
            outcome,
            surface: "build",
          });
        }
        return {
          restored: true,
          source: outcome,
          revision: result.response,
        };
      }
      if (result?.outcome === (host.ViewRevisionOutcome?.LOCAL_MISS || "local_miss")) {
        if (typeof host.cacheDiagTrace === "function") {
          host.cacheDiagTrace("view-revision-outcome", { outcome: "local_miss", surface: "build" });
        }
        return { restored: false, source: "local_miss", revision: result.response };
      }
    } catch (error) {
      console.warn("[build-navigation] view-revision assemble skipped", error);
    }
    return null;
  }

  async function fetchWorkspaceFragment(url) {
    const parsed = new URL(url, global.location.href);
    const appId =
      typeof appIdFromAppsPathname === "function"
        ? appIdFromAppsPathname(parsed.pathname)
        : parsed.pathname.split("/").filter(Boolean)[2] || "";
    const node = resolveBuildNode(url);
    if (!node) {
      throw new Error("build workspace fragment requires resolved node id");
    }
    const params = new URLSearchParams({
      app_id: appId,
      node,
      tab: buildTab(url) || "preview",
    });
    const focus = parsed.searchParams.get("focus");
    if (focus) params.set("focus", focus);
    const scope = parsed.searchParams.get("scope");
    if (scope) params.set("scope", scope);
    const reviewProjection = parsed.searchParams.get("review_projection");
    if (reviewProjection) params.set("review_projection", reviewProjection);
    const dataMode = parsed.searchParams.get("data_mode");
    if (dataMode) params.set("data_mode", dataMode);
    const draftHeaders =
      typeof ensureDraftSessionId === "function"
        ? { "x-mei-draft-session": ensureDraftSessionId() }
        : {};
    const controller = new AbortController();
    const timer = global.setTimeout(() => controller.abort(), FRAGMENT_FETCH_TIMEOUT_MS);
    try {
      const resp = await fetch(`/api/build/workspace-fragment?${params.toString()}`, {
        credentials: "same-origin",
        headers: {
          Accept: "application/json",
          "x-mei-spa-nav": "1",
          ...draftHeaders,
        },
        signal: controller.signal,
      });
      if (!resp.ok) throw new Error(`fragment failed: ${resp.status}`);
      return { payload: await resp.json(), resp };
    } finally {
      global.clearTimeout(timer);
    }
  }

  function hostBoot() {
    return global.__meiLangBoot || globalThis.__meiLangBoot || boot;
  }

  function fetchBuildRevision(url, options) {
    const host = hostBoot();
    const fn =
      host.fetchBuildFragmentRevision ||
      global.MeiBuildFragmentRevision?.fetchBuildFragmentRevision;
    if (typeof fn !== "function") return null;
    return fn.call(host, url, options);
  }

  function readBuildRevision(url) {
    const host = hostBoot();
    const fn =
      host.readBuildFragmentRevision || global.MeiBuildFragmentRevision?.readBuildFragmentRevision;
    return typeof fn === "function" ? fn.call(host, url) : null;
  }

  function readBuildFragmentCache(url, revision) {
    const host = hostBoot();
    const fn =
      host.readBuildFragmentHtml || global.MeiBuildFragmentRevision?.readBuildFragmentHtml;
    return typeof fn === "function" ? fn.call(host, url, revision) : null;
  }

  function rememberBuildRevision(url, revision) {
    const host = hostBoot();
    const fn =
      host.rememberBuildFragmentRevision ||
      global.MeiBuildFragmentRevision?.rememberBuildFragmentRevision;
    if (typeof fn === "function") fn.call(host, url, revision);
  }

  function rememberBuildFragment(url, revision, payload) {
    const host = hostBoot();
    const fn =
      host.rememberBuildFragmentHtml || global.MeiBuildFragmentRevision?.rememberBuildFragmentHtml;
    if (typeof fn === "function") fn.call(host, url, revision, payload);
  }

  function buildRevisionStillValid(url, revision) {
    const host = hostBoot();
    const fn =
      host.buildFragmentRevisionStillValid ||
      global.MeiBuildFragmentRevision?.buildFragmentRevisionStillValid;
    return typeof fn === "function" ? fn.call(host, url, revision) : false;
  }

  async function persistBuildPreviewSnapshot(url, knownRevision) {
    const tab = buildTab(url);
    if (tab && tab !== "preview") return false;
    const panel = document.querySelector('[data-manage-tab-panel="preview"]');
    const scroll = panel?.querySelector(".preview-pane-scroll");
    if (!(scroll instanceof HTMLElement)) return false;
    let revision = knownRevision || readBuildRevision(url);
    if (!revision) return false;
    const drilldownParts = ["mei-scene-drilldown-context", "mei-host-runtime-capabilities"]
      .map((id) => {
        const node = document.getElementById(id);
        return node instanceof HTMLElement ? node.outerHTML : "";
      })
      .filter(Boolean);
    const parsed = new URL(url, global.location.href);
    const payload = {
      preview_html: scroll.outerHTML,
      drilldown_script: drilldownParts.join(""),
      workspace_scripts: [],
      node: resolveBuildNode(url),
      focus: String(parsed.searchParams.get("focus") || ""),
      revision,
    };
    rememberBuildRevision(url, revision);
    rememberBuildFragment(url, revision, payload);
    if (typeof boot.cacheDiagTrace === "function") {
      boot.cacheDiagTrace("build-preview-persisted", {
        url,
        revision_digest: revision.revision_digest,
        previewHtmlBytes: String(payload.preview_html || "").length,
      });
    }
    return true;
  }

  function scheduleEagerBuildPreviewPersist(url, revision) {
    const run = () => {
      void persistBuildPreviewSnapshot(url, revision || readBuildRevision(url));
    };
    if (document.readyState === "loading") {
      document.addEventListener("DOMContentLoaded", run, { once: true });
    } else {
      global.requestAnimationFrame(run);
    }
    scheduleBuildPreviewPersist(url, revision);
  }

  function scheduleBuildPreviewPersist(url, knownRevision) {
    const run = () => {
      void persistBuildPreviewSnapshot(url, knownRevision || readBuildRevision(url));
    };
    global.addEventListener("pagehide", run, { once: true });
    document.addEventListener("visibilitychange", () => {
      if (document.visibilityState === "hidden") run();
    });
  }

  async function tryRestoreBuildPreviewFromCache(url, options) {
    const opts = options || {};
    const parsed = new URL(url, global.location.href);
    const tab = buildTab(url);
    if (tab && tab !== "preview") {
      return { restored: false, source: "not-preview" };
    }
    if (
      typeof hostBoot().fetchBuildFragmentRevision !== "function" &&
      typeof global.MeiBuildFragmentRevision?.fetchBuildFragmentRevision !== "function"
    ) {
      return { restored: false, source: "no-revision-api" };
    }
    try {
      const viewRevisionOutcome = await tryBuildViewRevisionAssemble(url);
      if (viewRevisionOutcome?.restored) {
        return viewRevisionOutcome;
      }

      let revision = await fetchBuildRevision(url, {
        timeoutMs: opts.timeoutMs || 8000,
        skipRemoteWhenValid: opts.skipRemoteWhenValid === true,
      });
      if (!revision) {
        revision = await fetchBuildRevision(url, { timeoutMs: opts.timeoutMs || 8000 });
      }
      if (!revision) {
        return { restored: false, source: "revision-miss", revision: null };
      }
      const cached = readBuildFragmentCache(url, revision);
      if (cached?.preview_html) {
        const ok = swapPreviewFragment(
          String(cached.preview_html || ""),
          String(cached.drilldown_script || ""),
        );
        if (!ok) {
          return { restored: false, source: "swap-failed", revision };
        }
        if (
          Array.isArray(cached.workspace_scripts) &&
          typeof boot.syncPreviewWorkspaceScripts === "function"
        ) {
          await boot.syncPreviewWorkspaceScripts(cached.workspace_scripts);
        }
        ensurePreviewTabVisible(url, null, { emit: false });
        wakePreviewRuntime("build-cold-cache", { fromCache: true });
        global.__meiBuildPreviewRestoredFromCache = 1;
        return { restored: true, source: "fragment-cache", revision };
      }
      if (
        !cached?.preview_html &&
        cached?.scene_manifest &&
        boot.viewRevisionClient?.tryAssembleLocal
      ) {
        const ctx = viewRevisionCtxFromUrl(url);
        const assemble = await boot.viewRevisionClient.tryAssembleLocal(ctx, {
          manifest: cached.scene_manifest,
          compose_defaults: cached.scene_manifest?.compose_defaults,
        });
        if (assemble?.ok) {
          ensurePreviewTabVisible(url, null, { emit: false });
          wakePreviewRuntime("build-manifest-cache", { fromCache: true });
          return { restored: true, source: "assemble_local", revision };
        }
      }
      rememberBuildRevision(url, revision);
      if (viewRevisionOutcome?.source === "local_miss") {
        return { restored: false, source: "local_miss", revision };
      }
      scheduleEagerBuildPreviewPersist(url, revision);
      return { restored: false, source: "fragment-miss", revision };
    } catch (error) {
      console.warn("[build-navigation] cold preview cache restore skipped", error);
      return { restored: false, source: "error" };
    }
  }

  async function tryComposeProjectionOnly(url) {
    try {
      const parsed = new URL(url, global.location.href);
      const reviewProjection = String(parsed.searchParams.get("review_projection") || "").trim();
      if (!reviewProjection) return false;
      const root = document.querySelector(".preview-pane-scroll, .shell");
      if (!root?.querySelector("[data-preview-scope], [data-mei-ui-role]")) {
        return false;
      }
      if (boot.viewCompositor?.clearComposeArtifacts) {
        boot.viewCompositor.clearComposeArtifacts(root);
      }
      if (global.MeiProjectionDepth?.applyReviewProjectionChrome) {
        global.MeiProjectionDepth.applyReviewProjectionChrome(root, {
          reviewProjection,
        });
        return true;
      }
      return false;
    } catch (_) {
      return false;
    }
  }

  async function navigateBuildTier1(url, replaceHistory, linkEl) {
    showBuildNavLoading(url);
    try {
      ensurePreviewTabVisible(url);
      if (await tryComposeProjectionOnly(url)) {
        global.__meiBuildPreviewRestoredFromCache = 1;
        return true;
      }
      const viewRevisionOutcome = await tryBuildViewRevisionAssemble(url);
      if (viewRevisionOutcome?.restored) {
        return true;
      }
      let payload = null;
      const fetchRev = fetchBuildRevision;
      if (typeof fetchRev === "function") {
        try {
          const remoteRevision = await fetchRev(url, { timeoutMs: 8000 });
          if (remoteRevision && buildRevisionStillValid(url, remoteRevision)) {
            const cached = readBuildFragmentCache(url, remoteRevision);
            if (cached?.preview_html) {
              payload = { ...cached, revision: remoteRevision };
            }
          }
        } catch (_) {}
      }
      if (!payload) {
        const fetched = await fetchWorkspaceFragment(url);
        payload = fetched.payload;
        if (payload?.revision) {
          rememberBuildRevision(url, payload.revision);
        }
        if (payload?.preview_html) {
          rememberBuildFragment(url, payload.revision, payload);
        } else if (payload?.scene_manifest) {
          rememberBuildFragment(url, payload.revision, payload);
        }
      }
      if (!payload?.preview_html && payload?.scene_manifest && boot.sceneManifestLoader && boot.viewCompositor) {
        const parsed = new URL(url, global.location.href);
        const ctx = viewRevisionCtxFromUrl(url);
        const appId = ctx.app_id || "";
        const sceneId = parsed.searchParams.get("scene") || "home";
        await boot.sceneManifestLoader.ensureLayers(
          [
            "structure.full",
            "eval.slot_group.scene:default",
            "theme.tokens",
            "layout.overlay",
            "shell.build",
          ],
          appId,
          sceneId,
          ctx,
          payload.scene_manifest,
        );
        if (boot.viewRevisionClient?.tryAssembleLocal) {
          const assemble = await boot.viewRevisionClient.tryAssembleLocal(ctx, {
            manifest: payload.scene_manifest,
            compose_defaults: payload.compose_defaults,
          });
          if (assemble?.ok) {
            return true;
          }
        }
        const batch = await boot.sceneManifestLoader.fetchLayerBatch(
          appId,
          sceneId,
          ["structure.full"],
          boot.sceneManifestLoader.readShellAxes(),
        );
        const structure = batch?.layers?.["structure.full"];
        const projection =
          parsed.searchParams.get("review_projection") ||
          payload.compose_defaults?.review_projection ||
          "live_full";
        const root = document.querySelector(".preview-pane-scroll, .shell");
        boot.viewCompositor.composePreview(root, structure, projection, null, null);
        return true;
      }
      const ok = swapPreviewFragment(
        String(payload?.preview_html || ""),
        String(payload.drilldown_script || ""),
      );
      if (!ok) return false;
      if (
        Array.isArray(payload.workspace_scripts) &&
        typeof boot.syncPreviewWorkspaceScripts === "function"
      ) {
        await boot.syncPreviewWorkspaceScripts(payload.workspace_scripts);
      }
      const shell = document.querySelector(".shell");
      if (shell) {
        if (payload.node) shell.setAttribute("data-build-node", String(payload.node));
        if (payload.focus) shell.setAttribute("data-build-focus", String(payload.focus));
        const coord = payload.compile_coordinate;
        if (coord && typeof coord === "object") {
          shell.setAttribute("data-compile-scene", String(coord.scene_id || ""));
          shell.setAttribute("data-compile-target", String(coord.preview_target || ""));
        }
        const parsed = new URL(url, global.location.href);
        const tab = String(parsed.searchParams.get("tab") || "").trim();
        const node = String(parsed.searchParams.get("node") || "").trim();
        const dataMode = String(parsed.searchParams.get("data_mode") || "").trim();
        const reviewProjection = String(
          parsed.searchParams.get("review_projection") || "",
        ).trim();
        if (dataMode) shell.setAttribute("data-data-mode", dataMode);
        if (reviewProjection) {
          shell.setAttribute("data-review-projection", reviewProjection);
        } else {
          shell.setAttribute("data-review-projection", "plane_region_section");
        }
        const resolvedTab = tab || inferPreviewTabFromNodeId(node);
        if (resolvedTab) shell.setAttribute("data-build-tab", resolvedTab);
        if (typeof syncBuildPresetTabs === "function") {
          syncBuildPresetTabs(
            dataMode || shell.getAttribute("data-data-mode"),
            reviewProjection || shell.getAttribute("data-review-projection"),
          );
        }
      }
      if (replaceHistory) global.history.replaceState({}, "", url);
      else global.history.pushState({}, "", url);
      lastBuildNavUrl = url;
      stats.tier1 += 1;
      runTier0PostNav(url);
      const nextNode = nodeIdFromUrl(url);
      const axesChange =
        typeof reviewAxesChanged === "function"
          ? reviewAxesChanged(lastBuildNavUrl, url)
          : { dataModeChanged: false };
      wakePreviewRuntime("build-fragment", {
        resetRuntimeQueryCache: axesChange.dataModeChanged || isPackCatalogNodeId(nextNode),
        pulsePreviewUpdated: true,
      });
      return true;
    } finally {
      clearBuildNavLoading();
    }
  }

  async function tryNavigateBuild(fromUrl, toUrl, options) {
    if (buildNavInFlight) {
      return { handled: false, tier: 2, reason: "in_flight" };
    }
    const host = hostBoot();
    if (typeof host.beginClientCommand === "function") {
      host.beginClientCommand({ kind: "BUILD_NAV", label: String(toUrl || "") });
    }
    const opts = options || {};
    const structureNav = isSameSceneStructureNav(fromUrl, toUrl);
    let tier = classifyBuildNavTier(fromUrl, toUrl, opts.linkEl);
    if (structureNav && tier === "full") {
      tier = "client";
    }
    global.__meiBuildNavLastTier = { tier, structureNav, fromUrl, toUrl };

    const finishTier0 = () => {
      syncBuildShellUrl(toUrl, !!opts.replaceHistory, opts.linkEl);
      stats.tier0 += 1;
      runTier0PostNav(fromUrl);
      return { handled: true, tier: 0 };
    };

    if (tier === "client") {
      if (structureNav || tier0TargetReady(toUrl)) {
        buildNavInFlight = true;
        try {
          return finishTier0();
        } finally {
          buildNavInFlight = false;
        }
      }
      buildNavInFlight = true;
      try {
        const ok = await navigateBuildTier1(toUrl, !!opts.replaceHistory, opts.linkEl);
        if (ok) return { handled: true, tier: 1 };
      } catch (err) {
        console.warn("[build-navigation] tier0 missing DOM; tier1 failed", err);
      } finally {
        buildNavInFlight = false;
      }
      stats.tier2 += 1;
      return { handled: false, tier: 2 };
    }
    if ((tier === "fragment" || structureNav) && !opts.skipFragment) {
      if (structureNav) {
        buildNavInFlight = true;
        try {
          return finishTier0();
        } finally {
          buildNavInFlight = false;
        }
      }
      buildNavInFlight = true;
      try {
        const ok = await navigateBuildTier1(toUrl, !!opts.replaceHistory, opts.linkEl);
        if (ok) return { handled: true, tier: 1 };
      } catch (err) {
        console.warn("[build-navigation] tier1 failed; fallback to full SPA", err);
      } finally {
        buildNavInFlight = false;
      }
    }
    if (structureNav) {
      buildNavInFlight = true;
      try {
        return finishTier0();
      } finally {
        buildNavInFlight = false;
      }
    }
    stats.tier2 += 1;
    return { handled: false, tier: 2 };
  }

  async function tryHandleBuildClick(event, targetUrl, replaceHistory) {
    try {
      if (!targetUrl || !isBuildWorkspacePathname(new URL(targetUrl, global.location.href).pathname)) {
        return false;
      }
      let linkEl = null;
      const path = event?.composedPath ? event.composedPath() : [];
      for (const item of path) {
        if (item instanceof HTMLAnchorElement && item.hasAttribute("data-build-node")) {
          linkEl = item;
          break;
        }
      }
      const result = await tryNavigateBuild(global.location.href, targetUrl, {
        replaceHistory,
        linkEl,
      });
      return result.handled;
    } catch (err) {
      console.warn("[build-navigation] click handler failed; fallback to SPA", err);
      clearBuildNavLoading();
      return false;
    }
  }

  global.MeiBuildNavigation = {
    buildTab,
    inferPreviewTabFromNodeId,
    treeLinkTab,
    isSameSceneStructureNav,
    readCompileCoordinate,
    coordinatesEqual,
    readDataModeFromUrl,
    readReviewProjectionFromUrl,
    reviewAxesChanged,
    classifyBuildNavTier,
    tryNavigateBuild,
    tryRestoreBuildPreviewFromCache,
    scheduleEagerBuildPreviewPersist,
    scheduleBuildPreviewPersist,
    persistBuildPreviewSnapshot,
    tryHandleBuildClick,
    shouldSkipPreviewRuntimeWake,
    shouldWakePreviewRuntime,
    swapPreviewFragment,
    getLastUrl: () => lastBuildNavUrl,
    noteUrl: (url) => {
      lastBuildNavUrl = String(url || global.location.href);
    },
    stats,
  };
  global.MeiBuildNavigation.noteUrl(global.location.href);
})(window);
