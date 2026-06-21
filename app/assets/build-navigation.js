/**
 * Build view fast navigation: Tier0 client-only (same compile coordinate),
 * Tier1 workspace fragment fetch, Tier2 full SPA fallback.
 */
(function (global) {
  "use strict";

  const boot = (global.__meiLangBoot = global.__meiLangBoot || {});
  const stats = { tier0: 0, tier1: 0, tier2: 0 };
  global.__meiBuildNavStats = stats;
  let lastBuildNavUrl = global.location.href;

  function isBuildWorkspacePathname(pathname) {
    return /^\/apps\/(?:build|manage)\//.test(String(pathname || ""));
  }

  function buildTab(rawUrl) {
    try {
      const parsed = new URL(rawUrl, global.location.href);
      const fromQuery = String(parsed.searchParams.get("tab") || "").trim().toLowerCase();
      if (fromQuery) return fromQuery;
      const shell = document.querySelector(".shell[data-build-tab]");
      return String(shell?.getAttribute("data-build-tab") || "overview").trim().toLowerCase();
    } catch (_) {
      return "overview";
    }
  }

  function readCompileCoordinateFromReachabilityTree(nodeId) {
    const id = String(nodeId || "").trim();
    if (!id) return null;
    const script = document.getElementById("mei-build-reachability-tree");
    if (!script) return null;
    try {
      const roots = JSON.parse(script.textContent || "[]");
      if (!Array.isArray(roots)) return null;
      const walk = (nodes) => {
        for (const node of nodes || []) {
          if (node?.node_id === id) {
            const target = String(node.compile_target || "").trim();
            if (!target) return null;
            return {
              scene: String(node.compile_scene || "").trim(),
              target,
            };
          }
          const nested = walk(node.children);
          if (nested) return nested;
        }
        return null;
      };
      for (const root of roots) {
        const found = walk(root.children);
        if (found) return found;
      }
    } catch (_) {}
    return null;
  }

  function nodeIdFromUrl(rawUrl) {
    try {
      return String(new URL(rawUrl, global.location.href).searchParams.get("node") || "").trim();
    } catch (_) {
      return "";
    }
  }

  function readCompileCoordinateFromLink(el) {
    if (!(el instanceof HTMLElement)) return null;
    const anchor =
      el.matches("a[data-compile-target]") ? el : el.closest("a[data-compile-target]");
    if (!(anchor instanceof HTMLElement)) return null;
    const target = String(anchor.getAttribute("data-compile-target") || "").trim();
    if (!target) return null;
    return {
      scene: String(anchor.getAttribute("data-compile-scene") || "").trim(),
      target,
    };
  }

  function readCompileCoordinateFromShell() {
    const shell = document.querySelector(".shell[data-compile-target]");
    if (!shell) return null;
    const target = String(shell.getAttribute("data-compile-target") || "").trim();
    if (!target) return null;
    return {
      scene: String(shell.getAttribute("data-compile-scene") || "").trim(),
      target,
    };
  }

  function readCompileCoordinate(rawUrl, linkEl) {
    const fromLink = linkEl ? readCompileCoordinateFromLink(linkEl) : null;
    if (fromLink) return fromLink;
    const fromTree = readCompileCoordinateFromReachabilityTree(nodeIdFromUrl(rawUrl));
    if (fromTree) return fromTree;
    return readCompileCoordinateFromShell();
  }

  function coordinatesEqual(a, b) {
    if (!a || !b) return false;
    return a.scene === b.scene && a.target === b.target;
  }

  function classifyBuildNavTier(fromUrl, toUrl, linkEl) {
    try {
      const from = new URL(fromUrl, global.location.href);
      const to = new URL(toUrl, global.location.href);
      if (!isBuildWorkspacePathname(to.pathname)) return "full";
      if (!isBuildWorkspacePathname(from.pathname)) return "full";
      if (from.pathname !== to.pathname) return "full";
      if (buildTab(toUrl) !== "preview") return "full";
      const toCoord = readCompileCoordinate(toUrl, linkEl);
      const fromCoord = readCompileCoordinate(fromUrl);
      if (!toCoord || !fromCoord) return "full";
      if (coordinatesEqual(fromCoord, toCoord)) return "client";
      return "fragment";
    } catch (_) {
      return "full";
    }
  }

  function syncBuildShellUrl(url, replaceHistory, linkEl) {
    const parsed = new URL(url, global.location.href);
    const shell = document.querySelector(".shell");
    if (shell) {
      const node = String(parsed.searchParams.get("node") || "").trim();
      const focus = String(parsed.searchParams.get("focus") || "").trim();
      const tab = String(parsed.searchParams.get("tab") || "").trim();
      if (node) shell.setAttribute("data-build-node", node);
      else shell.removeAttribute("data-build-node");
      if (focus) shell.setAttribute("data-build-focus", focus);
      else shell.removeAttribute("data-build-focus");
      if (tab) shell.setAttribute("data-build-tab", tab);
      const coord = readCompileCoordinate(url, linkEl);
      if (coord) {
        shell.setAttribute("data-compile-scene", coord.scene);
        shell.setAttribute("data-compile-target", coord.target);
      }
    }
    if (replaceHistory) global.history.replaceState({}, "", url);
    else global.history.pushState({}, "", url);
    lastBuildNavUrl = url;
  }

  function ensurePreviewTabVisible(rawUrl) {
    const tab = buildTab(rawUrl);
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

  function wakePreviewRuntime(scope) {
    if (typeof boot.scheduleFrameViewportRelayout === "function") {
      try {
        boot.scheduleFrameViewportRelayout();
      } catch (_) {}
    }
    if (typeof publishManagePreviewFromDoc === "function") {
      publishManagePreviewFromDoc(document, { resetRuntimeQueryCache: false });
    }
    if (typeof boot.mountManagePreviewBoard === "function") {
      void boot.mountManagePreviewBoard(document);
    }
    try {
      global.dispatchEvent(
        new CustomEvent("meilang:preview-updated", {
          detail: { scope: scope || "build-nav" },
        }),
      );
    } catch (_) {}
  }

  function runTier0PostNav() {
    ensurePreviewTabVisible(global.location.href);
    document.body.classList.remove("access-drilldown-open", "access-scene-board-open");
    if (typeof closeDrilldownOverlay === "function") {
      try {
        closeDrilldownOverlay();
      } catch (_) {}
    }
    if (typeof boot.clearManageWorkspaceLoadingState === "function") {
      boot.clearManageWorkspaceLoadingState();
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
  }

  function shouldSkipPreviewRuntimeWake(prevUrl, nextUrl) {
    return classifyBuildNavTier(prevUrl, nextUrl) === "client";
  }

  function shouldWakePreviewRuntime(prevUrl, nextUrl) {
    return !shouldSkipPreviewRuntimeWake(prevUrl, nextUrl);
  }

  function swapPreviewFragment(previewHtml, drilldownScript) {
    const panel = document.querySelector('[data-manage-tab-panel="preview"]');
    if (!panel) return false;
    const tpl = document.createElement("template");
    tpl.innerHTML = previewHtml;
    const nextScroll = tpl.content.querySelector(".preview-pane-scroll");
    const scroll = panel.querySelector(".preview-pane-scroll");
    if (nextScroll instanceof HTMLElement && scroll instanceof HTMLElement) {
      scroll.replaceWith(nextScroll.cloneNode(true));
    } else if (scroll instanceof HTMLElement) {
      scroll.innerHTML = previewHtml;
    } else {
      panel.innerHTML = previewHtml;
    }
    const nextBar = tpl.content.querySelector("#build-inspect-bar");
    const curBar = panel.querySelector("#build-inspect-bar");
    if (nextBar instanceof HTMLElement && curBar instanceof HTMLElement) {
      curBar.replaceWith(nextBar.cloneNode(true));
    }
    if (drilldownScript) {
      const existing = document.getElementById("mei-scene-drilldown-context");
      if (existing) existing.remove();
      const tpl = document.createElement("template");
      tpl.innerHTML = drilldownScript;
      const script = tpl.content.querySelector("script");
      if (script) document.body.appendChild(script.cloneNode(true));
    }
    return true;
  }

  async function fetchWorkspaceFragment(url) {
    const parsed = new URL(url, global.location.href);
    const parts = parsed.pathname.split("/").filter(Boolean);
    const appId = parts[2] || "";
    const params = new URLSearchParams({
      app_id: appId,
      node: String(parsed.searchParams.get("node") || ""),
      tab: String(parsed.searchParams.get("tab") || "preview"),
    });
    const focus = parsed.searchParams.get("focus");
    if (focus) params.set("focus", focus);
    const resp = await fetch(`/api/build/workspace-fragment?${params.toString()}`, {
      credentials: "same-origin",
      headers: { Accept: "application/json", "x-mei-spa-nav": "1" },
    });
    if (!resp.ok) throw new Error(`fragment failed: ${resp.status}`);
    return { payload: await resp.json(), resp };
  }

  async function navigateBuildTier1(url, replaceHistory, fromUrl) {
    const { payload } = await fetchWorkspaceFragment(url);
    const ok = swapPreviewFragment(
      String(payload.preview_html || ""),
      String(payload.drilldown_script || ""),
    );
    if (!ok) return false;
    const shell = document.querySelector(".shell");
    if (shell) {
      if (payload.node) shell.setAttribute("data-build-node", String(payload.node));
      if (payload.focus) shell.setAttribute("data-build-focus", String(payload.focus));
      const coord = payload.compile_coordinate;
      if (coord && typeof coord === "object") {
        shell.setAttribute("data-compile-scene", String(coord.scene_id || ""));
        shell.setAttribute("data-compile-target", String(coord.preview_target || ""));
      }
    }
    if (replaceHistory) global.history.replaceState({}, "", url);
    else global.history.pushState({}, "", url);
    lastBuildNavUrl = url;
    stats.tier1 += 1;
    runTier0PostNav();
    wakePreviewRuntime("build-fragment");
    return true;
  }

  async function tryNavigateBuild(fromUrl, toUrl, options) {
    const opts = options || {};
    const tier = classifyBuildNavTier(fromUrl, toUrl, opts.linkEl);
    if (tier === "client") {
      syncBuildShellUrl(toUrl, !!opts.replaceHistory, opts.linkEl);
      stats.tier0 += 1;
      runTier0PostNav();
      return { handled: true, tier: 0 };
    }
    if (tier === "fragment") {
      try {
        const ok = await navigateBuildTier1(toUrl, !!opts.replaceHistory, fromUrl);
        if (ok) return { handled: true, tier: 1 };
      } catch (err) {
        console.warn("[build-navigation] tier1 failed; fallback to full SPA", err);
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
      return false;
    }
  }

  global.MeiBuildNavigation = {
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
