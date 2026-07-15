(function initManageOpsThemeLayoutOverlay() {
  const global = window;
  const boot = (global.__meiLangBoot = global.__meiLangBoot || {});

  function notifyThemeLayoutOverlay(reason) {
    try {
      global.dispatchEvent(
        new CustomEvent("meilang:preview-updated", {
          bubbles: true,
          detail: { reason: reason || "theme-layout-overlay", resetRuntimeQueryCache: false },
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

  async function fetchThemeLayoutOverlay(appId) {
    const resp = await fetch(
      `/api/ops/themes/layout/overlay/${encodeURIComponent(appId)}`,
      {
        credentials: "same-origin",
        headers: { Accept: "application/json", ...draftSessionHeaders() },
      },
    );
    if (!resp.ok) throw new Error(`theme.layout overlay failed: ${resp.status}`);
    return resp.json();
  }

  function applyThemeLayoutPatches(root, entries) {
    if (!(root instanceof HTMLElement) || !entries || typeof entries !== "object") return;
    Object.entries(entries).forEach(([scope, patch]) => {
      if (!patch || typeof patch !== "object") return;
      const node =
        root.querySelector(`[data-preview-scope="${CSS.escape(scope)}"]`) ||
        root.querySelector(`[data-mei-ui-scope="${CSS.escape(scope)}"]`);
      if (!(node instanceof HTMLElement)) return;
      const paddingProfile = patch.paddingProfile ?? patch.padding_profile;
      if (paddingProfile) {
        node.dataset.manifestPaddingProfile = String(paddingProfile);
      }
      const sectionRows = patch.sectionRows || patch.section_rows;
      if (Array.isArray(sectionRows) && sectionRows.length > 0) {
        node.style.display = "grid";
        node.style.gridTemplateRows = sectionRows.map((row) => String(row)).join(" ");
        node.dataset.manifestSectionRows = sectionRows.join(",");
      }
      const gap = patch.gap ?? patch.stripGap;
      if (gap != null && gap !== "") {
        node.style.gap = String(gap).endsWith("px") ? String(gap) : `${gap}px`;
      }
    });
  }

  async function applyThemeLayoutOverlayHot(appId, targetWindow) {
    const view = targetWindow || global;
    const payload = await fetchThemeLayoutOverlay(appId);
    const store = global.MeiDraftLayerStore || boot.draftLayerStore;
    const sessionPatches = store?.normalizeOverlayPatches?.(
      store?.getSessionLayers?.(appId)?.themeLayout,
    );
    const merged = { ...(payload.entries || {}), ...(sessionPatches || {}) };
    const root =
      view.document.querySelector(".preview-pane-scroll") ||
      view.document.querySelector(".preview-pane");
    const compositor = boot.viewCompositor || view.__meiLangBoot?.viewCompositor;
    if (root instanceof HTMLElement && compositor?.applyThemeAndOverlay) {
      compositor.applyThemeAndOverlay(root, null, { patches: merged });
      notifyThemeLayoutOverlay("theme-layout-overlay");
      return payload;
    }
    applyThemeLayoutPatches(root, merged);
    notifyThemeLayoutOverlay("theme-layout-overlay");
    return payload;
  }

  async function putThemeLayoutSessionDraft(appId, layout, options) {
    const store = global.MeiDraftLayerStore || boot.draftLayerStore;
    if (!store?.putThemeLayoutPatches) {
      throw new Error("theme layout draft store unavailable");
    }
    store.putThemeLayoutPatches(appId, layout);
    if (options?.forceRematerialize && boot.viewCompositor?.recomposeFromLayerStore) {
      const axes = boot.sceneManifestLoader?.readShellAxes?.() || {};
      boot.viewCompositor.recomposeFromLayerStore(appId, axes);
    } else {
      await applyThemeLayoutOverlayHot(appId);
    }
    notifyThemeLayoutOverlay("theme-layout-draft");
    return { ok: true, local: true };
  }

  async function applyThemeLayoutDraftToConfig(appId) {
    const store = global.MeiDraftLayerStore || boot.draftLayerStore;
    const layout = store?.normalizeOverlayPatches?.(
      store?.getSessionLayers?.(appId)?.themeLayout,
    );
    const resp = await fetch(
      `/api/ops/themes/layout/apply/${encodeURIComponent(appId)}`,
      {
        method: "POST",
        credentials: "same-origin",
        headers: {
          "Content-Type": "application/json",
          Accept: "application/json",
          ...draftSessionHeaders(),
        },
        body: JSON.stringify({ layout: layout || {} }),
      },
    );
    if (!resp.ok) throw new Error(`theme.layout apply failed: ${resp.status}`);
    const payload = await resp.json();
    store?.clearSession?.(appId);
    notifyThemeLayoutOverlay("theme-layout-persisted");
    return payload;
  }

  async function refreshThemeLayoutOverlay(appId, root) {
    if (!appId) return null;
    return applyThemeLayoutOverlayHot(appId, root?.ownerDocument?.defaultView || global);
  }

  boot.MeiOpsThemeLayoutOverlay = {
    refresh: refreshThemeLayoutOverlay,
    applyPatches: applyThemeLayoutPatches,
    applyHot: applyThemeLayoutOverlayHot,
    putSessionDraft: putThemeLayoutSessionDraft,
    applyDraftToConfig: applyThemeLayoutDraftToConfig,
    fetchOverlay: fetchThemeLayoutOverlay,
    notify: notifyThemeLayoutOverlay,
  };
  global.MeiOpsThemeLayoutOverlay = boot.MeiOpsThemeLayoutOverlay;
})();
