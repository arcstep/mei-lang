/**
 * Render build reachability tree from shared structure.full artifact (client-side).
 */
(function initBuildTreeFromStructure(global) {
  "use strict";

  const boot = (global.__meiLangBoot = global.__meiLangBoot || {});

  const UI_ROLE_RANK = {
    plane: 0,
    region: 1,
    section: 2,
    slot: 2,
    content: 3,
  };

  function roleDepthRank(role) {
    return UI_ROLE_RANK[String(role || "").trim().toLowerCase()] ?? 99;
  }

  function nodeHref(appId, surface, nodeId) {
    const slug = String(surface || "layout").trim() || "layout";
    const params = new URLSearchParams();
    if (nodeId) params.set("node", nodeId);
    if (slug === "build") params.set("tab", "preview");
    const qs = params.toString();
    return `/apps/${encodeURIComponent(appId)}/${slug}${qs ? `?${qs}` : ""}`;
  }

  function filteredNodes(structureDoc, maxRole) {
    const maxDepth = roleDepthRank(maxRole);
    const nodes = Array.isArray(structureDoc?.nodes) ? structureDoc.nodes : [];
    const allowed = new Set();
    for (const node of nodes) {
      if (roleDepthRank(node.ui_role) <= maxDepth) {
        allowed.add(node.node_id);
      }
    }
    return nodes.filter((node) => allowed.has(node.node_id));
  }

  function childrenForParent(nodes, parentId) {
    return nodes.filter((node) => (node.parent_id || "") === (parentId || ""));
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
    link.title = node.label || node.node_id;
    link.setAttribute("data-build-node", node.node_id);
    if (node.ui_role) link.setAttribute("data-ui-role", node.ui_role);
    if (node.preview_scope) link.setAttribute("data-preview-scope", node.preview_scope);
    const spacer = document.createElement("span");
    spacer.className = "build-tree-spacer";
    spacer.setAttribute("aria-hidden", "true");
    const kind = document.createElement("span");
    kind.className = "build-tree-kind";
    kind.setAttribute("aria-hidden", "true");
    kind.textContent = "·";
    const label = document.createElement("span");
    label.className = "build-tree-label";
    label.textContent = node.label || node.node_id;
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
    details.open = true;
    const summary = document.createElement("summary");
    summary.className =
      node.node_id === options.activeNode
        ? "build-tree-summary build-tree-summary--active"
        : "build-tree-summary";
    const kind = document.createElement("span");
    kind.className = "build-tree-kind";
    kind.setAttribute("aria-hidden", "true");
    kind.textContent = "▸";
    const link = document.createElement("a");
    link.className = "build-tree-label build-tree-label--link";
    link.href = nodeHref(options.appId, options.surface, node.node_id);
    link.setAttribute("data-build-node", node.node_id);
    link.textContent = node.label || node.node_id;
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

  function renderStructureTree(structureDoc, options) {
    const opts = options || {};
    const appId = String(opts.appId || "").trim();
    if (!appId || !structureDoc) return false;
    const surface = String(opts.surface || "layout").trim() || "layout";
    const maxRole =
      String(opts.treeMaxUiRole || "").trim() ||
      String(document.body?.getAttribute("data-build-tree-max-ui-role") || "").trim() ||
      "section";
    const nodes = filteredNodes(structureDoc, maxRole);
    const roots = childrenForParent(nodes, "");
    const nav =
      document.querySelector("aside nav.build-reachability-tree") ||
      document.querySelector("nav.build-reachability-tree");
    if (!nav) return false;
    const list = document.createElement("ul");
    list.className = "build-tree-list";
    for (const root of roots) {
      list.append(renderTreeNode(root, nodes, {
        appId,
        surface,
        activeNode: String(opts.activeNode || "").trim(),
      }));
    }
    nav.replaceChildren(list);
    const script = document.getElementById("mei-build-reachability-tree");
    if (script) {
      script.textContent = JSON.stringify(nodes);
    }
    if (typeof global.MeiBuildTreePersist?.refresh === "function") {
      global.MeiBuildTreePersist.refresh();
    }
    return true;
  }

  boot.renderStructureTree = renderStructureTree;
})(typeof window !== "undefined" ? window : globalThis);
