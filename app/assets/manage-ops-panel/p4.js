(function initManageOpsLayoutTuningOverlay() {
  const global = window;
  const boot = (global.__meiLangBoot = global.__meiLangBoot || {});

  function notifyLayoutTuningOverlay(reason) {
    try {
      global.dispatchEvent(
        new CustomEvent("meilang:preview-updated", {
          bubbles: true,
          detail: { reason: reason || "layout-tuning-overlay", resetRuntimeQueryCache: false },
        }),
      );
    } catch (_) {}
  }

  function ensureDraftSessionId() {
    const cookieKey = "mei-draft-session";
    const match = String(document.cookie || "").match(/mei-draft-session=([^;]+)/);
    if (match && match[1]) return decodeURIComponent(match[1].trim());
    const id = `web-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`;
    document.cookie = `${cookieKey}=${encodeURIComponent(id)};path=/;SameSite=Lax`;
    return id;
  }

  function draftSessionHeaders() {
    return { "x-mei-draft-session": ensureDraftSessionId() };
  }

  async function fetchOverlay(appId) {
    const resp = await fetch(
      `/api/ops/layout-tuning/overlay/${encodeURIComponent(appId)}`,
      {
        credentials: "same-origin",
        headers: { Accept: "application/json", ...draftSessionHeaders() },
      },
    );
    if (!resp.ok) throw new Error(`layoutTuning overlay failed: ${resp.status}`);
    return resp.json();
  }

  function applyContentBudgetToNode(node, budget) {
    if (!(node instanceof HTMLElement) || !budget || typeof budget !== "object") return false;
    let patched = false;
    const rows = budget.rows ?? budget.content_rows ?? budget.contentRows;
    const gap = budget.gap ?? budget.content_gap ?? budget.contentGap;
    if (Array.isArray(rows) && rows.length > 0) {
      const total = rows.reduce((sum, row) => sum + Number(row), 0);
      if (total > 0) {
        node.style.gridTemplateRows = rows
          .map((row) => `${(Number(row) / total) * 100}fr`)
          .join(" ");
      } else {
        node.style.gridTemplateRows = rows.map((row) => `${row}px`).join(" ");
      }
      node.dataset.layoutTuningContentRows = rows.join(",");
      patched = true;
    }
    if (gap != null && gap !== "") {
      node.style.rowGap = `${gap}px`;
      node.dataset.layoutTuningContentGap = String(gap);
      patched = true;
    }
    return patched;
  }

  function applyOverlayEntries(root, entries) {
    if (!(root instanceof HTMLElement) || !entries || typeof entries !== "object") return false;
    let patched = false;
    Object.entries(entries).forEach(([scope, patch]) => {
      if (!patch || typeof patch !== "object") return;
      const selector = `[data-preview-scope="${CSS.escape(scope)}"]`;
      const node = root.querySelector(selector);
      if (!(node instanceof HTMLElement)) return;
      const slotHeight =
        patch.slotHeight ?? patch.slot_height ?? patch.card_height ?? patch.cardHeight;
      if (slotHeight != null) {
        node.style.setProperty("--mei-slot-height", `${slotHeight}px`);
        node.dataset.layoutTuningSlotHeight = String(slotHeight);
        patched = true;
      }
      const paddingProfile = patch.paddingProfile ?? patch.padding_profile;
      if (paddingProfile) {
        node.dataset.layoutTuningPaddingProfile = String(paddingProfile);
        patched = true;
      }
      const contentBudget = patch.content_budget ?? patch.contentBudget;
      if (applyContentBudgetToNode(node, contentBudget)) {
        patched = true;
      }
    });
    return patched;
  }

  async function applyLayoutTuningOverlayHot(appId, targetWindow) {
    const view = targetWindow || global;
    const payload = await fetchOverlay(appId);
    const root =
      view.document.querySelector(".preview-pane-scroll") ||
      view.document.querySelector(".preview-pane");
    const boot = view.__meiLangBoot || global.__meiLangBoot || {};
    if (boot.viewCompositor?.applyThemeAndOverlay) {
      boot.viewCompositor.applyThemeAndOverlay(root, null, {
        patches: payload.entries || {},
      });
      notifyLayoutTuningOverlay(payload.draft_active ? "layout-tuning-draft" : "layout-tuning-overlay");
      return;
    }
    if (applyOverlayEntries(root, payload.entries || {})) {
      notifyLayoutTuningOverlay(payload.draft_active ? "layout-tuning-draft" : "layout-tuning-overlay");
      if (typeof view.MeiFrameStageBoot?.scheduleFrameViewportRelayout === "function") {
        try {
          view.MeiFrameStageBoot.scheduleFrameViewportRelayout();
        } catch (_) {}
      }
    }
  }

  async function putSessionDraft(appId, tuning) {
    const store = global.MeiDraftLayerStore || boot.draftLayerStore;
    if (!store?.putLayoutOverlayPatches) {
      throw new Error("draft layer store unavailable");
    }
    store.putLayoutOverlayPatches(appId, tuning);
    const axes = boot.sceneManifestLoader?.readShellAxes?.() || {};
    if (boot.viewCompositor?.recomposeFromLayerStore) {
      boot.viewCompositor.recomposeFromLayerStore(appId, axes);
    }
    notifyLayoutTuningOverlay("layout-tuning-draft");
    return { ok: true, local: true };
  }

  async function applyDraftToConfig(appId) {
    const store = global.MeiDraftLayerStore || boot.draftLayerStore;
    const tuning = store?.normalizeOverlayPatches?.(
      store?.getSessionLayers?.(appId)?.layoutOverlay,
    );
    const resp = await fetch(
      `/api/ops/layout-tuning/apply/${encodeURIComponent(appId)}`,
      {
        method: "POST",
        credentials: "same-origin",
        headers: {
          "Content-Type": "application/json",
          Accept: "application/json",
          ...draftSessionHeaders(),
        },
        body: JSON.stringify({ tuning: tuning || {} }),
      },
    );
    if (!resp.ok) throw new Error(`layoutTuning apply failed: ${resp.status}`);
    const payload = await resp.json();
    store?.clearSession?.(appId);
    if (boot.sceneManifestLoader?.fetchManifest) {
      try {
        const axes = boot.sceneManifestLoader.readShellAxes?.() || {};
        await boot.sceneManifestLoader.fetchManifest(appId, "home", axes);
      } catch (_) {}
    }
    notifyLayoutTuningOverlay("layout-tuning-persisted");
    return payload;
  }

  global.MeiOpsLayoutTuningOverlay = {
    applyHot: applyLayoutTuningOverlayHot,
    putSessionDraft,
    applyDraftToConfig,
    fetchOverlay,
    notify: notifyLayoutTuningOverlay,
  };
})();
