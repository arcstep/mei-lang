/**
 * Dev > 场景原型 WYSIWYG temp panels (region/section layout + content theme).
 */
(function initWysiwygPanel(global) {
  "use strict";

  const boot = (global.__meiLangBoot = global.__meiLangBoot || {});

  function isBuildRoute() {
    return /^\/apps\/(?:build|manage)\//.test(String(global.location.pathname || ""));
  }

  function buildLayoutPatch(previewScope, uiRole, values) {
    return {
      preview_scope: previewScope,
      ui_role: uiRole,
      layout: values || {},
    };
  }

  function buildThemePatch(previewScope, uiRole, values) {
    return {
      preview_scope: previewScope,
      ui_role: uiRole,
      theme: values || {},
    };
  }

  function applySessionPatch(patch) {
    if (!patch || !boot.viewCompositor) return;
    const root = global.document.querySelector(".preview-pane-scroll, .shell");
    const overlay = { patches: { [patch.preview_scope]: patch.layout || patch.theme || {} } };
    const theme = patch.theme ? { colors: patch.theme.colors || {}, fonts: patch.theme.fonts || {} } : null;
    boot.viewCompositor.applyThemeAndOverlay(root, theme, overlay);
    if (typeof boot.MeiOpsLayoutTuningOverlay?.applyHot === "function") {
      boot.MeiOpsLayoutTuningOverlay.applyHot(patch);
    }
  }

  function openPanelForSelection(meta) {
    if (!isBuildRoute() || !meta) return;
    const role = String(meta.ui_role || "").toLowerCase();
    if (role === "region" || role === "section") {
      boot.wysiwygPanel = { kind: "layout", meta };
      return;
    }
    if (role === "content" || role === "slot") {
      boot.wysiwygPanel = { kind: "theme", meta };
    }
  }

  boot.wysiwygPanelApi = {
    buildLayoutPatch,
    buildThemePatch,
    applySessionPatch,
    openPanelForSelection,
  };
})(typeof window !== "undefined" ? window : globalThis);
