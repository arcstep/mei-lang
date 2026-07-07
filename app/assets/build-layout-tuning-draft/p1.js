/**
 * Build / 场景原型: layoutTuning session draft WYSIWYG bridge.
 * selectNode -> resolvePreviewScope -> buildDraftPatch -> putSessionDraft -> applyHot
 */
(function initBuildLayoutTuningDraftBridge(global) {
  "use strict";

  function isLayoutWorkspaceRoute() {
    const path = String(global.location.pathname || "");
    if (/^\/apps\/[^/]+\/layout(?:\/|$)/.test(path)) {
      const fromShell = String(
        document.querySelector(".shell[data-surface]")?.getAttribute("data-surface") || "",
      )
        .trim()
        .toLowerCase();
      return !fromShell || fromShell === "layout";
    }
    try {
      const boot = global.__meiLangBoot;
      if (typeof boot?.parseViewContext === "function") {
        const ctx = boot.parseViewContext(global.location.href);
        return String(ctx?.surface || "").trim().toLowerCase() === "layout";
      }
    } catch (_) {}
    return false;
  }

  function isWorkspaceRoute() {
    const path = String(global.location.pathname || "");
    if (/^\/apps\/[^/]+\/layout(?:\/|$)/.test(path)) return isLayoutWorkspaceRoute();
    try {
      const boot = global.__meiLangBoot;
      if (typeof boot?.parseViewContext === "function") {
        const ctx = boot.parseViewContext(global.location.href);
        const surface = String(ctx?.surface || "").trim().toLowerCase();
        return surface === "layout";
      }
    } catch (_) {}
    return false;
  }

  function appIdFromPath() {
    const parts = String(global.location.pathname || "")
      .split("/")
      .filter(Boolean);
    const appsIdx = parts.indexOf("apps");
    if (appsIdx >= 0 && parts[appsIdx + 1]) return parts[appsIdx + 1];
    return String(document.querySelector(".shell[data-app-path]")?.getAttribute("data-app-path") || "")
      .trim();
  }

  function activeBuildNode() {
    try {
      const fromUrl = String(new URL(global.location.href).searchParams.get("node") || "").trim();
      if (fromUrl) return fromUrl;
    } catch (_) {}
    return String(document.querySelector(".shell[data-build-node]")?.getAttribute("data-build-node") || "")
      .trim();
  }

  function readUiScopeMetaFromTree(nodeId) {
    const inspect = global.MeiBuildInspectHighlight;
    if (typeof inspect?.readUiScopeMetaFromNode === "function") {
      return inspect.readUiScopeMetaFromNode(nodeId);
    }
    const id = String(nodeId || "").trim();
    if (!id) return null;
    const script = document.getElementById("mei-build-reachability-tree");
    if (!script) return null;
    try {
      const roots = JSON.parse(script.textContent || "[]");
      if (!Array.isArray(roots)) return null;
      const flat =
        roots.length > 0 &&
        typeof roots[0]?.node_id === "string" &&
        (Object.prototype.hasOwnProperty.call(roots[0], "parent_id") ||
          (Array.isArray(roots[0]?.children) &&
            roots[0].children.length > 0 &&
            typeof roots[0].children[0] === "string") ||
          (Array.isArray(roots[0]?.children) && roots[0].children.length === 0));
      if (flat) {
        const node = roots.find((entry) => String(entry?.node_id || "").trim() === id);
        if (!node) return null;
        return {
          preview_scope: String(node.preview_scope || "").trim(),
          ui_role: String(node.ui_role || node.badges?.[0] || "").trim(),
        };
      }
      const walk = (nodes) => {
        for (const node of nodes || []) {
          if (node?.node_id === id) {
            return {
              preview_scope: String(node.preview_scope || "").trim(),
              ui_role: String(node.ui_role || node.badges?.[0] || "").trim(),
            };
          }
          const nested = walk(node.children);
          if (nested) return nested;
        }
        return null;
      };
      for (const root of roots) {
        const found = walk(root.children);
        if (found) return found;
      }
    } catch (_) {}
    return null;
  }

  function resolvePreviewScopeFromSelection() {
    const host = document.querySelector(
      ".preview-pane-scroll[data-build-inspect-scope], .preview-pane-scroll[data-build-inspect-active]",
    );
    const scopeFromHost = String(host?.getAttribute("data-build-inspect-scope") || "").trim();
    const rawScope = (() => {
      if (scopeFromHost) return scopeFromHost;
      const node = activeBuildNode();
      if (node.startsWith("ui-scope:")) {
        const meta = readUiScopeMetaFromTree(node);
        if (meta?.preview_scope) return meta.preview_scope;
      }
      if (node.startsWith("scene-panel:") || node.startsWith("scene-block:")) {
        const encoded = node.replace(/^scene-(?:panel|block):/i, "");
        const slash = encoded.indexOf("/");
        if (slash >= 0) return encoded.slice(slash + 1);
      }
      const selected = document.querySelector(
        "[data-manage-tab-panel='preview'] .build-inspect-selected[data-preview-scope], [data-manage-tab-panel='preview'] .build-inspect-selected[data-mei-panel-id]",
      );
      if (selected instanceof HTMLElement) {
        return String(
          selected.getAttribute("data-preview-scope") ||
            selected.getAttribute("data-mei-ui-scope") ||
            selected.getAttribute("data-mei-panel-id") ||
            "",
        ).trim();
      }
      return "";
    })();
    const formApi = global.MeiLayoutTuningForm;
    if (formApi?.resolveLayoutTuningScope) {
      return formApi.resolveLayoutTuningScope(rawScope);
    }
    return rawScope;
  }

  function themeLayoutScopeForPreview(previewScope) {
    const raw = String(previewScope || "").trim();
    if (!raw) return "";
    const parts = raw.split("/").filter(Boolean);
    if (parts.length >= 2 && parts[0] === "home") {
      const tier = parts[1].toUpperCase();
      const tail = parts.slice(1).join("/");
      return `home/${tier}/${tail.split("/")[0]}`;
    }
    return raw.split("/").slice(0, 2).join("/");
  }

  function buildDraftPatch(previewScope, fields) {
    const rawScope = String(previewScope || "").trim();
    if (!rawScope) return null;
    const layoutScope =
      global.MeiLayoutTuningForm?.resolveThemeLayoutScope?.(rawScope) ||
      themeLayoutScopeForPreview(rawScope);
    const layout = {};
    const sectionRows = fields?.sectionRows;
    if (Array.isArray(sectionRows) && sectionRows.length > 0) {
      layout[layoutScope] = {
        ...(layout[layoutScope] || {}),
        sectionRows: sectionRows.map((row) => String(row).trim()).filter(Boolean),
      };
    }
    const paddingProfile = String(fields?.paddingProfile || "").trim();
    if (paddingProfile) {
      const paddingScope =
        global.MeiLayoutTuningForm?.resolveLayoutTuningScope?.(rawScope) || rawScope;
      layout[paddingScope] = {
        ...(layout[paddingScope] || {}),
        paddingProfile,
      };
    }
    const compoundWidth = String(fields?.compoundWidth || "").trim();
    if (compoundWidth) {
      layout[rawScope] = {
        ...(layout[rawScope] || {}),
        compoundWidth,
      };
    }
    const gap = fields?.gap;
    if (gap != null && gap !== "") {
      layout[layoutScope] = {
        ...(layout[layoutScope] || {}),
        gap: String(gap).trim(),
      };
    }
    if (Object.keys(layout).length === 0) return null;
    return { layout, primaryScope: layoutScope };
  }

  function inspectBarRoot() {
    return document.getElementById("build-inspect-bar");
  }

  function ensureDraftControls() {
    const bar = inspectBarRoot();
    if (!bar) return null;
    let controls = bar.querySelector("#build-layout-tuning-draft-controls");
    if (controls instanceof HTMLElement) return controls;
    controls = document.createElement("div");
    controls.id = "build-layout-tuning-draft-controls";
    controls.className = "build-layout-tuning-draft-controls flex flex-wrap items-center gap-2 mt-2";
    controls.hidden = true;
    controls.innerHTML = [
      '<label class="flex items-center gap-1 text-xs">',
      "<span>sectionRows</span>",
      '<input type="text" data-draft-field="sectionRows" placeholder="1fr,2fr,3fr" class="w-28 rounded border px-1 py-0.5 text-xs" />',
      "</label>",
      '<label class="flex items-center gap-1 text-xs">',
      "<span>padding</span>",
      '<select data-draft-field="paddingProfile" class="rounded border px-1 py-0.5 text-xs">',
      '<option value="">—</option>',
      '<option value="compact">compact</option>',
      '<option value="comfortable">comfortable</option>',
      '<option value="spacious">spacious</option>',
      "</select>",
      "</label>",
      '<label class="flex items-center gap-1 text-xs">',
      "<span>gap</span>",
      '<input type="text" data-draft-field="gap" placeholder="12px" class="w-16 rounded border px-1 py-0.5 text-xs" />',
      "</label>",
      '<label class="flex items-center gap-1 text-xs">',
      "<span>compoundW</span>",
      '<input type="text" data-draft-field="compoundWidth" placeholder="220px" class="w-16 rounded border px-1 py-0.5 text-xs" />',
      "</label>",
      '<button type="button" data-draft-apply class="build-toolbar-btn text-xs">应用 draft 预览</button>',
      '<button type="button" data-draft-persist class="build-toolbar-btn text-xs">应用到配置</button>',
      '<span data-draft-scope class="text-xs opacity-70"></span>',
    ].join("");
    bar.appendChild(controls);
    controls.querySelector("[data-draft-apply]")?.addEventListener("click", () => {
      void applyDraftFromControls();
    });
    controls.querySelector("[data-draft-persist]")?.addEventListener("click", () => {
      void applyDraftFromControls({ persist: true });
    });
    if (!controls.__layoutTuningLiveBound) {
      controls.__layoutTuningLiveBound = true;
      const onLiveChange = () => {
        if (!isLayoutWorkspaceRoute()) return;
        const appId = appIdFromPath();
        const scope = resolvePreviewScopeFromSelection();
        if (!appId || !scope) return;
        const sectionRowsInput = controls.querySelector('[data-draft-field="sectionRows"]');
        const paddingSelect = controls.querySelector('[data-draft-field="paddingProfile"]');
        const gapInput = controls.querySelector('[data-draft-field="gap"]');
        const compoundInput = controls.querySelector('[data-draft-field="compoundWidth"]');
        const sectionRows =
          sectionRowsInput instanceof HTMLInputElement && sectionRowsInput.value
            ? sectionRowsInput.value
                .split(",")
                .map((part) => part.trim())
                .filter(Boolean)
            : null;
        const built = buildDraftPatch(scope, {
          sectionRows,
          paddingProfile:
            paddingSelect instanceof HTMLSelectElement ? paddingSelect.value : "",
          gap: gapInput instanceof HTMLInputElement && gapInput.value ? gapInput.value : null,
          compoundWidth:
            compoundInput instanceof HTMLInputElement && compoundInput.value
              ? compoundInput.value
              : null,
        });
        if (!built?.layout || Object.keys(built.layout).length === 0) return;
        void global.MeiOpsThemeLayoutOverlay?.putSessionDraft?.(appId, built.layout);
        const formApi = global.MeiLayoutTuningForm;
        if (formApi?.applySessionHot) {
          formApi.applySessionHot(appId);
        } else if (formApi?.scheduleSessionHot) {
          for (const [patchScope, patch] of Object.entries(built.tuning)) {
            formApi.scheduleSessionHot(appId, patchScope, patch, { debounceMs: 150 });
          }
        }
      };
      controls.addEventListener("input", onLiveChange);
      controls.addEventListener("change", onLiveChange);
    }
    return controls;
  }

  async function applyDraftFromControls(options) {
    if (!isWorkspaceRoute()) return;
    const overlay = global.MeiOpsThemeLayoutOverlay || global.MeiOpsLayoutTuningOverlay;
    if (!overlay?.putSessionDraft || !overlay?.applyHot) return;
    const appId = appIdFromPath();
    if (!appId) return;
    const controls = ensureDraftControls();
    if (!controls) return;
    const scope = resolvePreviewScopeFromSelection();
    if (!scope) return;
    const sectionRowsInput = controls.querySelector('[data-draft-field="sectionRows"]');
    const paddingSelect = controls.querySelector('[data-draft-field="paddingProfile"]');
    const gapInput = controls.querySelector('[data-draft-field="gap"]');
    const compoundInput = controls.querySelector('[data-draft-field="compoundWidth"]');
    const sectionRows =
      sectionRowsInput instanceof HTMLInputElement && sectionRowsInput.value
        ? sectionRowsInput.value
            .split(",")
            .map((part) => part.trim())
            .filter(Boolean)
        : null;
    const patch = buildDraftPatch(scope, {
      sectionRows,
      paddingProfile:
        paddingSelect instanceof HTMLSelectElement ? paddingSelect.value : "",
      gap: gapInput instanceof HTMLInputElement && gapInput.value ? gapInput.value : null,
      compoundWidth:
        compoundInput instanceof HTMLInputElement && compoundInput.value
          ? compoundInput.value
          : null,
    });
    if (!patch) return;
    await overlay.putSessionDraft(appId, patch.layout);
    if (options?.persist && typeof overlay.applyDraftToConfig === "function") {
      await overlay.applyDraftToConfig(appId);
    }
    try {
      global.dispatchEvent(
        new CustomEvent("meilang:preview-updated", {
          bubbles: true,
          detail: {
            scope: "theme-layout-draft",
            preview_scope: scope,
            resetRuntimeQueryCache: false,
          },
        }),
      );
    } catch (_) {}
  }

  function syncDraftControls() {
    if (!isWorkspaceRoute()) return;
    const controls = ensureDraftControls();
    if (!controls) return;
    const scope = resolvePreviewScopeFromSelection();
    const scopeLabel = controls.querySelector("[data-draft-scope]");
    if (!(scopeLabel instanceof HTMLElement)) return;
    if (!scope) {
      controls.hidden = true;
      scopeLabel.textContent = "";
      return;
    }
    controls.hidden = false;
    const displayScope =
      global.MeiLayoutTuningForm?.resolveLayoutTuningScope?.(scope) || scope;
    scopeLabel.textContent = `scope: ${scope}${displayScope !== scope ? ` → ${displayScope}` : ""}`;
    const formApi = global.MeiLayoutTuningForm;
    const appId = appIdFromPath();
    const fillFromEntry = (entry) => {
      if (formApi?.applyEntryToControls) {
        formApi.applyEntryToControls(controls, entry);
      }
    };
    if (formApi?.resolveLayoutTuningEntry) {
      const local = formApi.resolveLayoutTuningEntry(scope, { appId });
      if (local) fillFromEntry(local);
    }
    const overlay = global.MeiOpsThemeLayoutOverlay || global.MeiOpsLayoutTuningOverlay;
    if (appId && overlay?.fetchOverlay) {
      void overlay
        .fetchOverlay(appId)
        .then((payload) => {
          const layoutScope =
            formApi?.resolveThemeLayoutScope?.(scope) || themeLayoutScopeForPreview(scope);
          const entry =
            payload?.entries?.[layoutScope] ||
            payload?.entries?.[scope] ||
            (formApi?.resolveLayoutTuningEntry
              ? formApi.resolveLayoutTuningEntry(scope, {
                  appId,
                  overlayEntries: payload?.entries || {},
                })
              : null);
          if (entry) fillFromEntry(entry);
        })
        .catch(() => {});
    }
  }

  function selectionMetaFromActiveNode() {
    const node = activeBuildNode();
    if (!node.startsWith("ui-scope:")) return null;
    return readUiScopeMetaFromTree(node);
  }

  function scheduleSync() {
    global.requestAnimationFrame(() => {
      syncDraftControls();
      const meta = selectionMetaFromActiveNode();
      if (meta?.preview_scope && global.__meiLangBoot?.wysiwygPanelApi?.openPanelForSelection) {
        global.__meiLangBoot.wysiwygPanelApi.openPanelForSelection(meta);
      }
    });
  }

  global.addEventListener("mei:build-node-selected", () => {
    scheduleSync();
  });
  global.addEventListener("popstate", scheduleSync);
  global.addEventListener("meilang:preview-updated", scheduleSync);
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", scheduleSync);
  } else {
    scheduleSync();
  }
  document.addEventListener("click", (event) => {
    const target = event.target;
    if (!(target instanceof Element)) return;
    if (
      target.closest("[data-build-node]") ||
      target.closest("[data-preview-scope]") ||
      target.closest(".build-reachability-tree")
    ) {
      scheduleSync();
    }
  });

  global.MeiBuildLayoutTuningDraft = {
    resolvePreviewScopeFromSelection,
    buildDraftPatch,
    applyDraftFromControls,
    syncDraftControls,
  };

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", scheduleSync, { once: true });
  } else {
    scheduleSync();
  }
})(window);
