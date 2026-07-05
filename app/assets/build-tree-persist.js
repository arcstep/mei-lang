/**
 * Build view: persist left-tree open state + click/double-click expand shortcuts.
 */
(function (global) {
  "use strict";

  const CLICK_DELAY_MS = 280;

  function appIdFromPath() {
    try {
      const parts = String(global.location.pathname || "").split("/").filter(Boolean);
      if (parts[0] !== "apps" || parts.length < 2) return "";
      const slug = String(parts[1] || "").trim().toLowerCase();
      if (slug === "layout" || slug === "prototype" || slug === "app") {
        return parts[2] ? decodeURIComponent(parts[2]) : "";
      }
      return decodeURIComponent(parts[1]);
    } catch {
      return "";
    }
  }

  function storageKey(suffix) {
    const appId = appIdFromPath() || String(global.document?.body?.getAttribute?.("data-app-id") || "").trim() || "_";
    return `mei-workspace-tree:${appId}:${suffix}`;
  }

  const UI_ROLE_RANK = {
    plane: 0,
    region: 1,
    section: 2,
    content: 3,
    micro_layout: 2,
  };

  function isBuildRoute() {
    const path = String(global.location.pathname || "");
    if (/^\/apps\/[^/]+\/(?:layout|prototype)(?:\/|$)/.test(path)) {
      return true;
    }
    if (/^\/apps\/(?:build|manage)\//.test(path)) {
      return true;
    }
    try {
      const boot = global.__meiLangBoot;
      if (typeof boot?.parseViewContext === "function") {
        const ctx = boot.parseViewContext(global.location.href);
        const surface = String(ctx?.surface || ctx?.mode || "").trim().toLowerCase();
        return surface === "layout" || surface === "prototype";
      }
      if (typeof isWorkspaceSurfaceRoute === "function" && isWorkspaceSurfaceRoute(path)) {
        return true;
      }
    } catch (_) {}
    return false;
  }

  function sidebarScrollEl(root) {
    return root?.closest?.(".sidebar-scroll") || null;
  }

  function loadOpenSet() {
    try {
      const raw = global.sessionStorage.getItem(storageKey("open"));
      if (!raw) return new Set();
      const parsed = JSON.parse(raw);
      return new Set(Array.isArray(parsed) ? parsed : []);
    } catch {
      return new Set();
    }
  }

  function saveOpenSet(set) {
    try {
      global.sessionStorage.setItem(storageKey("open"), JSON.stringify([...set]));
    } catch {
      /* ignore quota */
    }
  }

  function captureScroll(root) {
    const scroll = sidebarScrollEl(root);
    if (!scroll) return;
    try {
      global.sessionStorage.setItem(storageKey("scroll"), String(scroll.scrollTop || 0));
    } catch {
      /* ignore */
    }
  }

  function restoreScroll(root, explicitTop) {
    const scroll = sidebarScrollEl(root);
    if (!scroll) return;
    try {
      if (typeof explicitTop === "number" && Number.isFinite(explicitTop)) {
        scroll.scrollTop = explicitTop;
        return;
      }
      const raw = global.sessionStorage.getItem(storageKey("scroll"));
      if (raw == null) return;
      const top = Number(raw);
      if (Number.isFinite(top)) {
        scroll.scrollTop = top;
      }
    } catch {
      /* ignore */
    }
  }

  function pinSidebarScroll(root, mutate) {
    const scroll = sidebarScrollEl(root);
    const pinnedTop = scroll?.scrollTop ?? null;
    mutate();
    if (!scroll || pinnedTop == null) return;
    const apply = () => {
      scroll.scrollTop = pinnedTop;
    };
    requestAnimationFrame(() => {
      apply();
      requestAnimationFrame(apply);
    });
  }

  function branchId(details) {
    return String(details?.getAttribute("data-build-tree-branch") || "").trim();
  }

  function branchDetailsFromSummary(summary) {
    if (!(summary instanceof HTMLElement)) return null;
    const details = summary.parentElement;
    if (!(details instanceof HTMLDetailsElement)) return null;
    if (!details.matches("details.build-tree-details[data-build-tree-branch]")) return null;
    return details;
  }

  function isNavigationClick(event) {
    const target = event.target;
    if (!(target instanceof Element)) return false;
    return Boolean(target.closest("a[href]"));
  }

  function nestedBranchDetails(details) {
    return Array.from(
      details.querySelectorAll(":scope > ul .build-tree-details[data-build-tree-branch]"),
    ).filter((node) => node instanceof HTMLDetailsElement);
  }

  function allDescendantBranchDetails(details) {
    return Array.from(
      details.querySelectorAll(".build-tree-details[data-build-tree-branch]"),
    ).filter((node) => node instanceof HTMLDetailsElement);
  }

  function syncOpenSetForDetails(details, openSet, open) {
    const id = branchId(details);
    if (!id) return;
    if (open) {
      openSet.add(id);
    } else {
      openSet.delete(id);
    }
  }

  function syncOpenSetForSubtree(details, openSet, open) {
    syncOpenSetForDetails(details, openSet, open);
    for (const nested of allDescendantBranchDetails(details)) {
      syncOpenSetForDetails(nested, openSet, open);
    }
  }

  function expandOneLevel(details, openSet) {
    details.open = true;
    syncOpenSetForDetails(details, openSet, true);
  }

  function expandAllDescendants(details, openSet) {
    details.open = true;
    for (const nested of allDescendantBranchDetails(details)) {
      nested.open = true;
    }
    syncOpenSetForSubtree(details, openSet, true);
  }

  function collapseAllDescendants(details, openSet) {
    for (const nested of allDescendantBranchDetails(details)) {
      nested.open = false;
    }
    details.open = false;
    syncOpenSetForSubtree(details, openSet, false);
  }

  function bindTreeExpandBehavior(root) {
    if (!root || root.__buildTreeExpandBound) return;
    root.__buildTreeExpandBound = true;
    const clickTimers = new WeakMap();

    root.addEventListener(
      "click",
      (event) => {
        if (isNavigationClick(event)) return;
        const summary = event.target.closest("summary.build-tree-summary");
        const details = branchDetailsFromSummary(summary);
        if (!details) return;
        event.preventDefault();

        const existing = clickTimers.get(details);
        if (existing) {
          global.clearTimeout(existing);
          clickTimers.delete(details);
          const openSet = loadOpenSet();
          expandAllDescendants(details, openSet);
          saveOpenSet(openSet);
          return;
        }

        const timer = global.setTimeout(() => {
          clickTimers.delete(details);
          const openSet = loadOpenSet();
          if (details.open) {
            collapseAllDescendants(details, openSet);
          } else {
            expandOneLevel(details, openSet);
            for (const nested of nestedBranchDetails(details)) {
              nested.open = false;
              syncOpenSetForDetails(nested, openSet, false);
            }
          }
          saveOpenSet(openSet);
        }, CLICK_DELAY_MS);
        clickTimers.set(details, timer);
      },
      true,
    );
  }

  function uiRoleRank(role) {
    const key = String(role || "").trim().toLowerCase();
    if (!key) return -1;
    return Object.prototype.hasOwnProperty.call(UI_ROLE_RANK, key) ? UI_ROLE_RANK[key] : 99;
  }

  function readTreeMaxUiRole(root) {
    const fromTree = String(root?.getAttribute("data-build-tree-max-ui-role") || "").trim();
    if (fromTree) return fromTree;
    const shell = document.querySelector(".shell[data-build-tree-max-ui-role]");
    return String(shell?.getAttribute("data-build-tree-max-ui-role") || "content").trim() || "content";
  }

  function readActivePresetSlug() {
    const shell = document.querySelector(".shell[data-build-preset]");
    return String(shell?.getAttribute("data-build-preset") || "").trim();
  }

  function presetChanged() {
    const preset = readActivePresetSlug();
    if (!preset) return false;
    try {
      const prev = String(global.sessionStorage.getItem(storageKey("preset")) || "").trim();
      if (prev === preset) return false;
      global.sessionStorage.setItem(storageKey("preset"), preset);
      return true;
    } catch {
      return false;
    }
  }

  function applyPresetDefaultExpand(root, openSet) {
    const maxRole = readTreeMaxUiRole(root);
    const maxRank = uiRoleRank(maxRole);
    root.querySelectorAll("details.build-tree-details[data-build-tree-branch]").forEach((details) => {
      const role = String(details.getAttribute("data-ui-role") || "").trim();
      const rank = uiRoleRank(role);
      const shouldOpen = rank >= 0 && rank <= maxRank;
      details.open = shouldOpen;
      syncOpenSetForDetails(details, openSet, shouldOpen);
    });
  }

  function restoreOpenState(root) {
    const openSet = loadOpenSet();
    root.querySelectorAll("details.build-tree-details[data-build-tree-branch]").forEach((details) => {
      const id = branchId(details);
      if (id && openSet.has(id)) {
        details.open = true;
      }
    });
  }

  function ensureActivePathOpen(root) {
    const active =
      root.querySelector(".build-tree-link--active") ||
      root.querySelector(".build-tree-summary--active");
    if (!active) return;
    const openSet = loadOpenSet();
    let node = active.closest("li.build-tree-node");
    while (node) {
      const details = node.querySelector(":scope > details.build-tree-details[data-build-tree-branch]");
      if (details) {
        const id = branchId(details);
        if (id) {
          details.open = true;
          openSet.add(id);
        }
      }
      const parentList = node.parentElement?.closest("li.build-tree-node");
      node = parentList || null;
    }
    saveOpenSet(openSet);
  }

  function bindTreePersist(root) {
    if (!root || root.__buildTreePersistBound) return;
    root.__buildTreePersistBound = true;
    const openSet = loadOpenSet();
    if (openSet.size === 0) {
      applyPresetDefaultExpand(root, openSet);
      saveOpenSet(openSet);
    } else {
      restoreOpenState(root);
    }
    ensureActivePathOpen(root);
    restoreScroll(root);
    bindTreeExpandBehavior(root);
    root.addEventListener(
      "toggle",
      (event) => {
        const details = event.target;
        if (!details?.matches?.("details.build-tree-details[data-build-tree-branch]")) return;
        const id = branchId(details);
        if (!id) return;
        const openSet = loadOpenSet();
        if (details.open) {
          openSet.add(id);
        } else {
          openSet.delete(id);
        }
        saveOpenSet(openSet);
      },
      true,
    );
  }

  function currentManageTab() {
    try {
      const url = new URL(global.location.href);
      const tab = String(url.searchParams.get("tab") || "").trim().toLowerCase();
      if (tab) return tab;
    } catch (_) {}
    const shell = document.querySelector(".shell[data-build-tab]");
    return String(shell?.getAttribute("data-build-tab") || "overview").trim().toLowerCase();
  }

  function inferPreviewTabFromNodeId(nodeId) {
    const id = String(nodeId || "").trim().toLowerCase();
    if (!id) return "";
    if (id.startsWith("scene-panel:") || id.startsWith("scene-block:") || id.includes("projection")) {
      return "preview";
    }
    return "";
  }

  function treeLinkTab(rawUrl, linkEl) {
    const nodeId = String(linkEl?.getAttribute?.("data-build-node") || "").trim();
    const inferred = inferPreviewTabFromNodeId(nodeId);
    if (inferred) return inferred;
    return currentManageTab();
  }

  function isWorkspaceSurfaceRoute() {
    const path = String(global.location.pathname || "");
    if (/^\/apps\/[^/]+\/(?:layout|prototype)(?:\/|$)/.test(path)) {
      return true;
    }
    try {
      const boot = global.__meiLangBoot;
      if (typeof boot?.parseViewContext === "function") {
        const ctx = boot.parseViewContext(global.location.href);
        const surface = String(ctx?.surface || ctx?.mode || "").trim().toLowerCase();
        return surface === "layout" || surface === "prototype";
      }
    } catch (_) {}
    return false;
  }

  function workspaceSurfaceFromDom() {
    const fromBody = String(global.document?.body?.getAttribute("data-surface") || "")
      .trim()
      .toLowerCase();
    if (fromBody === "layout" || fromBody === "prototype") return fromBody;
    try {
      const boot = global.__meiLangBoot;
      if (typeof boot?.parseViewContext === "function") {
        const ctx = boot.parseViewContext(global.location.href);
        const surface = String(ctx?.surface || ctx?.mode || "").trim().toLowerCase();
        if (surface === "layout" || surface === "prototype") return surface;
      }
    } catch (_) {}
    try {
      const surface = String(new URL(global.location.href).searchParams.get("surface") || "layout")
        .trim()
        .toLowerCase();
      return surface === "prototype" ? "prototype" : "layout";
    } catch (_) {
      return "layout";
    }
  }

  function syncTreeLinkTabs(root) {
    const onUnifiedWorkspace =
      typeof isUnifiedViewRoute === "function" &&
      isUnifiedViewRoute(global.location.pathname) &&
      isWorkspaceSurfaceRoute();
    root.querySelectorAll("a.build-tree-link, a.build-tree-label--link").forEach((link) => {
      try {
        const nodeId = String(link.getAttribute("data-build-node") || "").trim();
        const url = onUnifiedWorkspace
          ? new URL(global.location.href)
          : new URL(link.href, global.location.href);
        if (onUnifiedWorkspace) {
          url.searchParams.set("surface", workspaceSurfaceFromDom());
          if (nodeId) {
            url.searchParams.set("node", nodeId);
            url.searchParams.delete("focus");
          }
        }
        const tab = treeLinkTab(url.toString(), link);
        url.searchParams.set("tab", tab);
        link.href = url.toString();
      } catch (_) {}
    });
  }

  function bindTreeTabPersist(root) {
    if (root.__buildTreeTabBound) return;
    root.__buildTreeTabBound = true;
    root.addEventListener(
      "click",
      (event) => {
        const link = event.target.closest("a.build-tree-link, a.build-tree-label--link");
        if (!link || !link.href) return;
        captureScroll(root);
        if (isWorkspaceSurfaceRoute()) {
          event.preventDefault();
          event.stopImmediatePropagation();
          const nodeId = String(link.getAttribute("data-build-node") || "").trim();
          if (nodeId && global.MeiBuildInspectHighlight?.selectBuildNodeClient) {
            global.MeiBuildInspectHighlight.selectBuildNodeClient(nodeId);
          } else if (nodeId) {
            const shell = document.querySelector(".shell[data-build-node]");
            if (shell) shell.setAttribute("data-build-node", nodeId);
            syncTreeActiveFromNode(root, nodeId);
            if (global.MeiBuildInspectHighlight?.refresh) {
              global.MeiBuildInspectHighlight.refresh();
            }
          }
          return;
        }
        try {
          const url = new URL(link.href, global.location.href);
          url.searchParams.set("tab", treeLinkTab(url.toString(), link));
          url.searchParams.delete("focus");
          link.href = url.toString();
        } catch (_) {}
        const nodeId = String(link.getAttribute("data-build-node") || "").trim();
        if (typeof global.__meiLangBoot?.navigateInternal === "function") {
          event.preventDefault();
          event.stopImmediatePropagation();
          void global.__meiLangBoot.navigateInternal(link.href, false);
        } else if (typeof global.__meiLangBoot?.navigateSpa === "function") {
          event.preventDefault();
          event.stopImmediatePropagation();
          void global.__meiLangBoot.navigateSpa(link.href, false);
        }
      },
      true,
    );
    root.addEventListener(
      "mousedown",
      (event) => {
        const link = event.target.closest("a.build-tree-link, a.build-tree-label--link");
        if (!link) return;
        // Prevent focus scroll-into-view jitter when SPA handles navigation.
        event.preventDefault();
      },
      true,
    );
  }

  function readTreeMode(root) {
    const fromTree = String(root?.getAttribute("data-build-tree-mode-active") || "").trim().toLowerCase();
    if (fromTree === "compile" || fromTree === "structure") return fromTree;
    const shell = document.querySelector(".shell[data-build-tree-mode]");
    const fromShell = String(shell?.getAttribute("data-build-tree-mode") || "").trim().toLowerCase();
    return fromShell === "compile" ? "compile" : "structure";
  }

  function applyTreeMode(root, mode) {
    const activeMode = mode === "compile" ? "compile" : "structure";
    root.setAttribute("data-build-tree-mode-active", activeMode);
    root.querySelectorAll("[data-build-tree-root-group]").forEach((details) => {
      const group = String(details.getAttribute("data-build-tree-root-group") || "");
      const branch = details.closest(".build-tree-node");
      if (!(branch instanceof HTMLElement)) return;
      if (activeMode === "structure") {
        branch.hidden = group !== "ui_structure";
      } else {
        branch.hidden = ![
          "mcg",
          "scenes",
          "routes",
          "world",
          "datasets",
          "artifacts",
        ].includes(group);
      }
    });
  }

  function refresh(options) {
    if (!isBuildRoute()) return;
    const roots = document.querySelectorAll(
      "aside .build-reachability-tree, .build-tree-shell .build-reachability-tree, nav.build-reachability-tree, .build-reachability-tree",
    );
    if (!roots.length) return;
    if (presetChanged()) {
      const openSet = new Set();
      applyPresetDefaultExpand(roots[0], openSet);
      saveOpenSet(openSet);
    }
    roots.forEach((root) => {
      if (!(root instanceof HTMLElement)) return;
      bindTreePersist(root);
      bindTreeTabPersist(root);
      applyTreeMode(root, readTreeMode(root));
      syncTreeLinkTabs(root);
      pinSidebarScroll(root, () => {
        syncTreeActiveFromNode(root, options?.activeNode);
      });
      if (options && options.restorePersistedScroll) {
        restoreScroll(root);
      }
    });
  }

  function installBuildTreeSpaNavigation() {
    if (global.document?.documentElement?.dataset?.meiBuildTreeSpaBound === "1") return;
    if (global.document?.documentElement) {
      global.document.documentElement.dataset.meiBuildTreeSpaBound = "1";
    }
    global.document.addEventListener(
      "click",
      (event) => {
        const link = event.target.closest?.("a.build-tree-link, a.build-tree-label--link");
        if (!link || !link.href) return;
        if (isWorkspaceSurfaceRoute()) {
          event.preventDefault();
          event.stopImmediatePropagation();
          const nodeId = String(link.getAttribute("data-build-node") || "").trim();
          if (nodeId && global.MeiBuildInspectHighlight?.selectBuildNodeClient) {
            global.MeiBuildInspectHighlight.selectBuildNodeClient(nodeId);
          } else if (nodeId) {
            const shell = document.querySelector(".shell[data-build-node]");
            if (shell) shell.setAttribute("data-build-node", nodeId);
            const root = link.closest(".build-reachability-tree");
            if (root) syncTreeActiveFromNode(root);
          }
          return;
        }
        if (
          !(
            typeof isUnifiedViewRoute === "function" &&
            isUnifiedViewRoute(global.location.pathname)
          ) &&
          isBuildRoute() &&
          typeof global.__meiLangBoot?.navigateInternal === "function"
        ) {
          event.preventDefault();
          event.stopImmediatePropagation();
          void global.__meiLangBoot.navigateInternal(link.href, false);
        }
      },
      true,
    );
  }

  function syncTreeActiveFromNode(root, nodeIdOverride) {
    let nodeId = String(nodeIdOverride || "").trim();
    if (!nodeId) {
      try {
        nodeId = String(new URL(global.location.href).searchParams.get("node") || "").trim();
      } catch (_) {}
    }
    if (!nodeId) {
      nodeId = String(document.querySelector(".shell[data-build-node]")?.getAttribute("data-build-node") || "").trim();
    }
    root.querySelectorAll(".build-tree-link--active").forEach((el) => {
      el.classList.remove("build-tree-link--active");
    });
    root.querySelectorAll(".build-tree-summary--active").forEach((el) => {
      el.classList.remove("build-tree-summary--active");
    });
    if (!nodeId) return;
    const escaped =
      typeof global.CSS !== "undefined" && typeof global.CSS.escape === "function"
        ? global.CSS.escape(nodeId)
        : nodeId.replace(/\\/g, "\\\\").replace(/"/g, '\\"');
    const activeLink = root.querySelector(`a[data-build-node="${escaped}"]`);
    if (!(activeLink instanceof HTMLElement)) return;
    activeLink.classList.add("build-tree-link--active");
    const summary = activeLink.closest("summary");
    if (summary instanceof HTMLElement) {
      summary.classList.add("build-tree-summary--active");
    }
    ensureActivePathOpen(root);
  }

  function bind() {
    installBuildTreeSpaNavigation();
    if (!isBuildRoute()) return;
    refresh({ restorePersistedScroll: true });
    global.addEventListener("popstate", refresh);
    global.addEventListener("mei:manage-tab-change", refresh);
    global.addEventListener("mei:spa-navigation-complete", () => refresh());
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", bind);
  } else {
    bind();
  }

  global.MeiBuildTreePersist = { refresh, captureScroll };
})(window);
