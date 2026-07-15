/**
 * Unified structure anchor resolver for preview DOM and reachability tree.
 */
(function initStructureAnchor(global) {
  "use strict";

  const REVIEW_ROLE_DEPTH = { plane: 0, region: 1, section: 2, slot: 3, content: 3 };

  function readReachabilityTree() {
    const script = document.getElementById("mei-build-reachability-tree");
    if (!(script instanceof HTMLScriptElement) || !script.textContent) return [];
    try {
      const roots = JSON.parse(script.textContent);
      return Array.isArray(roots) ? roots : [];
    } catch (_) {
      return [];
    }
  }

  function walkReachability(nodes, visitor) {
    for (const node of nodes || []) {
      if (!node || typeof node !== "object") continue;
      const hit = visitor(node);
      if (hit) return hit;
      const nested = walkReachability(node.children, visitor);
      if (nested) return nested;
    }
    return null;
  }

  function findReachabilityNode(nodeId) {
    const id = String(nodeId || "").trim();
    if (!id) return null;
    return walkReachability(readReachabilityTree(), (node) =>
      node?.node_id === id ? node : null,
    );
  }

  function readAnchorFromElement(el) {
    if (!(el instanceof HTMLElement)) return null;
    const nodeId = String(
      el.getAttribute("data-build-node") ||
        el.getAttribute("data-mei-node-id") ||
        el.getAttribute("data-build-focus") ||
        "",
    ).trim();
    const previewScope = String(
      el.getAttribute("data-preview-scope") ||
        el.getAttribute("data-mei-ui-scope") ||
        el.getAttribute("data-mei-panel-id") ||
        "",
    ).trim();
    const uiRole = String(el.getAttribute("data-mei-ui-role") || "")
      .trim()
      .toLowerCase();
    if (!nodeId && !previewScope && !uiRole) return null;
    return {
      node_id: nodeId,
      preview_scope: previewScope,
      ui_role: uiRole,
      element: el,
    };
  }

  function resolveAnchorFromDom(nodeId, previewScope) {
    const id = String(nodeId || "").trim();
    const scope = String(previewScope || "").trim();
    if (id) {
      const byNode =
        document.querySelector(`[data-build-node="${CSS.escape(id)}"]`) ||
        document.querySelector(`[data-mei-node-id="${CSS.escape(id)}"]`);
      const anchor = readAnchorFromElement(byNode);
      if (anchor) return anchor;
      const byFocus = document.querySelector(`[data-build-focus="${CSS.escape(id)}"]`);
      const focusAnchor = readAnchorFromElement(byFocus);
      if (focusAnchor) return focusAnchor;
    }
    if (scope) {
      const byScope = document.querySelector(`[data-preview-scope="${CSS.escape(scope)}"]`);
      const scopeAnchor = readAnchorFromElement(byScope);
      if (scopeAnchor) return scopeAnchor;
      const byUiScope = document.querySelector(`[data-mei-ui-scope="${CSS.escape(scope)}"]`);
      return readAnchorFromElement(byUiScope);
    }
    return null;
  }

  function resolveAnchor(nodeId, previewScope) {
    const id = String(nodeId || "").trim();
    const scope = String(previewScope || "").trim();
    const fromDom = resolveAnchorFromDom(id, scope);
    if (fromDom) return fromDom;
    const treeNode = findReachabilityNode(id);
    if (treeNode) {
      return {
        node_id: String(treeNode.node_id || id),
        preview_scope: String(treeNode.preview_scope || scope),
        ui_role: String(treeNode.ui_role || treeNode.role || "")
          .trim()
          .toLowerCase(),
        source_file: String(treeNode.source_file || "").trim(),
        source_symbol: String(treeNode.source_symbol || "").trim(),
      };
    }
    if (scope) {
      return { node_id: id, preview_scope: scope, ui_role: "" };
    }
    return null;
  }

  function focusSelectorForAnchor(anchor) {
    if (!anchor || typeof anchor !== "object") return "";
    const scope = String(anchor.preview_scope || "").trim();
    if (scope) {
      const escaped = CSS.escape(scope);
      return `[data-mei-ui-scope="${escaped}"], [data-preview-scope="${escaped}"]`;
    }
    const nodeId = String(anchor.node_id || "").trim();
    if (nodeId) return `[data-build-node="${CSS.escape(nodeId)}"]`;
    return "";
  }

  function elementReviewDepth(el) {
    if (!(el instanceof HTMLElement)) return 99;
    const role = String(el.getAttribute("data-mei-ui-role") || "")
      .trim()
      .toLowerCase();
    if (role && Object.prototype.hasOwnProperty.call(REVIEW_ROLE_DEPTH, role)) {
      return REVIEW_ROLE_DEPTH[role];
    }
    if (el.hasAttribute("data-mei-panel-id")) return 1;
    if (el.hasAttribute("data-preview-scope")) return 2;
    if (el.hasAttribute("data-mei-use-key") || el.hasAttribute("data-build-node")) return 3;
    return 99;
  }

  global.MeiStructureAnchor = {
    resolveAnchor,
    resolveAnchorFromDom,
    readAnchorFromElement,
    findReachabilityNode,
    focusSelectorForAnchor,
    elementReviewDepth,
    REVIEW_ROLE_DEPTH,
  };
})(window);
