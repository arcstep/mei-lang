(() => {
  const boot = (window.__meiLangBoot = window.__meiLangBoot || {});

  const MANIFEST_DOM_IDS = [
    "mei-presentation-manifest",
    "mei-copilot-tour",
    "mei-speaker-tour",
  ];
  const STORAGE_KEY = "mei_copilot_presentation_v1";
  const STORAGE_KEY_LEGACY = "mei_copilot_tour_v1";
  const SLIDE_LAYER_ID = "mei-copilot-slide-layer";
  const EPHEMERAL_SOURCE = "ephemeral";
  const LIBRARY_SOURCE = "library";

  const state = {
    manifest: null,
    manifestSource: "",
    steps: [],
    stepIndex: 0,
    sessionActive: false,
    everStarted: false,
  };

  function readManifestFromDom() {
    for (const id of MANIFEST_DOM_IDS) {
      const node = document.getElementById(id);
      if (!(node instanceof HTMLScriptElement) || !node.textContent) continue;
      try {
        return JSON.parse(node.textContent);
      } catch (_) {
        /* try next */
      }
    }
    return null;
  }

  function readStoredManifest() {
    for (const key of [STORAGE_KEY, STORAGE_KEY_LEGACY, "mei_speaker_tour_v1"]) {
      try {
        const raw = sessionStorage.getItem(key);
        if (!raw) continue;
        const parsed = JSON.parse(raw);
        if (
          parsed &&
          typeof parsed === "object" &&
          parsed.__meiPresentationSession === true &&
          parsed.manifest &&
          typeof parsed.manifest === "object"
        ) {
          return parsed;
        }
        if (parsed && typeof parsed === "object") {
          return {
            __meiPresentationSession: true,
            source: "legacy",
            manifest: parsed,
          };
        }
      } catch (_) {
        /* try next */
      }
    }
    return null;
  }

  function persistManifest(manifest, options = {}) {
    if (!manifest || typeof manifest !== "object") return;
    try {
      const source = String(options.source || "").trim() || "session";
      sessionStorage.setItem(
        STORAGE_KEY,
        JSON.stringify({
          __meiPresentationSession: true,
          source,
          manifest,
        }),
      );
    } catch (_) {
      /* ignore */
    }
  }

  function clearStoredManifest() {
    for (const key of [STORAGE_KEY, STORAGE_KEY_LEGACY, "mei_speaker_tour_v1"]) {
      try {
        sessionStorage.removeItem(key);
      } catch (_) {
        /* ignore */
      }
    }
  }

  function normalizeSteps(manifest) {
    if (!manifest || !Array.isArray(manifest.steps)) return [];
    return manifest.steps.filter((step) => step && typeof step === "object");
  }

  function shouldPrefetchPresentationAssets() {
    const mei = typeof window !== "undefined" ? window.__mei : null;
    if (mei && mei.presentation_manifest_prefetch === false) return false;
    if (mei && mei.presentation_manifest_prefetch === true) return true;
    return false;
  }

  let manifestFetchPromise = null;

  async function fetchManifestFromAssets() {
    return null;
  }

  function ensureLoadedAsync() {
    if (state.steps.length) return Promise.resolve(true);
    const stored = readStoredManifest();
    const manifest = readManifestFromDom() || stored?.manifest || null;
    if (loadManifest(manifest, { source: stored?.source || "dom" })) return Promise.resolve(true);
    if (!manifestFetchPromise) {
      manifestFetchPromise = fetchManifestFromAssets()
        .then((fetched) => {
          manifestFetchPromise = null;
          return loadManifest(fetched, { source: "prefetch" });
        })
        .catch(() => {
          manifestFetchPromise = null;
          return false;
        });
    }
    return manifestFetchPromise;
  }

  function injectManifestScript(manifest) {
    if (!manifest || typeof manifest !== "object") return;
    let node = document.getElementById("mei-presentation-manifest");
    if (!(node instanceof HTMLScriptElement)) {
      node = document.createElement("script");
      node.type = "application/json";
      node.id = "mei-presentation-manifest";
      (document.head || document.body || document.documentElement).appendChild(node);
    }
    node.textContent = JSON.stringify(manifest);
  }

  function loadManifest(manifest, options = {}) {
    const steps = normalizeSteps(manifest);
    if (!steps.length) return false;
    const source = String(options.source || "").trim() || state.manifestSource || "session";
    if (source === EPHEMERAL_SOURCE || source === LIBRARY_SOURCE) {
      clearStoredManifest();
    }
    state.manifest = manifest;
    state.manifestSource = source;
    state.steps = steps;
    injectManifestScript(manifest);
    persistManifest(manifest, { source: state.manifestSource });
    return true;
  }

  function ensureLoaded() {
    if (state.steps.length) return true;
    const stored = readStoredManifest();
    const manifest = readManifestFromDom() || stored?.manifest || null;
    return loadManifest(manifest, { source: stored?.source || "dom" });
  }

  function hasManifest() {
    return Boolean(readManifestFromDom() || state.steps.length || readStoredManifest()?.manifest);
  }

  function prefetchManifest() {
    if (hasManifest() || state.steps.length) return Promise.resolve(true);
    if (!shouldPrefetchPresentationAssets()) return Promise.resolve(false);
    return ensureLoadedAsync();
  }

  function routeUtils() {
    return boot.presentationRouteUtils || global.MeiPresentationRouteUtils || null;
  }

  function rewriteStepRoute(route) {
    const utils = routeUtils();
    if (utils?.rewriteStepRoute) return utils.rewriteStepRoute(route);
    if (typeof global.rewriteLegacyPresentationRoute === "function") {
      return global.rewriteLegacyPresentationRoute(route);
    }
    return String(route || "").trim();
  }

  function dispatchWorldAction(detail) {
    const utils = routeUtils();
    if (utils?.dispatchWorldAction) return utils.dispatchWorldAction(detail);
    try {
      global.dispatchEvent(
        new CustomEvent("mei:presentation-world-action", { detail, bubbles: false }),
      );
      return true;
    } catch (_) {
      return false;
    }
  }

  function focusApi() {
    return window.MeiPresentation || null;
  }

  function clearFocus() {
    const api = focusApi();
    if (api && typeof api.clearFocus === "function") {
      api.clearFocus();
    }
  }

  function resetPlanes() {
    const api = focusApi();
    if (api && typeof api.resetPlanes === "function") {
      api.resetPlanes();
    }
  }

  function applyHighlight(viewpointId) {
    if (!viewpointId) return false;
    const api = focusApi();
    if (api && typeof api.dispatch === "function") {
      return Boolean(api.dispatch({ type: "highlight", viewpoint: viewpointId }));
    }
    if (api && typeof api.focus === "function") {
      return Boolean(api.focus(viewpointId));
    }
    return false;
  }

  function applyPresentationAction(action) {
    if (!action || typeof action !== "object") return false;
    const normalized = normalizePresentationAction(action);
    const type = String(normalized.type || "").trim();
    const api = focusApi();
    if (api && typeof api.dispatch === "function") {
      if (api.dispatch(normalized)) return true;
    }
    if (type === "highlight" || type === "focus") {
      const viewpoint = String(normalized.viewpoint || "").trim();
      if (viewpoint && applyHighlight(viewpoint)) return true;
    }
    return dispatchWorldAction(normalized);
  }

  function normalizePresentationAction(action) {
    if (!action || typeof action !== "object") return {};
    const normalized = { ...action };
    if (normalized.type === "open_board") {
      normalized.type = "open_t2_page";
    }
    if (!normalized.pageSceneId && normalized.page_scene_id) {
      normalized.pageSceneId = normalized.page_scene_id;
    }
    if (!normalized.pageSceneId && normalized.boardSceneId) {
      normalized.pageSceneId = normalized.boardSceneId;
    }
    if (!normalized.viewpoint && normalized.viewpointId) {
      normalized.viewpoint = normalized.viewpointId;
    }
    if (!normalized.viewFamily && normalized.view_family) {
      normalized.viewFamily = normalized.view_family;
    }
    if (!normalized.worldRef && normalized.world_ref) {
      normalized.worldRef = normalized.world_ref;
    }
    if (!normalized.entityId && normalized.entity_id) {
      normalized.entityId = normalized.entity_id;
    }
    if (!normalized.groupId && normalized.group_id) {
      normalized.groupId = normalized.group_id;
    }
    if (!normalized.cameraPreset && normalized.camera_preset) {
      normalized.cameraPreset = normalized.camera_preset;
    }
    if (!normalized.plane && normalized.planeId) {
      normalized.plane = normalized.planeId;
    }
    return normalized;
  }

  function collectCockpitActions(step) {
    const fromCockpit = step?.cockpit?.actions;
    if (Array.isArray(fromCockpit) && fromCockpit.length) return fromCockpit;
    if (Array.isArray(step?.actions) && step.actions.length) return step.actions;
    return [];
  }

  function applyCockpitActions(step) {
    collectCockpitActions(step).forEach((action) => {
      applyPresentationAction(action);
    });
  }

  function resolveComposition(step) {
    const raw = String(step?.composition || "").trim();
    if (raw === "slides_only" || raw === "cockpit_only" || raw === "slides_over_cockpit") {
      return raw;
    }
    if (step?.slide && step?.cockpit) return "slides_over_cockpit";
    if (step?.slide) return "slides_only";
    return "cockpit_only";
  }

  function escapeHtml(value) {
    return String(value || "")
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;");
  }

  function sanitizeClassToken(value, fallback = "default") {
    const token = String(value || "")
      .trim()
      .toLowerCase()
      .replace(/[^a-z0-9_-]+/g, "-")
      .replace(/^-+|-+$/g, "");
    return token || fallback;
  }

  function renderSlideLayoutFromIr(slide) {
    const slots = Array.isArray(slide?.slots) ? slide.slots : [];
    const slotMap = new Map(slots.map((slot) => [slot?.name, slot]));
    const layoutId = String(slide?.layout || "stack").trim();
    const layoutClass = sanitizeClassToken(layoutId, "stack");
    const renderNamedSlot = (name, tag = "section") => {
      const slot = slotMap.get(name);
      const html = String(slot?.html || "").trim();
      if (!html) return "";
      return `<${tag} class="mei-presentation-slot mei-presentation-slot--${sanitizeClassToken(name)}" data-slot="${escapeHtml(name)}">${html}</${tag}>`;
    };
    if (layoutClass === "title-and-evidence") {
      return (
        `<article class="mei-presentation-layout mei-presentation-layout--${layoutClass}" data-layout="${escapeHtml(layoutId)}">` +
        `<div class="mei-presentation-layout-grid">` +
        `<header class="mei-presentation-layout-head">${renderNamedSlot("title", "div")}</header>` +
        `<section class="mei-presentation-layout-body">${renderNamedSlot("body", "div")}${renderNamedSlot("support", "div")}</section>` +
        `<aside class="mei-presentation-layout-evidence">${renderNamedSlot("evidence", "div")}</aside>` +
        `</div>` +
        `</article>`
      );
    }
    const generic = slots
      .map((slot) => {
        const slotName = String(slot?.name || "").trim();
        const html = String(slot?.html || "").trim();
        if (!slotName || !html) return "";
        return `<section class="mei-presentation-slot mei-presentation-slot--${sanitizeClassToken(slotName)}" data-slot="${escapeHtml(slotName)}">${html}</section>`;
      })
      .join("");
    return `<article class="mei-presentation-layout mei-presentation-layout--${layoutClass}" data-layout="${escapeHtml(layoutId)}">${generic}</article>`;
  }

  function slideHtml(step) {
    const slide = step?.slide;
    if (!slide || typeof slide !== "object") return "";
    if (slide.html) return String(slide.html);
    if (slide.layout && Array.isArray(slide.slots) && slide.slots.length) {
      return renderSlideLayoutFromIr(slide);
    }
    if (slide.markdown) return String(slide.markdown);
    if (slide.document) return `<p class="mei-copilot-slide-doc">${String(slide.document)}</p>`;
    return "";
  }

  function mountSlideLayer(layer) {
    if (!(layer instanceof HTMLElement)) {
      return layer;
    }
    if (typeof boot.mountPresentationInViewport === "function" && boot.mountPresentationInViewport(layer)) {
      return layer;
    }
    if (typeof boot.relocatePresentationInViewport === "function") {
      boot.relocatePresentationInViewport();
      if (layer.classList.contains("mei-presentation-in-viewport")) {
        return layer;
      }
    }
    if (layer.parentElement !== document.body) {
      document.body.appendChild(layer);
    }
    layer.classList.remove("mei-presentation-in-viewport");
    return layer;
  }

  function ensureSlideLayer() {
    let layer = document.getElementById(SLIDE_LAYER_ID);
    if (!layer) {
      layer = document.createElement("div");
      layer.id = SLIDE_LAYER_ID;
      layer.className = "mei-copilot-slide-layer";
      layer.setAttribute("hidden", "hidden");
    }
    return mountSlideLayer(layer);
  }

  function slideEmbedRuntime() {
    return boot.presentationSlideEmbedRuntime || null;
  }

  function hideSlideLayer() {
    const embedRuntime = slideEmbedRuntime();
    if (embedRuntime && typeof embedRuntime.unmountAll === "function") {
      embedRuntime.unmountAll();
    }
    const layer = document.getElementById(SLIDE_LAYER_ID);
    if (!layer) return;
    layer.setAttribute("hidden", "hidden");
    layer.innerHTML = "";
    document.body.classList.remove("mei-copilot-slide-active");
  }

  function showSlideLayer(step, composition) {
    const html = slideHtml(step);
    if (!html) {
      hideSlideLayer();
      return;
    }
    const layer = ensureSlideLayer();
    layer.innerHTML = `<div class="mei-copilot-slide-inner">${html}</div>`;
    layer.removeAttribute("hidden");
    if (!layer.classList.contains("mei-presentation-in-viewport")) {
      document.body.classList.add("mei-copilot-slide-active");
    }
    if (composition === "slides_over_cockpit") {
      layer.classList.add("mei-copilot-slide-layer--overlay");
    } else {
      layer.classList.remove("mei-copilot-slide-layer--overlay");
    }
    const embedRuntime = slideEmbedRuntime();
    if (embedRuntime && typeof embedRuntime.mountSlideEmbeds === "function") {
      void embedRuntime.mountSlideEmbeds(layer, step);
    }
  }

  async function navigateToStepRoute(step) {
    const route = rewriteStepRoute(String(step?.route || step?.cockpit?.route || "").trim());
    if (!route) return;
    if (typeof boot.navigateInternal === "function") {
      await boot.navigateInternal(route, false);
      return;
    }
    window.location.href = route;
  }

  function currentStep() {
    return state.steps[state.stepIndex] || null;
  }

  function findStepIndexById(stepId) {
    const normalized = String(stepId || "").trim();
    if (!normalized) return -1;
    return state.steps.findIndex((step) => String(step?.id || "").trim() === normalized);
  }

  function currentViewpoint(step) {
    const actions = collectCockpitActions(step || currentStep());
    for (const action of actions) {
      const type = String(action?.type || "").trim();
      if (
        action &&
        [
          "highlight",
          "focus",
          "focus_entity",
          "focusEntity",
          "camera_move",
          "cameraMove",
          "show_group",
          "showGroup",
          "hide_group",
          "hideGroup",
        ].includes(type) &&
        action.viewpoint
      ) {
        return String(action.viewpoint).trim();
      }
    }
    return "";
  }

  async function applyStep() {
    if (!ensureLoaded()) return false;
    const step = currentStep();
    if (!step) return false;
    const composition = resolveComposition(step);
    clearFocus();
    resetPlanes();
    if (composition === "slides_only") {
      hideSlideLayer();
      showSlideLayer(step, composition);
    } else if (composition === "cockpit_only") {
      hideSlideLayer();
      await navigateToStepRoute(step);
      applyCockpitActions(step);
    } else {
      await navigateToStepRoute(step);
      applyCockpitActions(step);
      showSlideLayer(step, composition);
    }
    if (boot.copilotToolbar && typeof boot.copilotToolbar.onStepApplied === "function") {
      boot.copilotToolbar.onStepApplied(step, composition, state);
    }
    return true;
  }

  function contextSnapshot() {
    const step = currentStep();
    const composition = step ? resolveComposition(step) : "";
    return {
      presentationId: String(state.manifest?.id || "").trim(),
      stepId: String(step?.id || "").trim(),
      stepIndex: state.stepIndex,
      composition,
      viewpoint: currentViewpoint(step),
      caption: step ? String(step.caption || step.title || "") : "",
      manifestSource: state.manifestSource,
    };
  }

  function resetManifestState() {
    state.manifest = null;
    state.manifestSource = "";
    state.steps = [];
    state.stepIndex = 0;
    state.sessionActive = false;
    state.everStarted = false;
  }

  function clearSessionManifest() {
    clearStoredManifest();
    const manifestNode = document.getElementById("mei-presentation-manifest");
    if (manifestNode) manifestNode.remove();
    resetManifestState();
    return true;
  }

  function clearEphemeralManifest() {
    const stored = readStoredManifest();
    const source = String(state.manifestSource || stored?.source || "").trim();
    if (source !== EPHEMERAL_SOURCE && source !== LIBRARY_SOURCE) return false;
    return clearSessionManifest();
  }

  function replaceManifest(manifest, options = {}) {
    const source = String(options.source || EPHEMERAL_SOURCE).trim() || EPHEMERAL_SOURCE;
    hideSlideLayer();
    clearFocus();
    resetPlanes();
    clearStoredManifest();
    resetManifestState();
    return loadManifest(manifest, { source });
  }

  function runManifest(manifest, options = {}) {
    if (!replaceManifest(manifest, options)) return false;
    return engine.start({
      apply: options.apply !== false,
      stepIndex: Number.isFinite(Number(options.stepIndex)) ? Number(options.stepIndex) : 0,
    });
  }

  const engine = {
    hasManifest,
    ensureLoaded,
    ensureLoadedAsync,
    prefetchManifest,
    loadManifest,
    runManifest,
    replaceManifest,
    clearEphemeralManifest,
    clearSessionManifest,
    applyStep,
    currentStep,
    currentViewpoint,
    resolveComposition,
    contextSnapshot,
    get state() {
      return state;
    },
    get steps() {
      return state.steps.slice();
    },
    get stepIndex() {
      return state.stepIndex;
    },
    set stepIndex(value) {
      if (!Number.isFinite(Number(value))) return;
      state.stepIndex = Math.max(0, Math.min(state.steps.length - 1, Number(value)));
    },
    isActive() {
      return state.sessionActive && state.steps.length > 0;
    },
    isPaused() {
      return state.everStarted && !state.sessionActive && state.steps.length > 0;
    },
    pause() {
      if (!ensureLoaded()) return false;
      state.sessionActive = false;
      hideSlideLayer();
      clearFocus();
      resetPlanes();
      return true;
    },
    resume() {
      if (!ensureLoaded() || !state.everStarted) return false;
      state.sessionActive = true;
      void applyStep();
      return true;
    },
    stop() {
      state.sessionActive = false;
      state.everStarted = false;
      state.stepIndex = 0;
      hideSlideLayer();
      clearFocus();
      resetPlanes();
      return true;
    },
    start(options) {
      const opts = options && typeof options === "object" ? options : {};
      if (!ensureLoaded()) return false;
      if (Number.isFinite(Number(opts.stepIndex))) {
        state.stepIndex = Math.max(0, Math.min(state.steps.length - 1, Number(opts.stepIndex)));
      }
      state.everStarted = true;
      state.sessionActive = true;
      if (opts.apply !== false) {
        void applyStep();
      }
      return true;
    },
    next() {
      if (!ensureLoaded()) return false;
      state.stepIndex = Math.min(state.steps.length - 1, state.stepIndex + 1);
      if (state.everStarted) state.sessionActive = true;
      void applyStep();
      return true;
    },
    prev() {
      if (!ensureLoaded()) return false;
      state.stepIndex = Math.max(0, state.stepIndex - 1);
      if (state.everStarted) state.sessionActive = true;
      void applyStep();
      return true;
    },
    applyStepAt(index) {
      if (!ensureLoaded()) return false;
      if (!Number.isFinite(Number(index))) return false;
      state.stepIndex = Math.max(0, Math.min(state.steps.length - 1, Number(index)));
      void applyStep();
      return true;
    },
    applyStepId(stepId) {
      if (!ensureLoaded()) return false;
      const nextIndex = findStepIndexById(stepId);
      if (nextIndex < 0) return false;
      state.stepIndex = nextIndex;
      if (state.everStarted) state.sessionActive = true;
      void applyStep();
      return true;
    },
    highlight(viewpointId) {
      return applyHighlight(String(viewpointId || "").trim());
    },
  };

  boot.presentationStepEngine = engine;
})();
