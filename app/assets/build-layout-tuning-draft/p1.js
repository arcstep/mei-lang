/**
 * Build / 场景原型: layoutTuning session draft WYSIWYG bridge.
 * selectNode -> resolvePreviewScope -> buildDraftPatch -> putSessionDraft -> applyHot
 */
(function initBuildLayoutTuningDraftBridge(global) {
  "use strict";

  function isBuildRoute() {
    return /^\/apps\/(?:build|manage)\//.test(String(global.location.pathname || ""));
  }

  function appIdFromPath() {
    const parts = String(global.location.pathname || "")
      .split("/")
      .filter(Boolean);
    const idx = parts.indexOf("build");
    if (idx >= 0 && parts[idx + 1]) return parts[idx + 1];
    const manageIdx = parts.indexOf("manage");
    if (manageIdx >= 0 && parts[manageIdx + 1]) return parts[manageIdx + 1];
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
    const id = String(nodeId || "").trim();
    if (!id) return null;
    const script = document.getElementById("mei-build-reachability-tree");
    if (!script) return null;
    try {
      const roots = JSON.parse(script.textContent || "[]");
      if (!Array.isArray(roots)) return null;
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
      "[data-manage-tab-panel='preview'] .build-inspect-selected[data-preview-scope]",
    );
    if (selected instanceof HTMLElement) {
      return String(selected.getAttribute("data-preview-scope") || "").trim();
    }
    return "";
  }

  function buildDraftPatch(previewScope, fields) {
    const scope = String(previewScope || "").trim();
    if (!scope) return null;
    const patch = {};
    const slotHeight = fields?.slotHeight;
    if (slotHeight != null && slotHeight !== "") {
      const numeric = Number(slotHeight);
      patch.slotHeight = Number.isFinite(numeric) ? `${Math.round(numeric)}px` : String(slotHeight);
    }
    const paddingProfile = String(fields?.paddingProfile || "").trim();
    if (paddingProfile) {
      patch.paddingProfile = paddingProfile;
    }
    const rows = fields?.contentRows;
    const gap = fields?.contentGap;
    if ((Array.isArray(rows) && rows.length > 0) || (gap != null && gap !== "")) {
      patch.contentBudget = {};
      if (Array.isArray(rows) && rows.length > 0) {
        patch.contentBudget.rows = rows.map((row) => Number(row));
      }
      if (gap != null && gap !== "") {
        patch.contentBudget.gap = Number(gap);
      }
    }
    if (Object.keys(patch).length === 0) return null;
    return { tuning: { [scope]: patch } };
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
    return controls;
  }

  async function applyDraftFromControls(options) {
    if (!isBuildRoute()) return;
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
    if (boot.layerStore && boot.sceneManifestLoader?.fetchManifest) {
      try {
        const axes = boot.sceneManifestLoader.readShellAxes?.() || {};
        await boot.sceneManifestLoader.fetchManifest(appId, resolvePreviewScopeFromSelection() || "home", axes);
      } catch (_) {}
    }
    if (options?.persist && typeof overlay.applyDraftToConfig === "function") {
      await overlay.applyDraftToConfig(appId);
    } else {
      await overlay.applyHot(appId, global);
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
    if (!isBuildRoute()) return;
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
    scopeLabel.textContent = `scope: ${scope}`;
    const node = document.querySelector(
      `[data-preview-scope="${CSS.escape(scope)}"]`,
    );
    const applyEntryToControls = (entry) => {
      if (!entry || typeof entry !== "object") return;
      const rowsInput = controls.querySelector('[data-draft-field="contentRows"]');
      const gapInput = controls.querySelector('[data-draft-field="contentGap"]');
      const slotInput = controls.querySelector('[data-draft-field="slotHeight"]');
      const paddingSelect = controls.querySelector('[data-draft-field="paddingProfile"]');
      const contentBudget = entry.contentBudget || entry.content_budget;
      if (contentBudget && typeof contentBudget === "object") {
        const rows = contentBudget.rows || contentBudget.content_rows;
        if (rowsInput instanceof HTMLInputElement && Array.isArray(rows)) {
          rowsInput.value = rows.join(",");
        }
        const gap = contentBudget.gap ?? contentBudget.content_gap;
        if (gapInput instanceof HTMLInputElement && gap != null) {
          gapInput.value = String(gap);
        }
      }
      if (slotInput instanceof HTMLInputElement) {
        const slotHeight =
          entry.slotHeight ??
          entry.slot_height ??
          (node instanceof HTMLElement
            ? node.dataset.layoutTuningSlotHeight ||
              node.style.getPropertyValue("--mei-slot-height").replace(/px$/, "")
            : "");
        if (slotHeight) slotInput.value = String(slotHeight).trim();
      }
      if (paddingSelect instanceof HTMLSelectElement) {
        const profile =
          entry.paddingProfile ??
          entry.padding_profile ??
          (node instanceof HTMLElement ? node.dataset.layoutTuningPaddingProfile : "");
        if (profile) paddingSelect.value = String(profile);
      }
    };
    if (node instanceof HTMLElement) {
      applyEntryToControls({
        slotHeight:
          node.dataset.layoutTuningSlotHeight ||
          node.style.getPropertyValue("--mei-slot-height").replace(/px$/, ""),
        paddingProfile: node.dataset.layoutTuningPaddingProfile || "",
        contentBudget: {
          rows: (node.dataset.layoutTuningContentRows || node.dataset.manifestContentRows || "")
            .split(",")
            .map((part) => Number(part.trim()))
            .filter((value) => Number.isFinite(value)),
          gap:
            node.dataset.layoutTuningContentGap ||
            node.dataset.manifestContentGap ||
            node.style.rowGap?.replace(/px$/, ""),
        },
      });
    }
    const appId = appIdFromPath();
    const overlay = global.MeiOpsLayoutTuningOverlay;
    if (appId && overlay?.fetchOverlay) {
      void overlay
        .fetchOverlay(appId)
        .then((payload) => {
          const entry = payload?.entries?.[scope];
          if (entry) applyEntryToControls(entry);
        })
        .catch(() => {});
    }
  }

  function scheduleSync() {
    global.requestAnimationFrame(() => {
      syncDraftControls();
      const meta = resolvePreviewScopeFromSelection();
      if (meta && global.__meiLangBoot?.wysiwygPanelApi?.openPanelForSelection) {
        global.__meiLangBoot.wysiwygPanelApi.openPanelForSelection(meta);
      }
    });
  }

  global.addEventListener("popstate", scheduleSync);
  global.addEventListener("meilang:preview-updated", scheduleSync);
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
