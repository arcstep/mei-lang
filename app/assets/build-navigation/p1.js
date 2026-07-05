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
    const path = String(pathname || "");
    return (
      /^\/apps\/[^/]+\/(?:layout|prototype)(?:\/|$)/.test(path) ||
      /^\/apps\/(?:build|manage)\//.test(path)
    );
  }

  function isLayoutPrototypeWorkspace(pathname) {
    if (typeof isWorkspaceSurfaceRoute === "function") {
      return isWorkspaceSurfaceRoute(pathname);
    }
    return /^\/apps\/[^/]+\/(?:layout|prototype)(?:\/|$)/.test(String(pathname || ""));
  }

  function inferPreviewTabFromNodeId(nodeId) {
    const id = String(nodeId || "").trim();
    if (!id) return "";
    if (/^component:|^template:/i.test(id)) return "preview";
    if (/^(?:scene|route|projection):/i.test(id)) return "preview";
    if (/^board-(?:file|slot):/i.test(id)) return "preview";
    if (/^world-(?:dataset|metric|file):/i.test(id)) return "preview";
    if (/^dataset:/i.test(id)) return "preview";
    if (/^ui-scope:/i.test(id)) return "preview";
    if (/^(?:scene-panel|scene-block):/i.test(id)) return "preview";
    return "";
  }

  function treeLinkTab(rawUrl, linkEl) {
    const nodeId =
      (linkEl && linkEl.getAttribute && linkEl.getAttribute("data-build-node")) ||
      nodeIdFromUrl(rawUrl);
    return inferPreviewTabFromNodeId(nodeId) || currentManageTabFromUrl(rawUrl) || "overview";
  }

  function currentManageTabFromUrl(rawUrl) {
    try {
      const parsed = new URL(rawUrl, global.location.href);
      if (isLayoutPrototypeWorkspace(parsed.pathname)) return "";
      const tab = String(parsed.searchParams.get("tab") || "").trim().toLowerCase();
      if (tab) return tab;
    } catch (_) {}
    const shell = document.querySelector(".shell[data-build-tab]");
    return String(shell?.getAttribute("data-build-tab") || "overview").trim().toLowerCase();
  }

  function buildTab(rawUrl, linkEl) {
    try {
      const parsed = new URL(rawUrl, global.location.href);
      if (isLayoutPrototypeWorkspace(parsed.pathname)) return "";
      const nodeId =
        (linkEl && linkEl.getAttribute && linkEl.getAttribute("data-build-node")) ||
        nodeIdFromUrl(rawUrl);
      const fromNode = inferPreviewTabFromNodeId(nodeId);
      if (fromNode) return fromNode;
      const fromQuery = String(parsed.searchParams.get("tab") || "").trim().toLowerCase();
      if (fromQuery) return fromQuery;
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
    if (/^(?:scene-panel|scene-block|scene|route|ui-scope):/i.test(id)) {
      const scene = sceneIdFromNodeId(id);
      const fromTree = readCompileCoordinateFromReachabilityTree(id);
      if (fromTree) return fromTree;
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

  function readUiScopeMetaFromReachabilityTree(nodeId) {
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
            return {
              ui_role: String(node.ui_role || node.badges?.[0] || "").trim(),
              preview_scope: String(node.preview_scope || "").trim(),
              plane_tier: String(node.plane_tier || "").trim(),
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

  function tier0PanelTargetInDom(nodeId) {
    const node = String(nodeId || "").trim();
    if (node.startsWith("ui-scope:")) {
      const previewRoot = document.querySelector('[data-manage-tab-panel="preview"]');
      if (!previewRoot) return false;
      const target = previewRoot.querySelector(`[data-build-node="${cssEscape(node)}"]`);
      if (target) return true;
      const fromTree = readUiScopeMetaFromReachabilityTree(node);
      const scopePath = String(fromTree?.preview_scope || "").trim();
      if (scopePath) {
        const scoped = previewRoot.querySelector(
          `[data-mei-ui-scope="${cssEscape(scopePath)}"], [data-preview-scope="${cssEscape(scopePath)}"]`,
        );
        if (scoped) return true;
      }
      return Boolean(
        previewRoot.querySelector("[data-mei-ui-scope], [data-mei-tier], [data-preview-scope]"),
      );
    }
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

  function isStructureInspectNode(nodeId) {
    return /^(?:ui-scope|scene-panel|scene-block):/i.test(String(nodeId || "").trim());
  }

  function isSameSceneStructureNav(fromUrl, toUrl) {
    const fromNode = nodeIdFromUrl(fromUrl);
    const toNode = nodeIdFromUrl(toUrl);
    if (!isStructureInspectNode(toNode)) return false;
    const toScene = sceneIdFromNodeId(toNode);
    if (!toScene) return false;
    const fromScene =
      sceneIdFromNodeId(fromNode) ||
      readCompileCoordinateFromShell()?.scene ||
      "";
    return fromScene === toScene;
  }

  function readDataModeFromUrl(rawUrl) {
    try {
      const parsed = new URL(rawUrl, global.location.href);
      const fromQuery = String(parsed.searchParams.get("data_mode") || "").trim();
      if (fromQuery) return fromQuery.toLowerCase();
    } catch (_) {}
    const shell = document.querySelector(".shell");
    return String(shell?.getAttribute("data-data-mode") || "eval")
      .trim()
      .toLowerCase();
  }

  function readReviewProjectionFromUrl(rawUrl) {
    try {
      const parsed = new URL(rawUrl, global.location.href);
      const fromQuery = String(parsed.searchParams.get("review_projection") || "").trim();
      if (fromQuery) return fromQuery.toLowerCase().replace(/-/g, "_");
    } catch (_) {}
    const shell = document.querySelector(".shell");
    const fromShell = shell?.getAttribute("data-review-projection");
    if (fromShell) {
      return String(fromShell).trim().toLowerCase().replace(/-/g, "_");
    }
    return "plane_region_section";
  }

  function reviewAxesChanged(fromUrl, toUrl) {
    const fromDataMode = readDataModeFromUrl(fromUrl);
    const toDataMode = readDataModeFromUrl(toUrl);
    const dataModeChanged = fromDataMode !== toDataMode;
    const fromProjection = readReviewProjectionFromUrl(fromUrl);
    const toProjection = readReviewProjectionFromUrl(toUrl);
    const reviewProjectionChanged = fromProjection !== toProjection;
    return {
      dataModeChanged,
      reviewProjectionChanged,
      changed: dataModeChanged || reviewProjectionChanged,
    };
  }

  function isCrossAppWorkspaceSurfaceNav(from, to) {
    if (typeof isAppWorkspaceSurfaceRoute !== "function") return false;
    if (!isAppWorkspaceSurfaceRoute(from.pathname) || !isAppWorkspaceSurfaceRoute(to.pathname)) {
      return false;
    }
    const fromApp =
      typeof appIdFromAppsPathname === "function" ? appIdFromAppsPathname(from.pathname) : "";
    const toApp =
      typeof appIdFromAppsPathname === "function" ? appIdFromAppsPathname(to.pathname) : "";
    if (!fromApp || fromApp !== toApp) return false;
    const fromParts = from.pathname.split("/").filter(Boolean);
    const toParts = to.pathname.split("/").filter(Boolean);
    const fromSurface = fromParts[2] || "";
    const toSurface = toParts[2] || "";
    return fromSurface !== toSurface;
  }

  function classifyBuildNavTier(fromUrl, toUrl, linkEl) {
    try {
      const from = new URL(fromUrl, global.location.href);
      const to = new URL(toUrl, global.location.href);
      if (isCrossAppWorkspaceSurfaceNav(from, to)) {
        const fromScene =
          typeof sceneIdFromPathname === "function" ? sceneIdFromPathname(from.pathname) : "home";
        const toScene =
          typeof sceneIdFromPathname === "function" ? sceneIdFromPathname(to.pathname) : "home";
        if (fromScene === toScene) return "view_revision";
      }
      if (!isBuildWorkspacePathname(to.pathname)) return "full";
      if (!isBuildWorkspacePathname(from.pathname)) return "full";
      if (from.pathname !== to.pathname) return "full";
      const toNode = nodeIdFromUrl(toUrl);
      const previewTab =
        isLayoutPrototypeWorkspace(to.pathname) ||
        buildTab(toUrl, linkEl) === "preview" ||
        Boolean(inferPreviewTabFromNodeId(toNode));
      if (!previewTab) return "full";
      const axesChange = reviewAxesChanged(fromUrl, toUrl);
      if (axesChange.dataModeChanged) {
        return "fragment";
      }
      if (isSameSceneStructureNav(fromUrl, toUrl)) {
        return "client";
      }
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

  function syncBuildPresetTabs(dataMode, reviewProjection) {
    const dm = String(dataMode || "").trim().toLowerCase();
    const rp = String(reviewProjection || "")
      .trim()
      .toLowerCase()
      .replace(/-/g, "_");
    document.querySelectorAll("a.manage-view-tab--preset[data-build-preset]").forEach((tab) => {
      if (!(tab instanceof HTMLElement)) return;
      const tabDm = String(tab.getAttribute("data-build-data-mode") || "")
        .trim()
        .toLowerCase();
      const tabRp = String(tab.getAttribute("data-build-review-projection") || "")
        .trim()
        .toLowerCase()
        .replace(/-/g, "_");
      const active = tabDm === dm && tabRp === rp;
      tab.classList.toggle("is-active", active);
      tab.setAttribute("aria-selected", active ? "true" : "false");
    });
    const shell = document.querySelector(".shell");
    if (shell) {
      const preset = document.querySelector(
        `a.manage-view-tab--preset.is-active[data-build-preset]`,
      );
      if (preset instanceof HTMLElement) {
        shell.setAttribute("data-build-preset", preset.getAttribute("data-build-preset") || "");
      }
    }
  }

  function syncBuildShellUrl(url, replaceHistory, linkEl) {
    const parsed = new URL(url, global.location.href);
    const shell = document.querySelector(".shell");
    const layoutProto = isLayoutPrototypeWorkspace(parsed.pathname);
    if (shell) {
      const node = String(parsed.searchParams.get("node") || "").trim();
      const focus = String(parsed.searchParams.get("focus") || "").trim();
      let tab = String(parsed.searchParams.get("tab") || "").trim();
      const inferredTab = inferPreviewTabFromNodeId(node);
      if (!layoutProto) {
        if (!tab && inferredTab) {
          tab = inferredTab;
          parsed.searchParams.set("tab", inferredTab);
          url = parsed.href;
        }
      } else {
        parsed.searchParams.delete("tab");
        url = parsed.href;
      }
      if (node) shell.setAttribute("data-build-node", node);
      else shell.removeAttribute("data-build-node");
      if (focus) shell.setAttribute("data-build-focus", focus);
      else shell.removeAttribute("data-build-focus");
      const dataMode = String(parsed.searchParams.get("data_mode") || "").trim();
      const reviewProjection = String(
        parsed.searchParams.get("review_projection") || "",
      ).trim();
      if (dataMode) shell.setAttribute("data-data-mode", dataMode);
      if (reviewProjection) {
        shell.setAttribute("data-review-projection", reviewProjection);
      } else if (isBuildWorkspacePathname(parsed.pathname)) {
        const surface = parsed.pathname.match(/\/apps\/[^/]+\/([^/]+)/)?.[1];
        shell.setAttribute(
          "data-review-projection",
          surface === "prototype" ? "static_full" : "plane_region_section",
        );
      }
      if (layoutProto) {
        shell.removeAttribute("data-build-tab");
      } else {
        const resolvedTab = tab || inferredTab;
        if (resolvedTab) shell.setAttribute("data-build-tab", resolvedTab);
      }
      const coord = readCompileCoordinate(url, linkEl);
      if (coord) {
        shell.setAttribute("data-compile-scene", coord.scene);
        shell.setAttribute("data-compile-target", coord.target);
      }
      syncBuildPresetTabs(
        dataMode || shell.getAttribute("data-data-mode"),
        reviewProjection || shell.getAttribute("data-review-projection"),
      );
    }
    if (replaceHistory) global.history.replaceState({}, "", url);
    else global.history.pushState({}, "", url);
    lastBuildNavUrl = url;
  }

  function ensurePreviewTabVisible(rawUrl, linkEl, options) {
    const opts = options && typeof options === "object" ? options : {};
    let tab = buildTab(rawUrl, linkEl);
    let layoutProto = false;
    try {
      layoutProto = isLayoutPrototypeWorkspace(new URL(rawUrl, global.location.href).pathname);
      if (!tab && layoutProto) tab = "preview";
    } catch (_) {}
    const shell = document.querySelector(".shell[data-build-tab]");
    const shellTab = String(shell?.getAttribute("data-build-tab") || "").trim().toLowerCase();
    const current =
      layoutProto && !shellTab
        ? "preview"
        : String(shellTab || buildTab(global.location.href, linkEl))
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
    if (typeof boot.parseViewContext === "function") {
      const ctx = boot.parseViewContext(url);
      if (ctx) {
        const surface = ctx.surface || ctx.mode || "build";
        const tab =
          ctx.tab ||
          (String(surface).trim().toLowerCase() === "build" ? "preview" : "");
        return {
          app_id: ctx.app_id || ctx.appId,
          scene_id: ctx.scene_id || ctx.sceneId,
          surface,
          node: ctx.node || "",
          data_mode: ctx.data_mode || ctx.dataMode || "",
          review_projection: ctx.review_projection || ctx.reviewProjection || "",
          focus: ctx.focus || "",
          scope: ctx.scope || "",
          tab,
        };
      }
    }
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
    const urlTab = parsed.searchParams.get("tab") || "";
    return {
      app_id: appId,
      scene_id: parsed.searchParams.get("scene") || "home",
      surface,
      node,
      data_mode: parsed.searchParams.get("data_mode") || "",
      review_projection: parsed.searchParams.get("review_projection") || "",
      focus: parsed.searchParams.get("focus") || "",
      scope: parsed.searchParams.get("scope") || "",
      tab: urlTab || (surface === "build" ? "preview" : ""),
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

  async function assembleBuildFromManifestPayload(url, payload) {
    if (!payload?.scene_manifest) return false;
    const ctx = viewRevisionCtxFromUrl(url);
    if (boot.viewRevisionClient?.tryAssembleLocal) {
      const assemble = await boot.viewRevisionClient.tryAssembleLocal(ctx, {
        manifest: payload.scene_manifest,
        compose_defaults: payload.compose_defaults,
      });
      if (assemble?.ok) {
        return true;
      }
    }
    if (!boot.sceneManifestLoader || !boot.viewCompositor) {
      return false;
    }
    const parsed = new URL(url, global.location.href);
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
    const batch = await boot.sceneManifestLoader.fetchLayerBatch(
      appId,
      sceneId,
      ["structure.full"],
      boot.sceneManifestLoader.readShellAxes(),
    );
    const structure = batch?.layers?.["structure.full"];
    if (!structure) return false;
    const projection =
      parsed.searchParams.get("review_projection") ||
      payload.compose_defaults?.review_projection ||
      "live_full";
    const root =
      typeof boot.resolveComposeRoot === "function"
        ? boot.resolveComposeRoot(ctx.surface || "build")
        : document.querySelector(".preview-pane-scroll, .shell");
    if (!root) return false;
    boot.viewCompositor.composePreview(root, structure, projection, null, null);
    return true;
  }

  async function fetchWorkspaceFragment(url, options) {
    const opts = options || {};
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
    });
    const fragmentTab = buildTab(url);
    if (fragmentTab) params.set("tab", fragmentTab);
    else if (
      typeof workspaceSurfaceSlugFromAppsPathname !== "function" ||
      workspaceSurfaceSlugFromAppsPathname(parsed.pathname) === "build"
    ) {
      params.set("tab", "preview");
    }
    const focus = parsed.searchParams.get("focus");
    if (focus) params.set("focus", focus);
    const scope = parsed.searchParams.get("scope");
    if (scope) params.set("scope", scope);
    const reviewProjection = parsed.searchParams.get("review_projection");
    if (reviewProjection) params.set("review_projection", reviewProjection);
    const dataMode = parsed.searchParams.get("data_mode");
    if (dataMode) params.set("data_mode", dataMode);
    const wsSurface =
      typeof workspaceSurfaceSlugFromAppsPathname === "function"
        ? workspaceSurfaceSlugFromAppsPathname(parsed.pathname)
        : "";
    if (wsSurface) params.set("surface", wsSurface);
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

  function rememberBuildRevision(url, revision) {
    const host = hostBoot();
    const fn =
      host.rememberBuildFragmentRevision ||
      global.MeiBuildFragmentRevision?.rememberBuildFragmentRevision;
    if (typeof fn === "function") fn.call(host, url, revision);
  }

  function readBuildFragmentCache(url, revision) {
    const host = hostBoot();
    const fn =
      host.readBuildFragmentManifest || global.MeiBuildFragmentRevision?.readBuildFragmentManifest;
    return typeof fn === "function" ? fn.call(host, url, revision) : null;
  }

  function rememberBuildFragment(url, revision, payload) {
    const host = hostBoot();
    const fn =
      host.rememberBuildFragmentManifest ||
      global.MeiBuildFragmentRevision?.rememberBuildFragmentManifest;
    if (typeof fn === "function") fn.call(host, url, revision, payload);
  }

  function buildRevisionStillValid(url, revision) {
    const host = hostBoot();
    const fn =
      host.buildFragmentRevisionStillValid ||
      global.MeiBuildFragmentRevision?.buildFragmentRevisionStillValid;
    return typeof fn === "function" ? fn.call(host, url, revision) : false;
  }

  async function persistBuildPreviewSnapshot() {
    return false;
  }

  function scheduleEagerBuildPreviewPersist() {}

  function scheduleBuildPreviewPersist() {}

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
      if (cached?.scene_manifest) {
        const ok = await assembleBuildFromManifestPayload(url, cached);
        if (ok) {
          ensurePreviewTabVisible(url, null, { emit: false });
          wakePreviewRuntime("build-manifest-cache", { fromCache: true });
          global.__meiBuildPreviewRestoredFromCache = 1;
          return { restored: true, source: "assemble_local", revision };
        }
      }
      rememberBuildRevision(url, revision);
      if (viewRevisionOutcome?.source === "local_miss") {
        return { restored: false, source: "local_miss", revision };
      }
      return { restored: false, source: "manifest-miss", revision };
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
            if (cached?.scene_manifest) {
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
        if (payload?.scene_manifest) {
          rememberBuildFragment(url, payload.revision, payload);
        }
      }
      if (payload?.scene_manifest) {
        const ok = await assembleBuildFromManifestPayload(url, payload);
        if (!ok) return false;
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
        wakePreviewRuntime("build-manifest", {
          resetRuntimeQueryCache: axesChange.dataModeChanged || isPackCatalogNodeId(nextNode),
          pulsePreviewUpdated: true,
        });
        return true;
      }
      return false;
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

    if (tier === "view_revision") {
      buildNavInFlight = true;
      try {
        syncBuildShellUrl(toUrl, !!opts.replaceHistory, opts.linkEl);
        const assembled = await tryBuildViewRevisionAssemble(toUrl);
        if (assembled?.restored) {
          stats.tier0 += 1;
          runTier0PostNav(fromUrl);
          return { handled: true, tier: 0.5 };
        }
      } catch (err) {
        console.warn("[build-navigation] cross-surface view-revision failed", err);
      } finally {
        buildNavInFlight = false;
      }
      stats.tier2 += 1;
      return { handled: false, tier: 2 };
    }

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
