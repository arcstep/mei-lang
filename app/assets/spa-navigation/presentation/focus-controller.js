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
    const resolvedEntry = entry || readViewpointEntry(viewpointId);
    if (viewpointId) {
      focusViewpoint(viewpointId);
    }
    const cameraPreset = String(
      action?.cameraPreset ||
        action?.camera_preset ||
        resolvedEntry?.cameraPreset ||
        resolvedEntry?.camera_preset ||
        "",
    ).trim();
    const groupId = String(
      action?.groupId || action?.group_id || resolvedEntry?.groupId || resolvedEntry?.group_id || "",
    ).trim();
    return dispatchWorldTargetAction(
      {
        ...action,
        viewpoint: viewpointId || action?.viewpoint,
        cameraPreset,
        groupId,
        type: cameraPreset ? "camera_move" : "focus_entity",
        viewFamily: String(action?.viewFamily || action?.view_family || resolvedEntry?.viewFamily || "world").trim(),
        stageKind: String(action?.stageKind || action?.stage_kind || resolvedEntry?.stageKind || "world-stage").trim(),
      },
      resolvedEntry,
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
    if (boot.presentationDebug && typeof console !== "undefined" && typeof console.info === "function") {
      console.info("[mei] presentation world action", worldTarget, { applied });
    }
    return applied || true;
  }

  function clearViewpointFocus() {
    document.querySelectorAll(".mei-viewpoint-focus, .mei-structure-focus").forEach((node) => {
      node.classList.remove("mei-viewpoint-focus", "mei-structure-focus");
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

  function defaultHiddenPlanes() {
    const root = document.documentElement;
    const explicit = String(root.getAttribute("data-mei-default-hidden-planes") || "")
      .trim()
      .toLowerCase();
    if (explicit) {
      return explicit
        .split(/[,\s]+/)
        .map((entry) => normalizePlaneId(entry))
        .filter(Boolean);
    }
    if (document.querySelector("[data-mei-t2-page='true']")) {
      root.setAttribute("data-mei-default-hidden-planes", "t2");
      return ["t2"];
    }
    return [];
  }

  function t2PageSelector(panelId = "") {
    const normalized = String(panelId || "").trim();
    const nameSelector = normalized
      ? `[data-mei-t2-page="true"][data-mei-panel-name="${CSS.escape(normalized)}"]`
      : "";
    const scopeSelector = normalized
      ? `[data-mei-t2-page="true"][data-preview-scope$="/${CSS.escape(normalized)}"]`
      : "";
    return [nameSelector, scopeSelector].filter(Boolean).join(", ");
  }

  function allT2PagePanels() {
    return Array.from(document.querySelectorAll("[data-mei-t2-page='true']")).filter(
      (node) => node instanceof HTMLElement,
    );
  }

  function resetT2Panels() {
    document.documentElement.removeAttribute("data-mei-active-t2-panel");
    allT2PagePanels().forEach((node) => {
      node.toggleAttribute("hidden", true);
      node.classList.remove("mei-t2-page-active");
    });
  }

  function resolveT2PanelSceneId(panel, panelId) {
    const candidates = [
      panel.getAttribute("data-mei-board-scene"),
      panel.getAttribute("data-mei-scene-id"),
      panel.querySelector("[data-mei-drilldown-scene]")?.getAttribute("data-mei-drilldown-scene"),
    ];
    for (const value of candidates) {
      const trimmed = String(value || "").trim();
      if (trimmed) {
        return trimmed;
      }
    }
    const assemblies = window.__mei?.scene_projection_assembly_by_id;
    const panelName = String(panel.getAttribute("data-mei-panel-name") || panelId || "").trim();
    if (panelName && assemblies && typeof assemblies === "object") {
      for (const [sceneId, assembly] of Object.entries(assemblies)) {
        const key = String(assembly?.key || "");
        if (key.includes(panelName)) {
          return sceneId;
        }
      }
    }
    return panelName;
  }

  function openT2Panel(panelId) {
    // 0335: prefer Layer2 tab open; region visibility path is deprecated fallback.
    const selector = t2PageSelector(panelId);
    const target = selector ? document.querySelector(selector) : null;
    const openId = String(
      (target instanceof HTMLElement ? resolveT2PanelSceneId(target, panelId) : "") ||
        panelId ||
        "",
    ).trim();
    if (openId && typeof boot.openLayer2Tab === "function") {
      try {
        boot.openLayer2Tab({
          sceneId: openId,
          boardSceneId: openId,
          label: openId,
          overlaySize: "large",
          overlayWorkspace: { tab_policy: "append", size: "large" },
        });
        setPlaneVisibility("t2", true);
        document.documentElement.setAttribute("data-mei-active-t2-panel", openId);
        return true;
      } catch (_err) {
        /* fall through to legacy */
      }
    }
    if (!(target instanceof HTMLElement)) return false;
    setPlaneVisibility("t2", true);
    const normalized = String(
      target.getAttribute("data-mei-panel-name") || panelId || "",
    ).trim();
    allT2PagePanels().forEach((node) => {
      const active = node === target;
      node.toggleAttribute("hidden", !active);
      node.classList.toggle("mei-t2-page-active", active);
    });
    document.documentElement.setAttribute("data-mei-active-t2-panel", normalized);
    const legacySceneId = resolveT2PanelSceneId(target, panelId);
    if (legacySceneId && typeof boot.dispatchScopeActivation === "function") {
      const shell = document.querySelector("[data-runtime-node][data-app-path], .shell[data-app-path]");
      const appId = shell ? String(shell.getAttribute("data-app-path") || "").trim() : "";
      boot.dispatchScopeActivation({
        scope: legacySceneId,
        sceneId: legacySceneId,
        appId,
        source: "t2-inline",
      });
    }
    return true;
  }

  function resetPlaneVisibility() {
    SUPPORTED_PLANES.forEach((planeId) => {
      document.documentElement.classList.remove(planeHiddenClass(planeId));
    });
    defaultHiddenPlanes().forEach((planeId) => {
      document.documentElement.classList.add(planeHiddenClass(planeId));
    });
    resetT2Panels();
  }

  function setPlaneVisibility(planeId, visible) {
    const normalized = normalizePlaneId(planeId);
    if (!normalized) return false;
    document.documentElement.classList.toggle(planeHiddenClass(normalized), !visible);
    if (!visible && normalized === "t2") {
      resetT2Panels();
    }
    return true;
  }

  function focusViewpoint(viewpointId) {
    const entry = readViewpointEntry(viewpointId);
    if (!entry) return false;
    clearViewpointFocus();
    let target = null;
    const anchorApi = globalThis.MeiStructureAnchor;
    if (anchorApi && typeof anchorApi.resolveAnchor === "function") {
      const previewScope = String(
        entry.previewScope || entry.preview_scope || entry.panelPath || entry.panel_path || "",
      ).trim();
      const nodeId = String(entry.nodeId || entry.node_id || "").trim();
      const anchor = anchorApi.resolveAnchor(nodeId, previewScope);
      const selector = anchorApi.focusSelectorForAnchor(anchor);
      if (selector) {
        target = document.querySelector(selector);
      }
    }
    if (!(target instanceof HTMLElement)) {
      const selector = `[data-mei-viewpoint="${CSS.escape(viewpointId)}"]`;
      target = document.querySelector(selector);
    }
    if (!(target instanceof HTMLElement)) return false;
    target.classList.add("mei-viewpoint-focus", "mei-structure-focus");
    if (entry.tier) {
      document.documentElement.classList.add("mei-tier-dim");
    }
    stampWorldTargetDataset(target, entry);
    target.scrollIntoView({ block: "nearest", behavior: "smooth" });
    return true;
  }

  function focusStructure(target) {
    const nodeId = String(target?.node_id || target?.nodeId || "").trim();
    const previewScope = String(
      target?.preview_scope || target?.previewScope || "",
    ).trim();
    const uiRole = String(target?.ui_role || target?.uiRole || "")
      .trim()
      .toLowerCase();
    if (!nodeId && !previewScope) return false;
    clearViewpointFocus();
    let el = null;
    const anchorApi = globalThis.MeiStructureAnchor;
    if (anchorApi && typeof anchorApi.resolveAnchor === "function") {
      const anchor = anchorApi.resolveAnchor(nodeId, previewScope);
      el = anchor?.element || null;
      if (!(el instanceof HTMLElement) && typeof anchorApi.focusSelectorForAnchor === "function") {
        const selector = anchorApi.focusSelectorForAnchor(anchor);
        if (selector) el = document.querySelector(selector);
      }
    }
    if (!(el instanceof HTMLElement) && nodeId) {
      el =
        document.querySelector(`[data-build-node="${CSS.escape(nodeId)}"]`) ||
        document.querySelector(`[data-mei-node-id="${CSS.escape(nodeId)}"]`);
    }
    if (!(el instanceof HTMLElement) && previewScope) {
      el = document.querySelector(`[data-preview-scope="${CSS.escape(previewScope)}"]`);
    }
    if (!(el instanceof HTMLElement)) return false;
    el.classList.add("mei-structure-focus");
    el.scrollIntoView({ block: "nearest", behavior: "smooth" });
    try {
      document.dispatchEvent(
        new CustomEvent("mei:structure-focus", {
          detail: {
            node_id: nodeId || el.getAttribute("data-build-node") || "",
            preview_scope: previewScope || el.getAttribute("data-preview-scope") || "",
            ui_role: uiRole || el.getAttribute("data-mei-ui-role") || "",
          },
        }),
      );
    } catch (_) {}
    return true;
  }

  function highlightStructureNode(nodeId, previewScope) {
    return focusStructure({ node_id: nodeId, preview_scope: previewScope });
  }

  function readPresentationDeck() {
    const map =
      (globalThis.__mei && globalThis.__mei.presentation_map) ||
      (typeof window !== "undefined" && window.__mei && window.__mei.presentation_map) ||
      null;
    const deck = map && typeof map === "object" ? map.deck || map.presentation_deck : null;
    if (!deck || typeof deck !== "object") return null;
    const slides = Array.isArray(deck.slides) ? deck.slides : [];
    return {
      stageKind: String(deck.stageKind || deck.stage_kind || "presentation"),
      activeSlideId: String(deck.activeSlideId || deck.active_slide_id || "").trim(),
      slides: slides
        .map((slide, index) => ({
          id: String(slide?.id || "").trim(),
          title: slide?.title || null,
          chapter: slide?.chapter || null,
          pattern: slide?.pattern || null,
          order: Number.isFinite(slide?.order) ? Number(slide.order) : index,
        }))
        .filter((slide) => slide.id)
        .sort((a, b) => a.order - b.order),
    };
  }

  function deckPageIds() {
    const deck = readPresentationDeck();
    if (deck && deck.slides.length) {
      return deck.slides.map((slide) => slide.id);
    }
    return [];
  }

  function resolveDeckPageNode(pageId) {
    const normalized = String(pageId || "").trim();
    if (!normalized) return null;
    const selectors = [
      `[data-mei-ui-role="slide"][data-mei-panel-name="${CSS.escape(normalized)}"]`,
      `[data-mei-ui-role="slide"][data-mei-panel-name$="/${CSS.escape(normalized)}"]`,
      `[data-mei-panel-name="${CSS.escape(normalized)}"]`,
      `[data-mei-panel-name$="/${CSS.escape(normalized)}"]`,
      `[data-preview-scope$="/${CSS.escape(normalized)}"]`,
      `[data-preview-scope="${CSS.escape(normalized)}"]`,
      `[data-mei-panel-id$="/${CSS.escape(normalized)}"]`,
      `[data-mei-structure-label="${CSS.escape(normalized)}"]`,
    ];
    for (const selector of selectors) {
      const node = document.querySelector(selector);
      if (node instanceof HTMLElement) return node;
    }
    return null;
  }

  function listDeckPageNodes() {
    const nodes = [];
    const seen = new Set();
    for (const id of deckPageIds()) {
      const node = resolveDeckPageNode(id);
      if (!(node instanceof HTMLElement) || seen.has(node)) continue;
      seen.add(node);
      nodes.push(node);
    }
    if (nodes.length) return nodes;
    const slideNodes = Array.from(
      document.querySelectorAll('[data-mei-ui-role="slide"]'),
    ).filter((node) => node instanceof HTMLElement);
    if (slideNodes.length) return slideNodes;
    return [];
  }

  function currentDeckPageIndex() {
    const pages = listDeckPageNodes();
    const active = pages.findIndex((node) => !node.hasAttribute("hidden"));
    return active >= 0 ? active : 0;
  }

  function showDeckPage(pageIdOrIndex) {
    const pages = listDeckPageNodes();
    if (!pages.length) return false;
    let targetIndex = -1;
    if (typeof pageIdOrIndex === "number" && Number.isFinite(pageIdOrIndex)) {
      targetIndex = Math.max(0, Math.min(pages.length - 1, pageIdOrIndex));
    } else {
      const wanted = String(pageIdOrIndex || "").trim();
      targetIndex = pages.findIndex((node) => {
        const name = String(node.getAttribute("data-mei-panel-name") || "");
        const leaf = name.split("/").pop() || name;
        return name === wanted || name.endsWith(`/${wanted}`) || leaf === wanted;
      });
      if (targetIndex < 0) {
        const byId = resolveDeckPageNode(wanted);
        targetIndex = byId ? pages.indexOf(byId) : -1;
      }
    }
    if (targetIndex < 0) return false;
    pages.forEach((node, index) => {
      const active = index === targetIndex;
      node.toggleAttribute("hidden", !active);
      node.classList.toggle("mei-deck-page-active", active);
    });
    document.documentElement.setAttribute(
      "data-mei-active-deck-page",
      String(pages[targetIndex].getAttribute("data-mei-panel-name") || ""),
    );
    document.documentElement.setAttribute("data-mei-active-deck-page-index", String(targetIndex));
    return true;
  }

  function ensureDeckPageVisibility() {
    const pages = listDeckPageNodes();
    if (!pages.length) return false;
    const visible = pages.filter((node) => !node.hasAttribute("hidden"));
    if (visible.length === 1) return true;
    const deck = readPresentationDeck();
    if (deck?.activeSlideId) {
      if (showDeckPage(deck.activeSlideId)) return true;
    }
    return showDeckPage(0);
  }

  function showNextDeckPage() {
    const pages = listDeckPageNodes();
    if (!pages.length) return false;
    const next = Math.min(pages.length - 1, currentDeckPageIndex() + 1);
    return showDeckPage(next);
  }

  function showPrevDeckPage() {
    const pages = listDeckPageNodes();
    if (!pages.length) return false;
    const prev = Math.max(0, currentDeckPageIndex() - 1);
    return showDeckPage(prev);
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
      case "show_page":
      case "showPage":
        return showDeckPage(action.pageId || action.page || action.sectionId || action.id);
      case "highlight":
      case "focus": {
        const viewpointId = String(action.viewpoint || action.viewpointId || "").trim();
        const entry = readViewpointEntry(viewpointId);
        const focused = focusViewpoint(viewpointId);
        const dispatched = dispatchWorldTargetAction(action, entry);
        return focused || dispatched;
      }
      case "focus_structure":
      case "focusStructure":
        return focusStructure(action);
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
        const panelId = String(
          action.pagePanelId ||
            action.page_panel_id ||
            action.panelId ||
            action.panel_id ||
            "",
        ).trim();
        if (panelId) {
          return openT2Panel(panelId);
        }
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
    root.MeiPresentation.focusStructure = focusStructure;
    root.MeiPresentation.highlightStructureNode = highlightStructureNode;
    root.MeiPresentation.clearFocus = clearViewpointFocus;
    root.MeiPresentation.showPlane = (planeId) => setPlaneVisibility(planeId, true);
    root.MeiPresentation.hidePlane = (planeId) => setPlaneVisibility(planeId, false);
    root.MeiPresentation.resetPlanes = resetPlaneVisibility;
    root.MeiPresentation.showPage = showDeckPage;
    root.MeiPresentation.nextPage = showNextDeckPage;
    root.MeiPresentation.prevPage = showPrevDeckPage;
    root.MeiPresentation.ensureDeckPages = ensureDeckPageVisibility;
    root.MeiPresentation.listDeckPages = listDeckPageNodes;
    root.MeiPresentation.openT2Panel = openT2Panel;
    root.MeiPresentation.resetT2Panels = resetT2Panels;
    root.MeiPresentation.resetStages = resetStageVisibility;
    root.MeiPresentation.showStage = (stageKind) => setStageVisibility(stageKind, true);
    root.MeiPresentation.hideStage = (stageKind) => setStageVisibility(stageKind, false);
    root.MeiPresentation.dispatch = dispatchPresentationAction;
    root.MeiPresentation.map = readPresentationMap;
    root.MeiPresentation.resolveViewpoint = readViewpointEntry;
    root.MeiPresentation.zTiers = PRESENTATION_Z_TIERS;
    boot.dispatchPresentationAction = dispatchPresentationAction;
    boot.openT2Panel = openT2Panel;
    boot.showDeckPage = showDeckPage;
    boot.nextDeckPage = showNextDeckPage;
    boot.prevDeckPage = showPrevDeckPage;
    boot.ensureDeckPageVisibility = ensureDeckPageVisibility;
    resetPlaneVisibility();
    ensureDeckPageVisibility();
  }

  installFocusController();
