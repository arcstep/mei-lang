/**
 * ViewCompositor: compose review_projection depth without refetching structure.full.
 */
(function initViewCompositor(global) {
  "use strict";

  const boot = (global.__meiLangBoot = global.__meiLangBoot || {});

  const PROJECTION_MAX_ROLE = {
    plane_region: "region",
    plane_region_section: "section",
    content: "content",
    live_full: "content",
    static_full: "content",
  };

  function roleDepth(role) {
    const map = { plane: 0, region: 1, section: 2, slot: 3, content: 3 };
    return map[String(role || "").toLowerCase()] ?? 99;
  }

  function nodesForProjection(structureDoc, projection) {
    const maxRole = PROJECTION_MAX_ROLE[String(projection || "").toLowerCase()] || "content";
    const maxDepth = roleDepth(maxRole);
    const nodes = Array.isArray(structureDoc?.nodes) ? structureDoc.nodes : [];
    return nodes.filter((node) => roleDepth(node.ui_role) <= maxDepth);
  }

  function clearComposeArtifacts(root) {
    if (!(root instanceof HTMLElement)) return;
    root.querySelectorAll(".mei-compose-hidden").forEach((el) => {
      if (!(el instanceof HTMLElement)) return;
      el.classList.remove("mei-compose-hidden");
      el.removeAttribute("hidden");
    });
  }

  function applyThemeAndOverlay(root, themeTokens, layoutOverlay) {
    if (!(root instanceof HTMLElement)) return;
    const colors = themeTokens?.colors || {};
    const fonts = themeTokens?.fonts || {};
    Object.keys(colors).forEach((token) => {
      root.style.setProperty(`--mei-${token}`, String(colors[token]));
    });
    Object.keys(fonts).forEach((token) => {
      root.style.setProperty(`--mei-font-${token}`, String(fonts[token]));
    });
    const patches = layoutOverlay?.patches;
    if (patches && typeof patches === "object") {
      root.setAttribute("data-layout-overlay", JSON.stringify(patches));
      Object.entries(patches).forEach(([scope, patch]) => {
        if (!patch || typeof patch !== "object") return;
        const node = root.querySelector(`[data-preview-scope="${CSS.escape(scope)}"]`);
        if (!(node instanceof HTMLElement)) return;
        const contentBudget = patch.contentBudget || patch.content_budget;
        if (contentBudget && typeof contentBudget === "object") {
          const rows = contentBudget.rows || contentBudget.content_rows;
          if (Array.isArray(rows) && rows.length > 0) {
            const total = rows.reduce((sum, row) => sum + Number(row), 0);
            if (total > 0) {
              node.style.gridTemplateRows = rows.map((row) => `${(Number(row) / total) * 100}fr`).join(" ");
            }
          }
          const gap = contentBudget.gap ?? contentBudget.content_gap;
          if (gap != null && gap !== "") {
            node.style.rowGap = `${gap}px`;
          }
        }
      });
    }
  }

  function extractLayerDocument(layerValue) {
    if (!layerValue) return null;
    if (Array.isArray(layerValue.nodes) || layerValue.schema_version) {
      return layerValue;
    }
    if (layerValue.document) return layerValue.document;
    return layerValue;
  }

  function ensureStructureSkeleton(root, structureDoc) {
    if (!(root instanceof HTMLElement)) return false;
    const doc = extractLayerDocument(structureDoc);
    const nodes = Array.isArray(doc?.nodes) ? doc.nodes : [];
    if (root.querySelector("[data-preview-scope], [data-mei-ui-role]")) {
      return nodes.length > 0 || !!root.querySelector("[data-preview-scope]");
    }
    for (const node of nodes) {
      const scope = String(node.preview_scope || "").trim();
      if (!scope) continue;
      const el = document.createElement("div");
      el.setAttribute("data-preview-scope", scope);
      el.setAttribute("data-mei-ui-role", String(node.ui_role || ""));
      el.className = `mei-compose-node mei-compose-${String(node.ui_role || "node").toLowerCase()}`;
      root.appendChild(el);
    }
    return nodes.length > 0;
  }

  function applyShellLayer(root, shellLayer) {
    if (!(root instanceof HTMLElement)) return;
    const doc = extractLayerDocument(shellLayer);
    if (!doc) return;
    if (doc.tab) root.setAttribute("data-tab", String(doc.tab));
    if (doc.chrome) root.setAttribute("data-chrome", String(doc.chrome));
    if (doc.route_mode) root.setAttribute("data-route-mode", String(doc.route_mode));
    const topbar = String(doc.topbar_html || "").trim();
    if (topbar && !root.querySelector(".mei-shell-topbar")) {
      const wrap = document.createElement("div");
      wrap.innerHTML = topbar;
      const bar = wrap.firstElementChild;
      if (bar) root.prepend(bar);
    }
  }

  function composeFromLayers(root, layers, composeAxes) {
    if (!(root instanceof HTMLElement) || !layers) return false;
    const projection =
      composeAxes?.review_projection ||
      composeAxes?.reviewProjection ||
      "live_full";
    const structure = extractLayerDocument(layers["structure.full"]);
    if (!structure) return false;
    applyShellLayer(root, layers["shell.build"] || layers["shell.app"] || layers["shell.run"]);
    ensureStructureSkeleton(root, structure);
    const themeDoc = extractLayerDocument(layers["theme.tokens"]);
    const overlayDoc = extractLayerDocument(layers["layout.overlay"]);
    const evalDoc = layers["eval.slot_group.scene:default"] || null;
    if (evalDoc?.bootstrap_seed && globalThis.__mei) {
      globalThis.__mei.bootstrap_seed = evalDoc.bootstrap_seed;
    }
    composePreview(root, structure, projection, themeDoc, overlayDoc);
    return true;
  }

  function composePreview(root, structureDoc, projection, themeTokens, layoutOverlay) {
    clearComposeArtifacts(root);
    if (root instanceof HTMLElement) {
      root.setAttribute("data-compose-projection", String(projection || "live_full"));
      if (global.MeiProjectionDepth?.applyReviewProjectionChrome) {
        global.MeiProjectionDepth.applyReviewProjectionChrome(root, {
          reviewProjection: projection,
        });
      }
    }
    applyThemeAndOverlay(root, themeTokens, layoutOverlay);
    const visible = nodesForProjection(structureDoc, projection);
    return { visibleCount: visible.length, projection };
  }

  boot.viewCompositor = {
    nodesForProjection,
    composePreview,
    composeFromLayers,
    ensureStructureSkeleton,
    applyShellLayer,
    applyThemeAndOverlay,
    clearComposeArtifacts,
  };
})(typeof window !== "undefined" ? window : globalThis);
