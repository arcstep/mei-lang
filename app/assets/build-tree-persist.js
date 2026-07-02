/**
 * Build view: persist left-tree open state + click/double-click expand shortcuts.
 */
(function (global) {
  "use strict";

  const STORAGE_KEY = "mei-build-tree-open";
  const SCROLL_KEY = "mei-build-tree-scroll";
  const MODE_KEY = "mei-build-tree-mode";
  const CLICK_DELAY_MS = 280;

  function isBuildRoute() {
    return /^\/apps\/(?:build|manage)\//.test(String(global.location.pathname || ""));
  }

  function sidebarScrollEl(root) {
    return root?.closest?.(".sidebar-scroll") || null;
  }

  function loadOpenSet() {
    try {
      const raw = global.sessionStorage.getItem(STORAGE_KEY);
      if (!raw) return new Set();
      const parsed = JSON.parse(raw);
      return new Set(Array.isArray(parsed) ? parsed : []);
    } catch {
      return new Set();
    }
  }

  function saveOpenSet(set) {
    try {
      global.sessionStorage.setItem(STORAGE_KEY, JSON.stringify([...set]));
    } catch {
      /* ignore quota */
    }
  }

  function captureScroll(root) {
    const scroll = sidebarScrollEl(root);
    if (!scroll) return;
    try {
      global.sessionStorage.setItem(SCROLL_KEY, String(scroll.scrollTop || 0));
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
      const raw = global.sessionStorage.getItem(SCROLL_KEY);
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
    restoreOpenState(root);
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

  function treeLinkTab(rawUrl, linkEl) {
    const nav = global.MeiBuildNavigation;
    if (nav && typeof nav.treeLinkTab === "function") {
      return nav.treeLinkTab(rawUrl, linkEl);
    }
    if (nav && typeof nav.inferPreviewTabFromNodeId === "function") {
      const nodeId = String(linkEl?.getAttribute?.("data-build-node") || "").trim();
      const inferred = nav.inferPreviewTabFromNodeId(nodeId);
      if (inferred) return inferred;
    }
    return currentManageTab();
  }

  function syncTreeLinkTabs(root) {
    root.querySelectorAll("a.build-tree-link, a.build-tree-label--link").forEach((link) => {
      try {
        const url = new URL(link.href, global.location.href);
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
        try {
          const url = new URL(link.href, global.location.href);
          url.searchParams.set("tab", treeLinkTab(url.toString(), link));
          url.searchParams.delete("focus");
          link.href = url.toString();
        } catch (_) {}
        const nodeId = String(link.getAttribute("data-build-node") || "").trim();
        const structureNode = /^(?:ui-scope|scene-panel|scene-block):/i.test(nodeId);
        const nav = global.MeiBuildNavigation;
        if (!structureNode || typeof nav?.tryNavigateBuild !== "function") return;
        event.preventDefault();
        event.stopImmediatePropagation();
        void nav
          .tryNavigateBuild(global.location.href, link.href, { linkEl: link })
          .then((result) => {
            if (result?.handled) {
              if (typeof nav.noteUrl === "function") nav.noteUrl(link.href);
              return;
            }
            if (typeof global.__meiLangBoot?.navigateInternal === "function") {
              void global.__meiLangBoot.navigateInternal(link.href, false, { skipBuildNav: true });
            }
          })
          .catch(() => {
            if (typeof global.__meiLangBoot?.navigateInternal === "function") {
              void global.__meiLangBoot.navigateInternal(link.href, false, { skipBuildNav: true });
            }
          });
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

  function loadTreeMode() {
    try {
      const raw = String(global.sessionStorage.getItem(MODE_KEY) || "").trim().toLowerCase();
      return raw === "compile" ? "compile" : "structure";
    } catch {
      return "structure";
    }
  }

  function saveTreeMode(mode) {
    try {
      global.sessionStorage.setItem(MODE_KEY, mode);
    } catch {
      /* ignore */
    }
  }

  function applyTreeMode(root, mode) {
    const shell = root.closest(".build-tree-shell") || root.parentElement;
    const activeMode = mode === "compile" ? "compile" : "structure";
    root.setAttribute("data-build-tree-mode-active", activeMode);
    shell?.querySelectorAll(".build-tree-mode-btn").forEach((btn) => {
      const btnMode = String(btn.getAttribute("data-build-tree-mode") || "");
      btn.classList.toggle("is-active", btnMode === activeMode);
    });
    root.querySelectorAll("[data-build-tree-root-group]").forEach((details) => {
      const group = String(details.getAttribute("data-build-tree-root-group") || "");
      const branch = details.closest(".build-tree-node");
      if (!(branch instanceof HTMLElement)) return;
      if (activeMode === "structure") {
        branch.hidden = group !== "ui_structure";
      } else {
        branch.hidden = group === "ui_structure";
      }
    });
  }

  function bindTreeModeToggle(root) {
    const shell = root.closest(".build-tree-shell");
    if (!shell || shell.__buildTreeModeBound) return;
    shell.__buildTreeModeBound = true;
    shell.querySelectorAll(".build-tree-mode-btn").forEach((btn) => {
      btn.addEventListener("click", () => {
        const mode = String(btn.getAttribute("data-build-tree-mode") || "structure");
        saveTreeMode(mode);
        applyTreeMode(root, mode);
      });
    });
  }

  function refresh(options) {
    if (!isBuildRoute()) return;
    const root = document.querySelector(".build-reachability-tree");
    if (!root) return;
    bindTreePersist(root);
    bindTreeTabPersist(root);
    bindTreeModeToggle(root);
    applyTreeMode(root, loadTreeMode());
    syncTreeLinkTabs(root);
    pinSidebarScroll(root, () => {
      syncTreeActiveFromNode(root);
    });
    if (options && options.restorePersistedScroll) {
      restoreScroll(root);
    }
  }

  function syncTreeActiveFromNode(root) {
    let nodeId = "";
    try {
      nodeId = String(new URL(global.location.href).searchParams.get("node") || "").trim();
    } catch (_) {}
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
    if (!isBuildRoute()) return;
    refresh({ restorePersistedScroll: true });
    global.addEventListener("popstate", refresh);
    global.addEventListener("mei:manage-tab-change", refresh);
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", bind);
  } else {
    bind();
  }

  global.MeiBuildTreePersist = { refresh, captureScroll };
})(window);
