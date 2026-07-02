        panel.hidden = slug !== tab;
      });
      return;
    }
    if (typeof boot.switchManageTab === "function") {
      boot.switchManageTab(tab, { updateUrl: false, emit: true });
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
    ensurePreviewTabVisible(global.location.href);
    document.body.classList.remove("access-drilldown-open", "access-scene-board-open");
    if (typeof closeDrilldownOverlay === "function") {
      try {
        closeDrilldownOverlay();
      } catch (_) {}
    }
    if (typeof global.MeiBuildInspectHighlight?.refresh === "function") {
      global.MeiBuildInspectHighlight.refresh();
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
    if (isPackCatalogNodeId(nextNode) && nodeIdChanged(prevUrl, nextNode)) {
      wakePreviewRuntime("build-nav-catalog-node", {
        resetRuntimeQueryCache: true,
        pulsePreviewUpdated: true,
      });
    }
  }

  function shouldSkipPreviewRuntimeWake(prevUrl, nextUrl) {
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
    return true;
  }

  async function fetchWorkspaceFragment(url) {
    const parsed = new URL(url, global.location.href);
    const parts = parsed.pathname.split("/").filter(Boolean);
    const appId = parts[2] || "";
    const params = new URLSearchParams({
      app_id: appId,
      node: String(parsed.searchParams.get("node") || ""),
      tab: buildTab(url) || "preview",
    });
    const focus = parsed.searchParams.get("focus");
    if (focus) params.set("focus", focus);
    const controller = new AbortController();
    const timer = global.setTimeout(() => controller.abort(), FRAGMENT_FETCH_TIMEOUT_MS);
    try {
      const resp = await fetch(`/api/build/workspace-fragment?${params.toString()}`, {
        credentials: "same-origin",
        headers: { Accept: "application/json", "x-mei-spa-nav": "1" },
        signal: controller.signal,
      });
      if (!resp.ok) throw new Error(`fragment failed: ${resp.status}`);
      return { payload: await resp.json(), resp };
    } finally {
      global.clearTimeout(timer);
    }
  }

  async function navigateBuildTier1(url, replaceHistory, linkEl) {
    showBuildNavLoading(url);
    try {
      ensurePreviewTabVisible(url);
      const { payload } = await fetchWorkspaceFragment(url);
      const ok = swapPreviewFragment(
        String(payload.preview_html || ""),
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
        const resolvedTab = tab || inferPreviewTabFromNodeId(node);
        if (resolvedTab) shell.setAttribute("data-build-tab", resolvedTab);
      }
      if (replaceHistory) global.history.replaceState({}, "", url);
      else global.history.pushState({}, "", url);
      lastBuildNavUrl = url;
      stats.tier1 += 1;
      runTier0PostNav(url);
      const nextNode = nodeIdFromUrl(url);
      wakePreviewRuntime("build-fragment", {
        resetRuntimeQueryCache: isPackCatalogNodeId(nextNode),
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
    const opts = options || {};
    const tier = classifyBuildNavTier(fromUrl, toUrl, opts.linkEl);
    if (tier === "client") {
      if (!tier0TargetReady(toUrl)) {
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
      buildNavInFlight = true;
      try {
        syncBuildShellUrl(toUrl, !!opts.replaceHistory, opts.linkEl);
        stats.tier0 += 1;
        runTier0PostNav(fromUrl);
        return { handled: true, tier: 0 };
      } finally {
        buildNavInFlight = false;
      }
    }
    if (tier === "fragment" && !opts.skipFragment) {
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
    readCompileCoordinate,
    coordinatesEqual,
    classifyBuildNavTier,
    tryNavigateBuild,
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
