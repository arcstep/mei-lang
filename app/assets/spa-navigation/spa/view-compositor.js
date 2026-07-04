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
    }
  }

  function composePreview(root, structureDoc, projection, themeTokens, layoutOverlay) {
    const visible = nodesForProjection(structureDoc, projection);
    const scopes = new Set(visible.map((node) => String(node.preview_scope || "")));
    if (root instanceof HTMLElement) {
      root.querySelectorAll("[data-preview-scope]").forEach((el) => {
        const scope = String(el.getAttribute("data-preview-scope") || "");
        const show = !scope || scopes.has(scope);
        el.toggleAttribute("hidden", !show);
        el.classList.toggle("mei-compose-hidden", !show);
      });
      root.setAttribute("data-compose-projection", String(projection || "live_full"));
    }
    applyThemeAndOverlay(root, themeTokens, layoutOverlay);
    return { visibleCount: visible.length, projection };
  }

  boot.viewCompositor = {
    nodesForProjection,
    composePreview,
    applyThemeAndOverlay,
  };
})(typeof window !== "undefined" ? window : globalThis);
