/**
 * Build view: persist left-tree <details> open state across SPA navigations.
 */
(function (global) {
  "use strict";

  const STORAGE_KEY = "mei-build-tree-open";
  const SCROLL_KEY = "mei-build-tree-scroll";

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

  function syncTreeLinkTabs(root) {
    const tab = currentManageTab();
    if (!tab) return;
    root.querySelectorAll("a.build-tree-link, a.build-tree-label--link").forEach((link) => {
      try {
        const url = new URL(link.href, global.location.href);
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
          url.searchParams.set("tab", currentManageTab());
          url.searchParams.delete("focus");
          link.href = url.toString();
        } catch (_) {}
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

  function refresh(options) {
    if (!isBuildRoute()) return;
    const root = document.querySelector(".build-reachability-tree");
    if (!root) return;
    bindTreePersist(root);
    bindTreeTabPersist(root);
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
