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
      const tab = String(parsed.searchParams.get("tab") || "").trim().toLowerCase();
      if (tab) return tab;
    } catch (_) {}
    const shell = document.querySelector(".shell[data-build-tab]");
    return String(shell?.getAttribute("data-build-tab") || "overview").trim().toLowerCase();
  }

  function buildTab(rawUrl, linkEl) {
    try {
      const nodeId =
        (linkEl && linkEl.getAttribute && linkEl.getAttribute("data-build-node")) ||
        nodeIdFromUrl(rawUrl);
      const fromNode = inferPreviewTabFromNodeId(nodeId);
      if (fromNode) return fromNode;
      const parsed = new URL(rawUrl, global.location.href);
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

  function classifyBuildNavTier(fromUrl, toUrl, linkEl) {
    try {
      const from = new URL(fromUrl, global.location.href);
      const to = new URL(toUrl, global.location.href);
      if (!isBuildWorkspacePathname(to.pathname)) return "full";
      if (!isBuildWorkspacePathname(from.pathname)) return "full";
      if (from.pathname !== to.pathname) return "full";
      const toNode = nodeIdFromUrl(toUrl);
      const previewTab =
        buildTab(toUrl, linkEl) === "preview" || Boolean(inferPreviewTabFromNodeId(toNode));
      if (!previewTab) return "full";
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

  function syncBuildShellUrl(url, replaceHistory, linkEl) {
    const parsed = new URL(url, global.location.href);
    const shell = document.querySelector(".shell");
    if (shell) {
      const node = String(parsed.searchParams.get("node") || "").trim();
      const focus = String(parsed.searchParams.get("focus") || "").trim();
      let tab = String(parsed.searchParams.get("tab") || "").trim();
      const inferredTab = inferPreviewTabFromNodeId(node);
      if (!tab && inferredTab) {
        tab = inferredTab;
        parsed.searchParams.set("tab", inferredTab);
        url = parsed.href;
      }
      if (node) shell.setAttribute("data-build-node", node);
      else shell.removeAttribute("data-build-node");
      if (focus) shell.setAttribute("data-build-focus", focus);
      else shell.removeAttribute("data-build-focus");
      const resolvedTab = tab || inferredTab;
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

  function ensurePreviewTabVisible(rawUrl, linkEl, options) {
    const opts = options && typeof options === "object" ? options : {};
    const tab = buildTab(rawUrl, linkEl);
    const shell = document.querySelector(".shell[data-build-tab]");
    const current = String(
      shell?.getAttribute("data-build-tab") || buildTab(global.location.href, linkEl),
    )
      .trim()
      .toLowerCase();
    if (current === tab) {
      document.querySelectorAll("[data-manage-tab-panel]").forEach((panel) => {
        if (!(panel instanceof HTMLElement)) return;
        const slug = String(panel.getAttribute("data-manage-tab-panel") || "")
          .trim()
          .toLowerCase();
