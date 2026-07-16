(() => {
  const boot = (window.__meiLangBoot = window.__meiLangBoot || {});
  const ENTER_EVENT = "mei:map-world-enter-request";
  const PICK_EVENT = "mei:map-world-object-pick";

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

  function resolveWorldEntryViewpoint(entityId, objectId, objectType, objectKey) {
    const entity = String(entityId || "").trim();
    const object = String(objectId || "").trim();
    const resolver = boot.objectResolver || window.MeiObjectResolver;
    const descriptor =
      resolver && typeof resolver.resolve === "function"
        ? resolver.resolve({ entityId, objectId, objectType, objectKey })
        : null;
    const canonicalObjectId = String(descriptor?.objectId || object).trim();
    if (!entity && !canonicalObjectId && !objectType && objectKey == null) return null;
    const viewpoints = readPresentationMap()?.viewpoints || {};
    const candidates = Object.entries(viewpoints)
      .map(([viewpointId, entry]) => ({ viewpointId, entry }))
      .filter(({ entry }) => {
        const family = String(entry?.viewFamily || entry?.view_family || "").trim();
        const entryEntity = String(entry?.entityId || entry?.entity_id || "").trim();
        const entryObject = String(entry?.objectId || entry?.object_id || "").trim();
        return (
          family === "world" &&
          (canonicalObjectId ? entryObject === canonicalObjectId : entryEntity === entity)
        );
      });
    if (!candidates.length) return null;
    const entryPreferred = candidates.find(({ viewpointId }) =>
      String(viewpointId || "").endsWith("_world_entry"),
    );
    return entryPreferred || candidates[0];
  }

  function dispatchEnterWorldView(detail) {
    const entityId = String(detail?.entityId || detail?.entity_id || "").trim();
    const objectId = String(detail?.objectId || detail?.object_id || "").trim();
    const objectType = String(detail?.objectType || detail?.object_type || "").trim();
    const objectKey = detail?.objectKey ?? detail?.object_key;
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
      : resolveWorldEntryViewpoint(entityId, objectId, objectType, objectKey);
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
    const resolver = boot.objectResolver || window.MeiObjectResolver;
    const descriptor =
      resolver && typeof resolver.resolve === "function"
        ? resolver.resolve({
            objectId: objectId || entry.objectId || entry.object_id,
            objectType: objectType || entry.objectType || entry.object_type,
            objectKey: objectKey ?? entry.objectKey ?? entry.object_key,
            entityId: entityId || entry.entityId || entry.entity_id,
            sourceRef: detail?.sourceRef || detail?.source_ref || entry.sourceRef || entry.source_ref,
          })
        : null;
    if (descriptor) {
      action.objectDescriptor = descriptor;
      action.objectId = descriptor.objectId;
      action.objectIdentityStatus = descriptor.identityStatus;
      if (descriptor.objectType) action.objectType = descriptor.objectType;
      if (descriptor.objectKey !== undefined) action.objectKey = descriptor.objectKey;
      if (descriptor.entityId !== undefined) action.entityId = descriptor.entityId;
      if (descriptor.sourceRef !== undefined) action.sourceRef = descriptor.sourceRef;
    }
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

  function dispatchMapWorldObjectPick(detail) {
    const interaction = boot.interactionRuntime || window.MeiInteraction;
    if (!interaction || typeof interaction.dispatchMany !== "function") return false;
    const descriptor =
      (boot.objectResolver || window.MeiObjectResolver)?.resolve?.(detail || {}) || null;
    if (!descriptor) return false;
    const events = interaction.dispatchMany(["select", "focus_viewpoint"], {
      descriptor,
      source: detail?.source || "map-world-pick",
      targetId: detail?.targetId || detail?.target_id,
      targetRole: detail?.targetRole || detail?.target_role,
    });
    return events.length === 2;
  }

  function onMapWorldObjectPick(event) {
    dispatchMapWorldObjectPick(event?.detail || {});
  }

  function installMapWorldBridge() {
    if (boot.mapWorldBridgeMounted) return;
    boot.mapWorldBridgeMounted = true;
    window.addEventListener(ENTER_EVENT, onMapWorldEnterRequest);
    window.addEventListener(PICK_EVENT, onMapWorldObjectPick);
    boot.mapWorldBridge = {
      dispatchEnterWorldView,
      dispatchMapWorldObjectPick,
      resolveWorldEntryViewpoint,
    };
  }

  installMapWorldBridge();
})();
