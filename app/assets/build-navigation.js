/**
 * Build view fast navigation: Tier0 client-only (same compile coordinate),
 * Tier1 workspace fragment fetch, Tier2 full SPA fallback.
 */
(function (global) {
  "use strict";

  const boot = (global.__meiLangBoot = global.__meiLangBoot || {});
  const stats = { tier0: 0, tier1: 0, tier2: 0 };
  const FRAGMENT_FETCH_TIMEOUT_MS = 8000;
  global.__meiBuildNavStats = stats;
  let lastBuildNavUrl = global.location.href;
  let buildNavInFlight = false;

  function isBuildWorkspacePathname(pathname) {
    return /^\/apps\/(?:build|manage)\//.test(String(pathname || ""));
  }

  function inferPreviewTabFromNodeId(nodeId) {
    const id = String(nodeId || "").trim();
    if (!id) return "";
    if (/^component:|^template:/i.test(id)) return "preview";
    if (/^(?:scene|route|projection):/i.test(id)) return "preview";
    if (/^board-(?:file|slot):/i.test(id)) return "preview";
    if (/^world-(?:dataset|metric|file):/i.test(id)) return "preview";
    if (/^dataset:/i.test(id)) return "preview";
    return "";
  }

  function buildTab(rawUrl) {
    try {
      const parsed = new URL(rawUrl, global.location.href);
      const fromQuery = String(parsed.searchParams.get("tab") || "").trim().toLowerCase();
      if (fromQuery) return fromQuery;
      const fromNode = inferPreviewTabFromNodeId(nodeIdFromUrl(rawUrl));
      if (fromNode) return fromNode;
      const shell = document.querySelector(".shell[data-build-tab]");
      return String(shell?.getAttribute("data-build-tab") || "overview").trim().toLowerCase();
    } catch (_) {
      return "overview";
    }
  }

  function isPackCatalogNodeId(nodeId) {
    return /^(?:component|template):/i.test(String(nodeId || "").trim());
  }

  function isBuildCatalogPreviewNode(nodeId) {
    return isPackCatalogNodeId(nodeId);
  }

  function nodeIdChanged(prevUrl, nextUrl) {
    return nodeIdFromUrl(prevUrl) !== nodeIdFromUrl(nextUrl);
  }

  function sceneIdFromNodeId(nodeId) {
    const raw = String(nodeId || "").trim();
    if (!raw) return "";
    const payload = raw.includes(":") ? raw.split(":").slice(1).join(":") : raw;
    const head = payload.split("/").filter(Boolean)[0];
    return head || "";
  }

  function inferCompileCoordinateFromNodeId(nodeId) {
    const id = String(nodeId || "").trim();
    if (!id) return null;
    if (/^board-(?:file|slot):/i.test(id)) {
      const payload = id.replace(/^board-(?:file|slot):/i, "");
      const slash = payload.indexOf("/");
      const boardKey = slash >= 0 ? payload.slice(0, slash) : payload;
      const hashAt = boardKey.indexOf("#");
      const file = hashAt >= 0 ? boardKey.slice(0, hashAt) : boardKey;
      const scene = hashAt >= 0 ? boardKey.slice(hashAt + 1) : "";
      if (file) return { scene, target: file };
    }
    if (/^(?:scene-panel|scene-block|scene|route):/i.test(id)) {
      const scene = sceneIdFromNodeId(id);
      const shell = readCompileCoordinateFromShell();
      if (shell?.target && (!shell.scene || !scene || shell.scene === scene)) {
        return { scene: scene || shell.scene, target: shell.target };
      }
    }
    if (/^(?:component|template):/i.test(id)) {
      const fromTree = readCompileCoordinateFromReachabilityTree(id);
      if (fromTree) return fromTree;
      const link = document.querySelector(
        `.build-reachability-tree a[data-build-node="${CSS.escape(id)}"]`,
      );
      if (link instanceof HTMLElement) {
        const target = String(link.getAttribute("data-compile-target") || "").trim();
        if (target) {
          return {
            scene: String(link.getAttribute("data-compile-scene") || "").trim(),
            target,
          };
        }
      }
    }
    return null;
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
    const nodeId = nodeIdFromUrl(rawUrl);
    const fromTree = readCompileCoordinateFromReachabilityTree(nodeId);
    if (fromTree) return fromTree;
    const inferred = inferCompileCoordinateFromNodeId(nodeId);
    if (inferred) return inferred;
    return readCompileCoordinateFromShell();
  }

  function coordinatesEqual(a, b) {
    if (!a || !b) return false;
    if (a.target !== b.target) return false;
    // Board capsule = file + scene_export; slot switches share capsule with board-file.
    return a.scene === b.scene;
  }

  function boardCapsuleKeyFromNodeId(nodeId) {
    const raw = String(nodeId || "").trim();
    if (!/^board-(?:file|slot):/i.test(raw)) return "";
    const payload = raw.replace(/^board-(?:file|slot):/i, "");
    const slash = payload.indexOf("/");
    return slash >= 0 ? payload.slice(0, slash) : payload;
  }

  function boardExportChanged(prevNodeId, nextNodeId) {
    if (!/^board-/i.test(String(nextNodeId || ""))) return false;
    const prevKey = boardCapsuleKeyFromNodeId(prevNodeId);
    const nextKey = boardCapsuleKeyFromNodeId(nextNodeId);
    if (!nextKey) return false;
    return prevKey !== nextKey;
  }

  function cssEscape(value) {
    const raw = String(value || "");
    if (typeof global.CSS !== "undefined" && typeof global.CSS.escape === "function") {
      return global.CSS.escape(raw);
    }
    return raw.replace(/\\/g, "\\\\").replace(/"/g, '\\"');
  }

  function panelScopePathFromNodeId(nodeId) {
    const raw = String(nodeId || "").trim();
    if (!/^scene-panel:/i.test(raw)) return "";
    const encoded = raw.replace(/^scene-panel:/i, "");
    const slash = encoded.indexOf("/");
    return slash >= 0 ? encoded.slice(slash + 1) : "";
  }

  function tier0PanelTargetInDom(nodeId) {
    const node = String(nodeId || "").trim();
    if (!node.startsWith("scene-panel:")) return true;
    const panel = document.querySelector(
      `[data-manage-tab-panel="preview"] [data-build-node="${cssEscape(node)}"]`,
    );
    if (panel) return true;
    const scopePath = panelScopePathFromNodeId(node);
    if (!scopePath) return false;
    return Boolean(
      document.querySelector(
        `[data-manage-tab-panel="preview"] [data-preview-scope="${cssEscape(scopePath)}"]`,
      ),
    );
  }

  function tier0TargetReady(toUrl) {
    const nodeId = nodeIdFromUrl(toUrl);
    if (!nodeId) return true;
    if (/^board-(?:file|slot):/i.test(nodeId)) {
      const surface = document.querySelector(
        '[data-manage-tab-panel="preview"] .preview-surface, [data-manage-tab-panel="preview"] .preview-stage',
      );
      return Boolean(surface);
    }
    return tier0PanelTargetInDom(nodeId);
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
      if (coordinatesEqual(fromCoord, toCoord)) {
        return "client";
      }
      return "fragment";
    } catch (_) {
      return "full";
    }
  }

  function showBuildNavLoading(url) {
    if (typeof showManageWorkspaceLoadingState === "function") {
      showManageWorkspaceLoadingState(url);
    }
  }

  function clearBuildNavLoading() {
    if (typeof boot.clearManageWorkspaceLoadingState === "function") {
      boot.clearManageWorkspaceLoadingState();
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
      const resolvedTab = tab || inferPreviewTabFromNodeId(node);
      if (resolvedTab) shell.setAttribute("data-build-tab", resolvedTab);
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
    const shell = document.querySelector(".shell[data-build-tab]");
    const current = String(shell?.getAttribute("data-build-tab") || buildTab(global.location.href))
      .trim()
      .toLowerCase();
    if (current === tab) {
      document.querySelectorAll("[data-manage-tab-panel]").forEach((panel) => {
        if (!(panel instanceof HTMLElement)) return;
        const slug = String(panel.getAttribute("data-manage-tab-panel") || "")
          .trim()
          .toLowerCase();
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
