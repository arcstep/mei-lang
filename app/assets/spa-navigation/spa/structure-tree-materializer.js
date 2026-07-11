/**
 * Structure tree materializer: render aside reachability tree from structure.full.
 * Shared by access + manage bundles (DOM aligned with app/src/ui/build_tree.rs).
 */
(function initStructureTreeMaterializer(global) {
  "use strict";

  const boot = (global.__meiLangBoot = global.__meiLangBoot || {});

  const UI_ROLE_RANK = {
    scene: -1,
    plane: 0,
    region: 1,
    section: 2,
    slot: 2,
    content: 3,
    budget: 2,
  };

  function roleDepthRank(role) {
    return UI_ROLE_RANK[String(role || "").trim().toLowerCase()] ?? 99;
  }

  function parentKey(node) {
    return String(node?.parent_id || "").trim();
  }

  function nodeHref(appId, surface, nodeId) {
    const slug = String(surface || "layout").trim() || "layout";
    const params = new URLSearchParams();
    if (nodeId) params.set("node", nodeId);
    const pathname = String(global.location?.pathname || "");
    if (typeof isUnifiedViewRoute === "function" && isUnifiedViewRoute(pathname)) {
      // legacy view URLs sealed → Access home
      return `/apps/${encodeURIComponent(appId)}/home`;
    }
    if (slug === "build" || slug === "layout" || slug === "prototype") {
      return `/apps/${encodeURIComponent(appId)}/home`;
    }
    const qs = params.toString();
    return `/apps/${encodeURIComponent(appId)}/home${qs ? `?${qs}` : ""}`;
  }

  function filteredNodes(structureDoc, maxRole) {
    const maxDepth = roleDepthRank(maxRole);
    const nodes = Array.isArray(structureDoc?.nodes) ? structureDoc.nodes : [];
    const byId = new Map(nodes.map((node) => [node.node_id, node]));
    const allowed = new Set();
    for (const node of nodes) {
      if (roleDepthRank(node.ui_role) > maxDepth) continue;
      let cur = node;
      while (cur) {
        allowed.add(cur.node_id);
        const parentId = parentKey(cur);
        cur = parentId ? byId.get(parentId) : null;
      }
    }
    return nodes.filter((node) => allowed.has(node.node_id));
  }

  function isCompoundSlotWrapper(node, nodes) {
    const role = String(node?.ui_role || "").trim().toLowerCase();
    if (role !== "slot") return false;
    const label = String(node?.label || "").trim().toLowerCase();
    if (label !== "compound" && !label.endsWith("_compound")) return false;
    const kids = childrenForParentRaw(nodes, node.node_id);
    return (
      kids.length > 0 &&
      kids.every((child) => String(child?.ui_role || "").trim().toLowerCase() === "slot")
    );
  }

  function childrenForParentRaw(nodes, parentId) {
    const pid = String(parentId || "").trim();
    return nodes.filter((node) => parentKey(node) === pid);
  }

  function childrenForParent(nodes, parentId) {
    const direct = childrenForParentRaw(nodes, parentId);
    const out = [];
    for (const child of direct) {
      if (isCompoundSlotWrapper(child, nodes)) {
        out.push(...childrenForParentRaw(nodes, child.node_id));
      } else {
        out.push(child);
      }
    }
    return out;
  }

  function resolveRoots(nodes, sceneRoots) {
    const byId = new Map(nodes.map((node) => [node.node_id, node]));
    const lookup = (id) => {
      const key = String(id || "").trim();
      if (!key) return null;
      if (byId.has(key)) return byId.get(key);
      const prefixed = key.startsWith("ui-scope:") ? key : `ui-scope:${key}`;
      if (byId.has(prefixed)) return byId.get(prefixed);
      const stripped = key.replace(/^ui-scope:/, "");
      if (byId.has(stripped)) return byId.get(stripped);
      return null;
    };
    if (Array.isArray(sceneRoots) && sceneRoots.length) {
      const roots = sceneRoots.map((id) => lookup(id)).filter(Boolean);
      if (roots.length) return roots;
    }
    return childrenForParent(nodes, "");
  }

  function displayLabel(node) {
    const label = String(node?.label || "").trim();
    if (label) return label;
    const scope = String(node?.preview_scope || "").trim();
    if (scope) return scope.replace(/^\.+/, "");
    return String(node?.node_id || "").trim();
  }

  function uiScopeGlyph(node) {
    const role = String(node?.ui_role || "").trim().toLowerCase();
    switch (role) {
      case "plane":
        return "P";
      case "region":
        return "R";
      case "section":
        return "§";
      case "slot":
        return "L";
      case "content":
        return "C";
      case "budget":
        return "B";
      case "scene":
        return "S";
      default:
        return "U";
    }
  }

  function branchDefaultOpen(node) {
    const role = String(node?.ui_role || "").trim().toLowerCase();
    return role === "scene" || role === "plane" || role === "region" || role === "section";
  }

  function appendCountBadge(labelEl, childCount) {
    if (!(childCount > 0)) return;
    const badge = document.createElement("span");
    badge.className = "build-tree-badge build-tree-badge--count";
    badge.title = `${childCount} 个子节点`;
    badge.textContent = String(childCount);
    labelEl.append(badge);
  }

  function renderLeaf(node, options) {
    const li = document.createElement("li");
    li.className = "build-tree-node";
    const link = document.createElement("a");
    link.className =
      node.node_id === options.activeNode
        ? "build-tree-link build-tree-link--active"
        : "build-tree-link";
    link.href = nodeHref(options.appId, options.surface, node.node_id);
    link.title = displayLabel(node);
    link.setAttribute("data-build-node", node.node_id);
    if (node.ui_role) link.setAttribute("data-ui-role", node.ui_role);
    if (node.preview_scope) link.setAttribute("data-preview-scope", node.preview_scope);
    const spacer = document.createElement("span");
    spacer.className = "build-tree-spacer";
    spacer.setAttribute("aria-hidden", "true");
    const kind = document.createElement("span");
    kind.className = "build-tree-kind";
    kind.setAttribute("aria-hidden", "true");
    kind.textContent = uiScopeGlyph(node);
    const label = document.createElement("span");
    label.className = "build-tree-label";
    label.textContent = displayLabel(node);
    link.append(spacer, kind, label);
    li.append(link);
    return li;
  }

  function renderBranch(node, nodes, options) {
    const kids = childrenForParent(nodes, node.node_id);
    const li = document.createElement("li");
    li.className = "build-tree-node build-tree-node--branch";
    const details = document.createElement("details");
    details.className = "build-tree-details";
    details.setAttribute("data-build-tree-branch", node.node_id);
    details.setAttribute("data-build-tree-children-count", String(kids.length));
    if (node.ui_role) details.setAttribute("data-ui-role", node.ui_role);
    details.open = branchDefaultOpen(node);
    const summary = document.createElement("summary");
    summary.className =
      node.node_id === options.activeNode
        ? "build-tree-summary build-tree-summary--active"
        : "build-tree-summary";
    const kind = document.createElement("span");
    kind.className = "build-tree-kind";
    kind.setAttribute("aria-hidden", "true");
    kind.textContent = uiScopeGlyph(node);
    const link = document.createElement("a");
    link.className = "build-tree-label build-tree-label--link";
    link.href = nodeHref(options.appId, options.surface, node.node_id);
    link.setAttribute("data-build-node", node.node_id);
    link.textContent = displayLabel(node);
    appendCountBadge(link, kids.length);
    summary.append(kind, link);
    const nested = document.createElement("ul");
    nested.className = "build-tree-list build-tree-list--nested";
    for (const child of kids) {
      nested.append(renderTreeNode(child, nodes, options));
    }
    details.append(summary, nested);
    li.append(details);
    return li;
  }

  function renderTreeNode(node, nodes, options) {
    const kids = childrenForParent(nodes, node.node_id);
    if (!kids.length) return renderLeaf(node, options);
    return renderBranch(node, nodes, options);
  }

  function ensureTreeMount(maxRole) {
    let shell = document.querySelector("aside .build-tree-shell");
    if (!shell) {
      const scroll = document.querySelector("aside .sidebar-scroll");
      if (!scroll) return null;
      shell = document.createElement("div");
      shell.className = "build-tree-shell";
      shell.setAttribute("data-build-tree-shell", "true");
      const inner = document.createElement("div");
      inner.className = "build-reachability-tree";
      shell.append(inner);
      scroll.replaceChildren(shell);
    }
    let tree = shell.querySelector(".build-reachability-tree");
    if (!tree) {
      tree = document.createElement("div");
      tree.className = "build-reachability-tree";
      shell.append(tree);
    }
    tree.setAttribute("data-build-tree-mode-active", "structure");
    if (maxRole) tree.setAttribute("data-build-tree-max-ui-role", maxRole);
    return tree;
  }

  function workspaceStructureTreeReady(generation) {
    const nav =
      document.querySelector("aside .build-reachability-tree") ||
      document.querySelector(".build-tree-shell .build-reachability-tree") ||
      document.querySelector("nav.build-reachability-tree");
    if (!nav?.querySelector(".build-tree-list .build-tree-node")) return false;
    if (generation == null) return true;
    const stamped = String(nav.getAttribute("data-mei-tree-generation") || "").trim();
    return stamped === String(generation);
  }

  function renderStructureTree(structureDoc, options) {
    const opts = options || {};
    const appId = String(opts.appId || "").trim();
    if (!appId || !structureDoc) return false;
    const surface = String(opts.surface || "layout").trim() || "layout";
    const generation = opts.generation;
    if (generation != null && workspaceStructureTreeReady(generation)) {
      return true;
    }
    const maxRole =
      String(opts.treeMaxUiRole || "").trim() ||
      String(document.body?.getAttribute("data-build-tree-max-ui-role") || "").trim() ||
      String(
        document.querySelector(".shell[data-build-tree-max-ui-role]")?.getAttribute(
          "data-build-tree-max-ui-role",
        ) || "",
      ).trim() ||
      String(
        document.querySelector(".build-reachability-tree")?.getAttribute(
          "data-build-tree-max-ui-role",
        ) || "",
      ).trim() ||
      (String(opts.surface || global.location?.pathname || "")
        .toLowerCase()
        .includes("layout")
        ? "content"
        : "slot");
    const nodes = filteredNodes(structureDoc, maxRole);
    const roots = resolveRoots(nodes, structureDoc.scene_roots);
    const tree = ensureTreeMount(maxRole);
    if (!tree) return false;
    const list = document.createElement("ul");
    list.className = "build-tree-list";
    for (const root of roots) {
      list.append(
        renderTreeNode(root, nodes, {
          appId,
          surface,
          activeNode: String(opts.activeNode || "").trim(),
        }),
      );
    }
    tree.replaceChildren(list);
    if (generation != null) {
      tree.setAttribute("data-mei-tree-generation", String(generation));
    }
    const script = document.getElementById("mei-build-reachability-tree");
    if (script) {
      script.textContent = JSON.stringify(nodes);
    }
    if (typeof global.MeiBuildTreePersist?.refresh === "function") {
      global.MeiBuildTreePersist.refresh();
    }
    return true;
  }

  boot.structureTreeMaterializer = {
    renderStructureTree,
    ensureTreeMount,
    workspaceStructureTreeReady,
    filteredNodes,
    resolveRoots,
  };
  boot.renderStructureTree = renderStructureTree;
  boot.workspaceStructureTreeReady = workspaceStructureTreeReady;
})(typeof window !== "undefined" ? window : globalThis);
