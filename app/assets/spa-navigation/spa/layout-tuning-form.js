/**
 * Layout tuning form: resolve preview_scope values and apply session overlay hot patches.
 */
(function initLayoutTuningForm(global) {
  "use strict";

  const boot = (global.__meiLangBoot = global.__meiLangBoot || {});

  function previewRoot() {
    return (
      document.querySelector("[data-manage-tab-panel='preview'] .preview-pane-scroll") ||
      document.querySelector(".preview-pane-scroll") ||
      document.querySelector(".preview-pane")
    );
  }

  function normalizeScope(scope) {
    return String(scope || "").trim();
  }

  function domEntryForScope(scope) {
    const key = normalizeScope(scope);
    if (!key) return null;
    const inspect = global.MeiBuildInspectHighlight;
    let node = document.querySelector(`[data-preview-scope="${CSS.escape(key)}"]`);
    if (!(node instanceof HTMLElement) && inspect?.readStructureDomScope) {
      const root = previewRoot();
      const candidates = root
        ? Array.from(
            root.querySelectorAll(
              "[data-preview-scope], [data-mei-ui-scope], [data-mei-panel-id]",
            ),
          )
        : [];
      let best = null;
      let bestScore = -1;
      for (const el of candidates) {
        if (!(el instanceof HTMLElement)) continue;
        const domScope = inspect.readStructureDomScope(el);
        const score =
          typeof inspect.scopeAlignScore === "function"
            ? inspect.scopeAlignScore(domScope, key)
            : domScope === key
              ? 10_000
              : 0;
        if (score > bestScore) {
          bestScore = score;
          best = el;
        }
      }
      node = best;
    }
    if (!(node instanceof HTMLElement)) return null;
    const slotHeight =
      node.dataset.layoutTuningSlotHeight ||
      node.style.getPropertyValue("--mei-slot-height").replace(/px$/i, "").trim();
    const paddingProfile = node.dataset.layoutTuningPaddingProfile || "";
    const rowsRaw =
      node.dataset.layoutTuningContentRows || node.dataset.manifestContentRows || "";
    const gapRaw =
      node.dataset.layoutTuningContentGap ||
      node.dataset.manifestContentGap ||
      node.style.rowGap?.replace(/px$/i, "").trim() ||
      "";
    const entry = {};
    if (slotHeight) entry.slotHeight = slotHeight;
    if (paddingProfile) entry.paddingProfile = paddingProfile;
    const rows = rowsRaw
      .split(",")
      .map((part) => Number(part.trim()))
      .filter((value) => Number.isFinite(value));
    if (rows.length > 0 || gapRaw) {
      entry.contentBudget = {};
      if (rows.length > 0) entry.contentBudget.rows = rows;
      if (gapRaw) entry.contentBudget.gap = Number(gapRaw);
    }
    return Object.keys(entry).length > 0 ? entry : null;
  }

  function manifestEntryForScope(scope) {
    const key = normalizeScope(scope);
    if (!key) return null;
    const manifest = globalThis.__mei?.layout_budget_manifest?.entries;
    if (!manifest || typeof manifest !== "object") return null;
    const raw = manifest[key];
    if (!raw || typeof raw !== "object") return null;
    const entry = {};
    const slotHeight = raw.slot_height_px ?? raw.slotHeightPx ?? raw.slotHeight;
    if (slotHeight != null) entry.slotHeight = slotHeight;
    const paddingProfile = raw.padding_profile ?? raw.paddingProfile;
    if (paddingProfile) entry.paddingProfile = String(paddingProfile);
    const rows = raw.content_rows ?? raw.contentRows;
    const gap = raw.content_gap ?? raw.contentGap;
    if ((Array.isArray(rows) && rows.length > 0) || gap != null) {
      entry.contentBudget = {};
      if (Array.isArray(rows) && rows.length > 0) {
        entry.contentBudget.rows = rows.map((row) => Number(row));
      }
      if (gap != null && gap !== "") entry.contentBudget.gap = Number(gap);
    }
    return Object.keys(entry).length > 0 ? entry : null;
  }

  function mergeEntry(base, overlay) {
    const out = { ...(base || {}) };
    if (!overlay || typeof overlay !== "object") return out;
    if (overlay.slotHeight != null || overlay.slot_height != null) {
      out.slotHeight = overlay.slotHeight ?? overlay.slot_height;
    }
    if (overlay.paddingProfile || overlay.padding_profile) {
      out.paddingProfile = overlay.paddingProfile ?? overlay.padding_profile;
    }
    const budget = overlay.contentBudget || overlay.content_budget;
    if (budget && typeof budget === "object") {
      out.contentBudget = { ...(out.contentBudget || {}), ...budget };
    }
    return out;
  }

  function ancestorScopes(scope) {
    const parts = normalizeScope(scope).split("/").filter(Boolean);
    const out = [];
    while (parts.length > 0) {
      out.push(parts.join("/"));
      parts.pop();
    }
    return out;
  }

  function resolveLayoutTuningScope(previewScope) {
    const key = normalizeScope(previewScope);
    if (!key) return "";
    const ancestors = ancestorScopes(key);
    for (const candidate of ancestors) {
      if (manifestEntryForScope(candidate)) return candidate;
    }
    return key;
  }

  function resolvePatchScopes(previewScope) {
    const key = normalizeScope(previewScope);
    const ancestors = ancestorScopes(key);
    let rowsScope = "";
    let slotScope = "";
    let paddingScope = "";
    for (const candidate of ancestors) {
      const entry = manifestEntryForScope(candidate);
      if (!entry) continue;
      if (!rowsScope && entry.contentBudget?.rows?.length) rowsScope = candidate;
      if (!paddingScope && entry.paddingProfile) paddingScope = candidate;
      if (!slotScope && entry.slotHeight != null) slotScope = candidate;
    }
    const fallback = ancestors[ancestors.length - 1] || key;
    return {
      rows: rowsScope || fallback,
      slot: slotScope || fallback,
      padding: paddingScope || fallback,
      primary: resolveLayoutTuningScope(key),
    };
  }

  function resolveLayoutTuningEntry(scope, options) {
    const opts = options || {};
    const key = normalizeScope(scope);
    if (!key) return null;
    let entry = {};
    for (const ancestor of ancestorScopes(key).reverse()) {
      entry = mergeEntry(entry, manifestEntryForScope(ancestor) || {});
    }
    const primary = resolveLayoutTuningScope(key);
    if (opts.overlayEntries?.[primary]) {
      entry = mergeEntry(entry, opts.overlayEntries[primary]);
    }
    for (const ancestor of ancestorScopes(key)) {
      if (opts.overlayEntries?.[ancestor]) {
        entry = mergeEntry(entry, opts.overlayEntries[ancestor]);
      }
    }
    const store = global.MeiDraftLayerStore || boot.draftLayerStore;
    const sessionPatches = store?.normalizeOverlayPatches?.(
      store?.getSessionLayers?.(opts.appId)?.layoutOverlay,
    );
    for (const ancestor of ancestorScopes(key)) {
      if (sessionPatches?.[ancestor]) {
        entry = mergeEntry(entry, sessionPatches[ancestor]);
      }
    }
    const domEntry = domEntryForScope(key);
    if (domEntry) {
      entry = mergeEntry(entry, domEntry);
    }
    return Object.keys(entry).length > 0 ? entry : null;
  }

  async function fetchOverlayEntries(appId) {
    const overlayApi = global.MeiOpsLayoutTuningOverlay;
    if (!overlayApi?.fetchOverlay || !appId) return {};
    try {
      const payload = await overlayApi.fetchOverlay(appId);
      return payload?.entries && typeof payload.entries === "object" ? payload.entries : {};
    } catch (_) {
      return {};
    }
  }

  function applyMergedOverlayPatches(patches, targetWindow) {
    const view = targetWindow || global;
    const root = previewRoot();
    const compositor = boot.viewCompositor || view.__meiLangBoot?.viewCompositor;
    if (root instanceof HTMLElement && compositor?.applyThemeAndOverlay) {
      compositor.applyThemeAndOverlay(root, null, { patches: patches || {} });
      return true;
    }
    const overlayApi = view.MeiOpsLayoutTuningOverlay || global.MeiOpsLayoutTuningOverlay;
    if (overlayApi?.applyHot && patches) {
      return false;
    }
    return false;
  }

  function resolveThemeLayoutScope(previewScope) {
    const key = normalizeScope(previewScope);
    const parts = key.split("/").filter(Boolean);
    if (parts[0] === "home" && parts.length >= 3) {
      return `home/${parts[1].toUpperCase()}/${parts[2]}`;
    }
    if (parts.length >= 1) {
      return `home/T1/${parts[0]}`;
    }
    return key;
  }

  function mergedThemeLayoutPatches(appId) {
    const store = global.MeiDraftLayerStore || boot.draftLayerStore;
    return (
      store?.normalizeOverlayPatches?.(store?.getSessionLayers?.(appId)?.themeLayout) || {}
    );
  }

  function mergedSessionPatches(appId) {
    const store = global.MeiDraftLayerStore || boot.draftLayerStore;
    const layoutOverlay =
      store?.normalizeOverlayPatches?.(store?.getSessionLayers?.(appId)?.layoutOverlay) || {};
    const themeLayout = mergedThemeLayoutPatches(appId);
    return { ...layoutOverlay, ...themeLayout };
  }

  function applySessionHot(appId, targetWindow) {
    const patches = mergedSessionPatches(appId);
    const applied = applyMergedOverlayPatches(patches, targetWindow);
    const view = targetWindow || global;
    if (typeof view.MeiFrameStageBoot?.scheduleFrameViewportRelayout === "function") {
      try {
        view.MeiFrameStageBoot.scheduleFrameViewportRelayout();
      } catch (_) {}
    }
    return applied;
  }

  function putSessionPatch(appId, scope, patch, options) {
    const store = global.MeiDraftLayerStore || boot.draftLayerStore;
    if (!store?.putLayoutOverlayPatches) return false;
    const key = normalizeScope(scope);
    if (!key || !patch || typeof patch !== "object") return false;
    store.putLayoutOverlayPatches(appId, { [key]: patch });
    if (options?.forceRematerialize && boot.viewCompositor?.recomposeFromLayerStore) {
      const axes = boot.sceneManifestLoader?.readShellAxes?.() || {};
      boot.viewCompositor.recomposeFromLayerStore(appId, axes);
      return true;
    }
    return applySessionHot(appId, options?.targetWindow);
  }

  const debouncers = new Map();

  function scheduleSessionHot(appId, scope, patch, options) {
    const opts = options || {};
    const delay = Number(opts.debounceMs ?? 150);
    const token = `${appId}:${normalizeScope(scope)}`;
    const prev = debouncers.get(token);
    if (prev) global.clearTimeout(prev);
    const handle = global.setTimeout(() => {
      debouncers.delete(token);
      putSessionPatch(appId, scope, patch, opts);
      try {
        global.dispatchEvent(
          new CustomEvent("meilang:preview-updated", {
            bubbles: true,
            detail: {
              scope: "layout-tuning-draft",
              preview_scope: normalizeScope(scope),
              resetRuntimeQueryCache: false,
            },
          }),
        );
      } catch (_) {}
    }, delay);
    debouncers.set(token, handle);
  }

  function applyEntryToControls(controls, entry) {
    if (!controls || !entry || typeof entry !== "object") return;
    const sectionRowsInput = controls.querySelector('[data-draft-field="sectionRows"]');
    const gapInput = controls.querySelector('[data-draft-field="gap"]');
    const compoundInput = controls.querySelector('[data-draft-field="compoundWidth"]');
    const paddingSelect = controls.querySelector('[data-draft-field="paddingProfile"]');
    const sectionRows = entry.sectionRows || entry.section_rows;
    if (sectionRowsInput instanceof HTMLInputElement && Array.isArray(sectionRows)) {
      sectionRowsInput.value = sectionRows.join(",");
    }
    const gap = entry.gap ?? entry.stripGap;
    if (gapInput instanceof HTMLInputElement && gap != null) {
      gapInput.value = String(gap);
    }
    const compoundWidth = entry.compoundWidth ?? entry.compound_width;
    if (compoundInput instanceof HTMLInputElement && compoundWidth) {
      compoundInput.value = String(compoundWidth);
    }
    const profile = entry.paddingProfile ?? entry.padding_profile;
    if (paddingSelect instanceof HTMLSelectElement && profile) {
      paddingSelect.value = String(profile);
    }
  }

  global.MeiLayoutTuningForm = {
    resolveLayoutTuningEntry,
    resolveLayoutTuningScope,
    resolveThemeLayoutScope,
    resolvePatchScopes,
    fetchOverlayEntries,
    putSessionPatch,
    scheduleSessionHot,
    applySessionHot,
    applyEntryToControls,
    mergedSessionPatches,
    domEntryForScope,
    manifestEntryForScope,
  };
  boot.layoutTuningForm = global.MeiLayoutTuningForm;
})(typeof window !== "undefined" ? window : globalThis);
