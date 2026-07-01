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

  const state = {
    manifest: null,
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
        if (parsed && typeof parsed === "object") return parsed;
      } catch (_) {
        /* try next */
      }
    }
    return null;
  }

  function persistManifest(manifest) {
    if (!manifest || typeof manifest !== "object") return;
    try {
      sessionStorage.setItem(STORAGE_KEY, JSON.stringify(manifest));
    } catch (_) {
      /* ignore */
    }
  }

  function normalizeSteps(manifest) {
    if (!manifest || !Array.isArray(manifest.steps)) return [];
    return manifest.steps.filter((step) => step && typeof step === "object");
  }

  function loadManifest(manifest) {
    const steps = normalizeSteps(manifest);
    if (!steps.length) return false;
    state.manifest = manifest;
    state.steps = steps;
    state.sessionActive = true;
    persistManifest(manifest);
    return true;
  }

  function ensureLoaded() {
    if (state.steps.length) return true;
    const manifest = readManifestFromDom() || readStoredManifest();
    return loadManifest(manifest);
  }

  function hasManifest() {
    return Boolean(readManifestFromDom() || state.steps.length || readStoredManifest());
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
    const type = String(action.type || "").trim();
    const api = focusApi();
    if (!api || typeof api.dispatch !== "function") {
      if (type === "highlight") return applyHighlight(String(action.viewpoint || "").trim());
      return false;
    }
    return Boolean(api.dispatch(action));
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

  function slideHtml(step) {
    const slide = step?.slide;
    if (!slide || typeof slide !== "object") return "";
    if (slide.html) return String(slide.html);
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

  function hideSlideLayer() {
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
  }

  async function navigateToStepRoute(step) {
    const route = String(step?.route || step?.cockpit?.route || "").trim();
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

  function currentViewpoint(step) {
    const actions = collectCockpitActions(step || currentStep());
    for (const action of actions) {
      if (action && String(action.type || "") === "highlight" && action.viewpoint) {
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
    };
  }

  const engine = {
    hasManifest,
    ensureLoaded,
    loadManifest,
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
    highlight(viewpointId) {
      return applyHighlight(String(viewpointId || "").trim());
    },
  };

  boot.presentationStepEngine = engine;
})();
