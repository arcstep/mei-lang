/**
 * WYSIWYG temp panels for 开发 > 场景原型 (layout + theme).
 */
(function initWysiwygPanel(global) {
  "use strict";

  const boot = (global.__meiLangBoot = global.__meiLangBoot || {});
  let panelEl = null;

  function workspaceSurface() {
    const fromShell = String(
      global.document?.querySelector?.(".shell[data-surface]")?.getAttribute("data-surface") || "",
    )
      .trim()
      .toLowerCase();
    if (fromShell) return fromShell;
    try {
      const boot = global.__meiLangBoot;
      if (typeof boot?.parseViewContext === "function") {
        const surface = String(boot.parseViewContext(global.location.href)?.surface || "")
          .trim()
          .toLowerCase();
        if (surface) return surface;
      }
    } catch (_) {}
    const path = String(global.location.pathname || "");
    const match = path.match(/^\/apps\/[^/]+\/(layout|prototype)(?:\/|$)/);
    return match ? match[1] : "layout";
  }

  function isLayoutWorkspaceRoute() {
    return workspaceSurface() === "layout";
  }

  function isPrototypeWorkspaceRoute() {
    return workspaceSurface() === "prototype";
  }

  function isWorkspaceRoute() {
    const path = String(global.location.pathname || "");
    if (/^\/apps\/[^/]+\/(?:layout|prototype)(?:\/|$)/.test(path)) return true;
    try {
      const boot = global.__meiLangBoot;
      if (typeof boot?.parseViewContext === "function") {
        const surface = String(boot.parseViewContext(global.location.href)?.surface || "")
          .trim()
          .toLowerCase();
        return surface === "layout" || surface === "prototype";
      }
    } catch (_) {}
    return false;
  }

  function ensurePanel() {
    if (panelEl) return panelEl;
    panelEl = global.document.createElement("aside");
    panelEl.id = "mei-wysiwyg-panel";
    panelEl.className = "mei-wysiwyg-panel";
    panelEl.hidden = true;
    panelEl.innerHTML =
      '<header class="mei-wysiwyg-panel__title"></header><div class="mei-wysiwyg-panel__body"></div>';
    global.document.body.appendChild(panelEl);
    return panelEl;
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

  function appIdFromPath() {
    const parts = String(global.location.pathname || "")
      .split("/")
      .filter(Boolean);
    const appsIdx = parts.indexOf("apps");
    if (appsIdx >= 0 && parts[appsIdx + 1]) return parts[appsIdx + 1];
    const appIdx = parts.indexOf("app");
    if (appIdx >= 0 && parts[appIdx + 1]) return parts[appIdx + 1];
    return String(document.querySelector(".shell[data-app-path]")?.getAttribute("data-app-path") || "")
      .trim();
  }

  async function applySessionPatch(patch) {
    if (!patch) return;
    const appId = appIdFromPath();
    const root = global.document.querySelector(".preview-pane-scroll, .shell");
    const overlayApi = boot.MeiOpsLayoutTuningOverlay || global.MeiOpsLayoutTuningOverlay;
    if (patch.layout && overlayApi?.putSessionDraft && appId) {
      const scope = String(patch.preview_scope || "").trim();
      if (scope) {
        const tuning = { [scope]: patch.layout };
        try {
          await overlayApi.putSessionDraft(appId, tuning);
        } catch (error) {
          console.warn("[wysiwyg-panel] session draft failed", error);
        }
      }
    } else if (patch.theme && overlayApi?.putSessionDraft && appId) {
      const store = global.MeiDraftLayerStore || boot.draftLayerStore;
      store?.putThemeTokensPatch?.(appId, patch.theme);
      boot.viewCompositor?.recomposeFromLayerStore?.(
        appId,
        boot.sceneManifestLoader?.readShellAxes?.() || {},
      );
    }
    global.dispatchEvent(new CustomEvent("meilang:preview-updated", { detail: { patch } }));
  }

  function renderPanel(kind, meta) {
    const panel = ensurePanel();
    const title = panel.querySelector(".mei-wysiwyg-panel__title");
    const body = panel.querySelector(".mei-wysiwyg-panel__body");
    if (!title || !body) return;
    title.textContent =
      kind === "layout"
        ? `布局 · ${meta.preview_scope || meta.ui_role || ""}`
        : `样式 · ${meta.preview_scope || ""}`;
    body.innerHTML = "";
    if (kind === "layout") {
      const scopeNode = meta.preview_scope
        ? global.document.querySelector(
            `[data-preview-scope="${CSS.escape(meta.preview_scope)}"]`,
          )
        : null;
      const gap = global.document.createElement("input");
      gap.type = "text";
      gap.placeholder = "gap (e.g. 8px)";
      gap.dataset.field = "gap";
      if (scopeNode instanceof HTMLElement) {
        gap.value =
          scopeNode.dataset.layoutTuningContentGap ||
          String(scopeNode.style.rowGap || scopeNode.style.gap || "").trim();
      }
      const formApi = global.MeiLayoutTuningForm;
      const appId = appIdFromPath();
      if (formApi?.resolveLayoutTuningEntry && meta.preview_scope) {
        const entry = formApi.resolveLayoutTuningEntry(meta.preview_scope, { appId });
        const gapVal = entry?.contentBudget?.gap ?? entry?.content_budget?.gap;
        if (gapVal != null && gapVal !== "") gap.value = String(gapVal);
      }
      gap.addEventListener("input", () => {
        const gapVal = gap.value.trim();
        if (!meta.preview_scope || !appId) return;
        const layout = {};
        if (gapVal) {
          const numeric = Number(gapVal);
          layout.contentBudget = {
            gap: Number.isFinite(numeric) ? numeric : gapVal,
          };
        }
        const patch = buildLayoutPatch(meta.preview_scope, meta.ui_role, layout);
        if (formApi?.scheduleSessionHot && patch?.layout) {
          formApi.scheduleSessionHot(appId, meta.preview_scope, patch.layout, { debounceMs: 150 });
        }
      });
      body.appendChild(gap);
      const btn = global.document.createElement("button");
      btn.type = "button";
      btn.textContent = "应用布局";
      btn.addEventListener("click", () => {
        const gapVal = gap.value.trim();
        const layout = {};
        if (gapVal) {
          const numeric = Number(gapVal);
          layout.contentBudget = {
            gap: Number.isFinite(numeric) ? numeric : gapVal,
          };
        }
        void applySessionPatch(buildLayoutPatch(meta.preview_scope, meta.ui_role, layout));
      });
      body.appendChild(btn);
    } else {
      const color = global.document.createElement("input");
      color.type = "text";
      color.placeholder = "color (#333)";
      color.dataset.field = "color";
      body.appendChild(color);
      const btn = global.document.createElement("button");
      btn.type = "button";
      btn.textContent = "应用样式";
      btn.addEventListener("click", () => {
        applySessionPatch(
          buildThemePatch(meta.preview_scope, meta.ui_role, {
            colors: { fg: color.value },
          }),
        );
      });
      body.appendChild(btn);
    }
    panel.hidden = false;
  }

  function openPanelForSelection(meta) {
    if (!isWorkspaceRoute() || !meta) return;
    const role = String(meta.ui_role || "").toLowerCase();
    if (
      isLayoutWorkspaceRoute() &&
      (role === "region" || role === "section" || role === "slot")
    ) {
      boot.wysiwygPanel = { kind: "layout", meta };
      renderPanel("layout", meta);
      return;
    }
    if (isPrototypeWorkspaceRoute() && (role === "content" || role === "slot")) {
      boot.wysiwygPanel = { kind: "theme", meta };
      renderPanel("theme", meta);
    }
  }

  boot.wysiwygPanelApi = {
    buildLayoutPatch,
    buildThemePatch,
    applySessionPatch,
    openPanelForSelection,
  };
})(typeof window !== "undefined" ? window : globalThis);
