/**
 * Build view: persist left-tree <details> open state across SPA navigations.
 */
(function (global) {
  "use strict";

  const STORAGE_KEY = "mei-build-tree-open";

  function isBuildRoute() {
    return /^\/apps\/(?:build|manage)\//.test(String(global.location.pathname || ""));
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

  function refresh() {
    if (!isBuildRoute()) return;
    const root = document.querySelector(".build-reachability-tree");
    if (!root) return;
    bindTreePersist(root);
  }

  function bind() {
    if (!isBuildRoute()) return;
    refresh();
    global.addEventListener("popstate", refresh);
    global.addEventListener("mei:manage-tab-change", refresh);
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", bind);
  } else {
    bind();
  }

  global.MeiBuildTreePersist = { refresh };
})(window);
