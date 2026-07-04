(function initManageOpsLayoutTuningOverlay() {
  const global = window;

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

  async function fetchOverlay(appId) {
    const resp = await fetch(
      `/api/ops/layout-tuning/overlay/${encodeURIComponent(appId)}`,
      { credentials: "same-origin", headers: { Accept: "application/json" } },
    );
    if (!resp.ok) throw new Error(`layoutTuning overlay failed: ${resp.status}`);
    return resp.json();
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
    });
    return patched;
  }

  async function applyLayoutTuningOverlayHot(appId, targetWindow) {
    const view = targetWindow || global;
    const payload = await fetchOverlay(appId);
    const root =
      view.document.querySelector(".preview-pane-scroll") ||
      view.document.querySelector(".preview-pane");
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
    const resp = await fetch(
      `/api/ops/layout-tuning/draft/${encodeURIComponent(appId)}`,
      {
        method: "PUT",
        credentials: "same-origin",
        headers: { "Content-Type": "application/json", Accept: "application/json" },
        body: JSON.stringify({ tuning }),
      },
    );
    if (!resp.ok) throw new Error(`layoutTuning draft failed: ${resp.status}`);
    return resp.json();
  }

  global.MeiOpsLayoutTuningOverlay = {
    applyHot: applyLayoutTuningOverlayHot,
    putSessionDraft,
    notify: notifyLayoutTuningOverlay,
  };
})();
