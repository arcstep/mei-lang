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

  function buildDraftPatch(previewScope, fields) {
    const rawScope = String(previewScope || "").trim();
    if (!rawScope) return null;
    const patchScopes =
      global.MeiLayoutTuningForm?.resolvePatchScopes?.(rawScope) || {
        rows: rawScope,
        slot: rawScope,
        padding: rawScope,
        primary: rawScope,
      };
    const tuning = {};
    const slotHeight = fields?.slotHeight;
    if (slotHeight != null && slotHeight !== "") {
      const numeric = Number(slotHeight);
      const value = Number.isFinite(numeric) ? `${Math.round(numeric)}px` : String(slotHeight);
      const slotKey = patchScopes.slot || rawScope;
      tuning[slotKey] = { ...(tuning[slotKey] || {}), slotHeight: value };
    }
    const paddingProfile = String(fields?.paddingProfile || "").trim();
    if (paddingProfile) {
      const paddingKey = patchScopes.padding || rawScope;
      tuning[paddingKey] = { ...(tuning[paddingKey] || {}), paddingProfile };
    }
    const rows = fields?.contentRows;
    const gap = fields?.contentGap;
    if ((Array.isArray(rows) && rows.length > 0) || (gap != null && gap !== "")) {
      const rowsKey = patchScopes.rows || rawScope;
      const budget = { ...((tuning[rowsKey] || {}).contentBudget || {}) };
      if (Array.isArray(rows) && rows.length > 0) {
        budget.rows = rows.map((row) => Number(row));
      }
      if (gap != null && gap !== "") {
        budget.gap = Number(gap);
      }
      tuning[rowsKey] = { ...(tuning[rowsKey] || {}), contentBudget: budget };
    }
    if (Object.keys(tuning).length === 0) return null;
    return { tuning, primaryScope: patchScopes.primary || rawScope };
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
      '<span>slotHeight</span>',
      '<input type="number" min="32" step="4" data-draft-field="slotHeight" class="w-20 rounded border px-1 py-0.5 text-xs" />',
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
      "<span>rows</span>",
      '<input type="text" data-draft-field="contentRows" placeholder="120,80" class="w-24 rounded border px-1 py-0.5 text-xs" />',
      "</label>",
      '<label class="flex items-center gap-1 text-xs">',
      "<span>gap</span>",
      '<input type="number" min="0" step="2" data-draft-field="contentGap" class="w-16 rounded border px-1 py-0.5 text-xs" />',
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
        const slotInput = controls.querySelector('[data-draft-field="slotHeight"]');
        const paddingSelect = controls.querySelector('[data-draft-field="paddingProfile"]');
        const rowsInput = controls.querySelector('[data-draft-field="contentRows"]');
        const gapInput = controls.querySelector('[data-draft-field="contentGap"]');
        const rows =
          rowsInput instanceof HTMLInputElement && rowsInput.value
            ? rowsInput.value
                .split(",")
                .map((part) => Number(part.trim()))
                .filter((value) => Number.isFinite(value))
            : null;
        const built = buildDraftPatch(scope, {
          slotHeight:
            slotInput instanceof HTMLInputElement && slotInput.value ? slotInput.value : null,
          paddingProfile:
            paddingSelect instanceof HTMLSelectElement ? paddingSelect.value : "",
          contentRows: rows,
          contentGap:
            gapInput instanceof HTMLInputElement && gapInput.value ? gapInput.value : null,
        });
        if (!built?.tuning || Object.keys(built.tuning).length === 0) return;
        void global.MeiOpsLayoutTuningOverlay?.putSessionDraft?.(appId, built.tuning);
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
    const overlay = global.MeiOpsLayoutTuningOverlay;
    if (!overlay?.putSessionDraft || !overlay?.applyHot) return;
    const appId = appIdFromPath();
    if (!appId) return;
    const controls = ensureDraftControls();
    if (!controls) return;
    const scope = resolvePreviewScopeFromSelection();
    if (!scope) return;
    const slotInput = controls.querySelector('[data-draft-field="slotHeight"]');
    const paddingSelect = controls.querySelector('[data-draft-field="paddingProfile"]');
    const rowsInput = controls.querySelector('[data-draft-field="contentRows"]');
    const gapInput = controls.querySelector('[data-draft-field="contentGap"]');
    const rows =
      rowsInput instanceof HTMLInputElement && rowsInput.value
        ? rowsInput.value
            .split(",")
            .map((part) => Number(part.trim()))
            .filter((value) => Number.isFinite(value))
        : null;
    const patch = buildDraftPatch(scope, {
      slotHeight:
        slotInput instanceof HTMLInputElement && slotInput.value
          ? slotInput.value
          : null,
      paddingProfile:
        paddingSelect instanceof HTMLSelectElement ? paddingSelect.value : "",
      contentRows: rows,
      contentGap:
        gapInput instanceof HTMLInputElement && gapInput.value ? gapInput.value : null,
    });
    if (!patch) return;
    await overlay.putSessionDraft(appId, patch.tuning);
    if (options?.persist && typeof overlay.applyDraftToConfig === "function") {
      await overlay.applyDraftToConfig(appId);
    }
    try {
      global.dispatchEvent(
        new CustomEvent("meilang:preview-updated", {
          bubbles: true,
          detail: {
            scope: "layout-tuning-draft",
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
    const overlay = global.MeiOpsLayoutTuningOverlay;
    if (appId && overlay?.fetchOverlay) {
      void overlay
        .fetchOverlay(appId)
        .then((payload) => {
          const entry = formApi?.resolveLayoutTuningEntry
            ? formApi.resolveLayoutTuningEntry(scope, {
                appId,
                overlayEntries: payload?.entries || {},
              })
            : payload?.entries?.[scope];
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
