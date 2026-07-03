  const PRESENTATION_Z_TIERS = {
    t0: { min: 0, max: 1000, default: 1 },
    t1: { min: 1001, max: 2000, default: 1001 },
    t2: { min: 2001, max: 3000, default: 2001 },
    presentation: { min: 5000, max: 5399, default: 5000 },
    copilot: { min: 5400, max: 5799, default: 5400 },
    host: { min: 5800, max: 99999, default: 5800 },
  };
  const SUPPORTED_PLANES = ["t0", "t1", "t2"];
  const SUPPORTED_STAGE_KINDS = ["map-stage", "world-stage"];

  function normalizeStageKind(raw) {
    const kind = String(raw || "").trim().toLowerCase();
    return SUPPORTED_STAGE_KINDS.includes(kind) ? kind : "";
  }

  function stageHiddenClass(stageKind) {
    const normalized = normalizeStageKind(stageKind);
    return normalized ? `mei-stage-hidden-${normalized}` : "";
  }

  function resetStageVisibility() {
    const runtime = boot.worldStageRuntime;
    if (runtime && typeof runtime.resetStageVisibility === "function") {
      runtime.resetStageVisibility();
      return;
    }
    document.documentElement.classList.remove(
      "mei-stage-hidden-map-stage",
      "mei-stage-hidden-world-stage",
    );
    document.documentElement.classList.add("mei-stage-hidden-world-stage");
  }

  function setStageVisibility(stageKind, visible) {
    const runtime = boot.worldStageRuntime;
    if (runtime && typeof runtime.setStageVisibility === "function") {
      return runtime.setStageVisibility(stageKind, visible);
    }
    const normalized = normalizeStageKind(stageKind);
    if (!normalized) return false;
    document.documentElement.classList.toggle(stageHiddenClass(normalized), !visible);
    return true;
  }

  function enterWorldStageViewCore(action, entry) {
    const runtime = boot.worldStageRuntime;
    if (runtime && typeof runtime.enterWorldStageView === "function") {
      runtime.enterWorldStageView({ action, entry });
    } else {
      setStageVisibility("map-stage", false);
      setStageVisibility("world-stage", true);
      document.documentElement.classList.add("mei-world-stage-active");
    }
    const viewpointId = String(action?.viewpoint || action?.viewpointId || "").trim();
    if (viewpointId) {
      focusViewpoint(viewpointId);
    }
    return dispatchWorldTargetAction(
      {
        ...action,
        type: action?.cameraPreset || action?.camera_preset ? "camera_move" : "focus_entity",
        viewFamily: String(action?.viewFamily || action?.view_family || entry?.viewFamily || "world").trim(),
        stageKind: String(action?.stageKind || action?.stage_kind || entry?.stageKind || "world-stage").trim(),
      },
      entry || readViewpointEntry(viewpointId),
    );
  }

  function enterWorldStageView(action, entry) {
    if (action?.skipWorldTransition) {
      return enterWorldStageViewCore(action, entry);
    }
    const transition = boot.worldStageTransition;
    if (transition && typeof transition.runEnter === "function") {
      const label = String(
        action?.worldEnterLabel ||
          action?.label ||
          entry?.label ||
          action?.entityId ||
          "空间场景",
      ).trim();
      void transition.runEnter({ ...action, worldEnterLabel: label }, () =>
        enterWorldStageViewCore({ ...action, skipWorldTransition: true }, entry),
      );
      return true;
    }
    return enterWorldStageViewCore(action, entry);
  }

  function exitWorldStageViewCore(action, entry) {
    const runtime = boot.worldStageRuntime;
    if (runtime && typeof runtime.exitWorldStageView === "function") {
      runtime.exitWorldStageView({ action, entry });
    } else {
      setStageVisibility("world-stage", false);
      setStageVisibility("map-stage", true);
      document.documentElement.classList.remove("mei-world-stage-active");
    }
    const viewpointId = String(action?.viewpoint || action?.viewpointId || "").trim();
    if (viewpointId) {
      focusViewpoint(viewpointId);
    }
    return dispatchWorldTargetAction(
      {
        ...action,
        type: "camera_move",
        viewFamily: String(action?.viewFamily || action?.view_family || entry?.viewFamily || "map").trim(),
        stageKind: String(action?.stageKind || action?.stage_kind || entry?.stageKind || "map-stage").trim(),
      },
      entry || readViewpointEntry(viewpointId),
    );
  }

  function exitWorldStageView(action, entry) {
    if (action?.skipWorldTransition) {
      return exitWorldStageViewCore(action, entry);
    }
    const transition = boot.worldStageTransition;
    if (transition && typeof transition.runExit === "function") {
      void transition.runExit(action, () =>
        exitWorldStageViewCore({ ...action, skipWorldTransition: true }, entry),
      );
      return true;
    }
    return exitWorldStageViewCore(action, entry);
  }

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

  function readViewpointEntry(viewpointId) {
    const id = String(viewpointId || "").trim();
    if (!id) return null;
    const map = readPresentationMap();
    return map?.viewpoints?.[id] || null;
  }

  function stampWorldTargetDataset(target, entry) {
    if (!(target instanceof HTMLElement) || !entry || typeof entry !== "object") {
      return;
    }
    const mappings = [
      ["meiFocusTier", entry.tier],
      ["meiViewFamily", entry.viewFamily],
      ["meiStageKind", entry.stageKind],
      ["meiWorldRef", entry.worldRef],
      ["meiEntityId", entry.entityId],
      ["meiGroupId", entry.groupId],
      ["meiCameraPreset", entry.cameraPreset],
    ];
    mappings.forEach(([key, value]) => {
      if (value === undefined || value === null || value === "") return;
      target.dataset[key] = String(value);
    });
  }

  function resolveWorldTarget(action, entry) {
    const worldTarget = {
      type: String(action?.type || action?.kind || "").trim(),
      viewpointId: String(action?.viewpoint || action?.viewpointId || "").trim(),
      viewFamily: String(action?.viewFamily || action?.view_family || entry?.viewFamily || "").trim(),
      stageKind: String(action?.stageKind || action?.stage_kind || entry?.stageKind || "").trim(),
      worldRef: String(action?.worldRef || action?.world_ref || entry?.worldRef || "").trim(),
      entityId: String(action?.entityId || action?.entity_id || entry?.entityId || "").trim(),
      groupId: String(action?.groupId || action?.group_id || entry?.groupId || "").trim(),
      cameraPreset: String(
        action?.cameraPreset || action?.camera_preset || entry?.cameraPreset || "",
      ).trim(),
    };
    return worldTarget;
  }

  function dispatchWorldTargetAction(action, entry) {
    const worldTarget = resolveWorldTarget(action, entry);
    if (
      !worldTarget.viewpointId &&
      !worldTarget.viewFamily &&
      !worldTarget.stageKind &&
      !worldTarget.worldRef &&
      !worldTarget.entityId &&
      !worldTarget.groupId &&
      !worldTarget.cameraPreset
    ) {
      return false;
    }
    const root = typeof globalThis !== "undefined" ? globalThis : window;
    boot.lastWorldPresentationAction = worldTarget;
    const runtime = boot.worldStageRuntime;
    let applied = false;
    if (runtime && typeof runtime.applyWorldTarget === "function") {
      applied = Boolean(runtime.applyWorldTarget(worldTarget));
    }
    root.dispatchEvent(
      new CustomEvent("mei:presentation-world-action", {
        detail: worldTarget,
      }),
    );
    if (typeof console !== "undefined" && typeof console.info === "function") {
      console.info("[mei] presentation world action", worldTarget, { applied });
    }
    return applied || true;
  }

  function clearViewpointFocus() {
    document.querySelectorAll(".mei-viewpoint-focus").forEach((node) => {
      node.classList.remove("mei-viewpoint-focus");
    });
    document.documentElement.classList.remove("mei-tier-dim");
  }

  function planeHiddenClass(planeId) {
    return `mei-plane-hidden-${planeId}`;
  }

  function normalizePlaneId(raw) {
    const planeId = String(raw || "").trim().toLowerCase();
    return SUPPORTED_PLANES.includes(planeId) ? planeId : "";
  }

  function resetPlaneVisibility() {
    SUPPORTED_PLANES.forEach((planeId) => {
      document.documentElement.classList.remove(planeHiddenClass(planeId));
    });
  }

  function setPlaneVisibility(planeId, visible) {
    const normalized = normalizePlaneId(planeId);
    if (!normalized) return false;
    document.documentElement.classList.toggle(planeHiddenClass(normalized), !visible);
    return true;
  }

  function focusViewpoint(viewpointId) {
    const entry = readViewpointEntry(viewpointId);
    if (!entry) return false;
    clearViewpointFocus();
    const selector = `[data-mei-viewpoint="${CSS.escape(viewpointId)}"]`;
    const target = document.querySelector(selector);
    if (!(target instanceof HTMLElement)) return false;
    target.classList.add("mei-viewpoint-focus");
    if (entry.tier) {
      document.documentElement.classList.add("mei-tier-dim");
    }
    stampWorldTargetDataset(target, entry);
    target.scrollIntoView({ block: "nearest", behavior: "smooth" });
    return true;
  }

  function dispatchPresentationAction(action) {
    if (!action || typeof action !== "object") return false;
    const type = String(action.type || action.kind || "").trim();
    switch (type) {
      case "show_plane":
      case "showPlane":
        return setPlaneVisibility(action.plane || action.tier || action.planeId, true);
      case "hide_plane":
      case "hidePlane":
        return setPlaneVisibility(action.plane || action.tier || action.planeId, false);
      case "highlight":
      case "focus": {
        const viewpointId = String(action.viewpoint || action.viewpointId || "").trim();
        const entry = readViewpointEntry(viewpointId);
        const focused = focusViewpoint(viewpointId);
        const dispatched = dispatchWorldTargetAction(action, entry);
        return focused || dispatched;
      }
      case "camera_move":
      case "cameraMove":
        return dispatchWorldTargetAction(
          action,
          readViewpointEntry(action.viewpoint || action.viewpointId),
        );
      case "focus_entity":
      case "focusEntity": {
        const viewpointId = String(action.viewpoint || action.viewpointId || "").trim();
        const entry = readViewpointEntry(viewpointId);
        if (viewpointId) {
          focusViewpoint(viewpointId);
        }
        return dispatchWorldTargetAction(action, entry);
      }
      case "show_group":
      case "showGroup":
      case "hide_group":
      case "hideGroup":
        return dispatchWorldTargetAction(
          action,
          readViewpointEntry(action.viewpoint || action.viewpointId),
        );
      case "enter_world_view":
      case "enterWorldView": {
        const viewpointId = String(action.viewpoint || action.viewpointId || "").trim();
        const entry = readViewpointEntry(viewpointId);
        return enterWorldStageView(action, entry);
      }
      case "exit_world_view":
      case "exitWorldView": {
        const viewpointId = String(action.viewpoint || action.viewpointId || "").trim();
        const entry = readViewpointEntry(viewpointId);
        return exitWorldStageView(action, entry);
      }
      case "cutaway_toggle":
      case "cutawayToggle":
        return dispatchWorldTargetAction(
          action,
          readViewpointEntry(action.viewpoint || action.viewpointId),
        );
      case "clear_focus":
      case "clearFocus":
        clearViewpointFocus();
        return true;
      case "open_t2_page":
      case "open_board": {
        const sceneId = String(
          action.pageSceneId ||
            action.page_scene_id ||
            action.boardSceneId ||
            action.sceneId ||
            "",
        ).trim();
        if (!sceneId || typeof boot.openScene !== "function") return false;
        boot.openScene({
          scene_id: sceneId,
          kind: "scene_open",
          projection: String(action.projection || "overlay"),
        });
        return true;
      }
      default:
        return false;
    }
  }

  function installFocusController() {
    if (boot.focusControllerMounted) return;
    boot.focusControllerMounted = true;
    const root = typeof globalThis !== "undefined" ? globalThis : window;
    root.MeiPresentation = root.MeiPresentation || {};
    root.MeiPresentation.focus = focusViewpoint;
    root.MeiPresentation.clearFocus = clearViewpointFocus;
    root.MeiPresentation.showPlane = (planeId) => setPlaneVisibility(planeId, true);
    root.MeiPresentation.hidePlane = (planeId) => setPlaneVisibility(planeId, false);
    root.MeiPresentation.resetPlanes = resetPlaneVisibility;
    root.MeiPresentation.resetStages = resetStageVisibility;
    root.MeiPresentation.showStage = (stageKind) => setStageVisibility(stageKind, true);
    root.MeiPresentation.hideStage = (stageKind) => setStageVisibility(stageKind, false);
    root.MeiPresentation.dispatch = dispatchPresentationAction;
    root.MeiPresentation.map = readPresentationMap;
    root.MeiPresentation.resolveViewpoint = readViewpointEntry;
    root.MeiPresentation.zTiers = PRESENTATION_Z_TIERS;
    boot.dispatchPresentationAction = dispatchPresentationAction;
  }

  installFocusController();
