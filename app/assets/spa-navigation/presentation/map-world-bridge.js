(() => {
  const boot = (window.__meiLangBoot = window.__meiLangBoot || {});
  const ENTER_EVENT = "mei:map-world-enter-request";

  function readPresentationMap() {
    const node = document.getElementById("mei-presentation-map");
    if (!(node instanceof HTMLScriptElement) || !node.textContent) {
      return { viewpoints: {} };
    }
    try {
      return JSON.parse(node.textContent);
    } catch (_error) {
      return { viewpoints: {} };
    }
  }

  function resolveWorldEntryViewpoint(entityId) {
    const id = String(entityId || "").trim();
    if (!id) return null;
    const viewpoints = readPresentationMap()?.viewpoints || {};
    const candidates = Object.entries(viewpoints)
      .map(([viewpointId, entry]) => ({ viewpointId, entry }))
      .filter(({ entry }) => {
        const family = String(entry?.viewFamily || entry?.view_family || "").trim();
        const entity = String(entry?.entityId || entry?.entity_id || "").trim();
        return family === "world" && entity === id;
      });
    if (!candidates.length) return null;
    const entryPreferred = candidates.find(({ viewpointId }) =>
      String(viewpointId || "").endsWith("_world_entry"),
    );
    return entryPreferred || candidates[0];
  }

  function dispatchEnterWorldView(detail) {
    const entityId = String(detail?.entityId || detail?.entity_id || "").trim();
    const explicitViewpoint = String(
      detail?.viewpoint ||
        detail?.viewpointId ||
        detail?.enterViewpoint ||
        detail?.enter_viewpoint ||
        "",
    ).trim();
    const matched = explicitViewpoint
      ? {
          viewpointId: explicitViewpoint,
          entry: readPresentationMap()?.viewpoints?.[explicitViewpoint] || null,
        }
      : resolveWorldEntryViewpoint(entityId);
    if (!matched?.viewpointId) {
      if (typeof console !== "undefined" && typeof console.warn === "function") {
        console.warn("[mei] map-world-bridge: no world viewpoint for entity", entityId);
      }
      return false;
    }
    const entry = matched.entry || {};
    const action = {
      type: "enter_world_view",
      viewpoint: matched.viewpointId,
      entityId: entityId || entry.entityId || entry.entity_id || "",
      viewFamily: "world",
      stageKind: "world-stage",
      worldEnterLabel: String(
        detail?.worldEnterLabel ||
          detail?.enterLabel ||
          entry.label ||
          entry.name ||
          entityId ||
          "",
      ).trim(),
      worldRef: String(detail?.worldRef || detail?.world_ref || entry.worldRef || entry.world_ref || "").trim(),
      cameraPreset: String(
        detail?.cameraPreset ||
          detail?.camera_preset ||
          entry.cameraPreset ||
          entry.camera_preset ||
          "",
      ).trim(),
      groupId: String(
        detail?.groupId || detail?.group_id || entry.groupId || entry.group_id || "",
      ).trim(),
      panelId: String(detail?.panelId || entry.panelId || "world_viewport").trim(),
    };
    const dispatch = boot.dispatchPresentationAction;
    if (typeof dispatch === "function") {
      return dispatch(action);
    }
    if (window.MeiPresentation && typeof window.MeiPresentation.dispatch === "function") {
      return window.MeiPresentation.dispatch(action);
    }
    return false;
  }

  function onMapWorldEnterRequest(event) {
    dispatchEnterWorldView(event?.detail || {});
  }

  function installMapWorldBridge() {
    if (boot.mapWorldBridgeMounted) return;
    boot.mapWorldBridgeMounted = true;
    window.addEventListener(ENTER_EVENT, onMapWorldEnterRequest);
    boot.mapWorldBridge = {
      dispatchEnterWorldView,
      resolveWorldEntryViewpoint,
    };
  }

  installMapWorldBridge();
})();
