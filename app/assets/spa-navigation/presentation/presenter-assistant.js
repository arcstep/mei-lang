(() => {
  const boot = (window.__meiLangBoot = window.__meiLangBoot || {});
  if (boot.presenterAssistantMounted) return;
  boot.presenterAssistantMounted = true;

  const PANEL_ID = "mei-presenter-assistant";
  const CAPTION_ID = "mei-presenter-caption";

  function readTour() {
    const node = document.getElementById("mei-speaker-tour");
    if (!(node instanceof HTMLScriptElement) || !node.textContent) return null;
    try {
      return JSON.parse(node.textContent);
    } catch (_) {
      return null;
    }
  }

  function isSpeakerRoute() {
    return /^\/apps\/speaker\//.test(String(window.location.pathname || ""));
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
    if (!viewpointId) return;
    const api = focusApi();
    if (api && typeof api.dispatch === "function") {
      api.dispatch({ type: "highlight", viewpoint: viewpointId });
      return;
    }
    if (api && typeof api.focus === "function") {
      api.focus(viewpointId);
    }
  }

  function applyActions(actions) {
    if (!Array.isArray(actions)) return;
    actions.forEach((action) => {
      if (!action || typeof action !== "object") return;
      const type = String(action.type || "").trim();
      if (type === "highlight") {
        applyHighlight(String(action.viewpoint || "").trim());
      }
    });
  }

  async function navigateToStep(step) {
    const route = String(step?.route || "").trim();
    if (!route) return;
    if (typeof boot.navigateInternal === "function") {
      await boot.navigateInternal(route, false);
      return;
    }
    window.location.href = route;
  }

  function ensureCaption() {
    let node = document.getElementById(CAPTION_ID);
    if (node) return node;
    node = document.createElement("div");
    node.id = CAPTION_ID;
    node.className = "mei-presenter-caption";
    node.setAttribute("hidden", "hidden");
    document.body.appendChild(node);
    return node;
  }

  function ensurePanel() {
    let panel = document.getElementById(PANEL_ID);
    if (panel) return panel;
    panel = document.createElement("aside");
    panel.id = PANEL_ID;
    panel.className = "mei-presenter-assistant";
    panel.innerHTML =
      '<header class="mei-presenter-assistant-head">' +
      '<strong class="mei-presenter-assistant-title" data-presenter-title="true">演说助手</strong>' +
      '<span class="mei-presenter-assistant-progress" data-presenter-progress="true"></span>' +
      "</header>" +
      '<div class="mei-presenter-assistant-toolbar">' +
      '<button type="button" class="mei-presenter-btn" data-presenter-prev="true">上一步</button>' +
      '<button type="button" class="mei-presenter-btn" data-presenter-next="true">下一步</button>' +
      '<button type="button" class="mei-presenter-btn" data-presenter-caption-toggle="true">气泡</button>' +
      '<button type="button" class="mei-presenter-btn" data-presenter-select-toggle="true">组件选择</button>' +
      "</div>";
    panel.addEventListener("click", (event) => {
      const target = event.target;
      if (!(target instanceof HTMLElement)) return;
      if (target.dataset.presenterPrev === "true") {
        state.stepIndex = Math.max(0, state.stepIndex - 1);
        void applyStep();
      }
      if (target.dataset.presenterNext === "true") {
        state.stepIndex = Math.min(state.steps.length - 1, state.stepIndex + 1);
        void applyStep();
      }
      if (target.dataset.presenterCaptionToggle === "true") {
        state.captionVisible = !state.captionVisible;
        renderCaption();
      }
      if (target.dataset.presenterSelectToggle === "true") {
        state.selectMode = !state.selectMode;
        document.body.classList.toggle("mei-presenter-select-mode", state.selectMode);
        target.classList.toggle("is-active", state.selectMode);
      }
    });
    document.body.appendChild(panel);
    ensureCaption();
    return panel;
  }

  const state = {
    tour: null,
    steps: [],
    stepIndex: 0,
    captionVisible: true,
    selectMode: false,
  };

  function currentStep() {
    return state.steps[state.stepIndex] || null;
  }

  function renderCaption() {
    const caption = ensureCaption();
    const step = currentStep();
    const text = step ? String(step.caption || step.title || "") : "";
    if (!text || !state.captionVisible) {
      caption.setAttribute("hidden", "hidden");
      caption.textContent = "";
      return;
    }
    caption.removeAttribute("hidden");
    caption.textContent = text;
  }

  function renderPanel() {
    const panel = ensurePanel();
    const step = currentStep();
    const title = panel.querySelector("[data-presenter-title]");
    const progress = panel.querySelector("[data-presenter-progress]");
    if (title instanceof HTMLElement) {
      title.textContent = step?.title || state.tour?.title || "演说助手";
    }
    if (progress instanceof HTMLElement) {
      progress.textContent =
        state.steps.length > 0 ? `${state.stepIndex + 1} / ${state.steps.length}` : "";
    }
    renderCaption();
  }

  async function applyStep() {
    const step = currentStep();
    if (!step) return;
    clearFocus();
    await navigateToStep(step);
    applyActions(step.actions);
    renderPanel();
  }

  function bindSelectMode() {
    document.addEventListener(
      "click",
      (event) => {
        if (!state.selectMode) return;
        const target = event.target;
        if (!(target instanceof Element)) return;
        const host = target.closest("[data-mei-viewpoint]");
        if (!(host instanceof HTMLElement)) return;
        const viewpointId = String(host.dataset.meiViewpoint || "").trim();
        if (!viewpointId) return;
        event.preventDefault();
        event.stopPropagation();
        applyHighlight(viewpointId);
      },
      true,
    );
  }

  function exposeConsoleApi() {
    const root = window;
    root.MeiSpeaker = root.MeiSpeaker || {};
    root.MeiSpeaker.applyStep = (index) => {
      if (!Number.isFinite(Number(index))) return;
      state.stepIndex = Math.max(0, Math.min(state.steps.length - 1, Number(index)));
      void applyStep();
    };
    root.MeiSpeaker.highlight = applyHighlight;
    root.MeiSpeaker.next = () => {
      state.stepIndex = Math.min(state.steps.length - 1, state.stepIndex + 1);
      void applyStep();
    };
    root.MeiSpeaker.prev = () => {
      state.stepIndex = Math.max(0, state.stepIndex - 1);
      void applyStep();
    };
  }

  function init() {
    if (!isSpeakerRoute()) return;
    const tour = readTour();
    if (!tour || !Array.isArray(tour.steps) || !tour.steps.length) return;
    state.tour = tour;
    state.steps = tour.steps;
    state.stepIndex = 0;
    ensurePanel();
    bindSelectMode();
    exposeConsoleApi();
    void applyStep();
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", init, { once: true });
  } else {
    init();
  }

  document.addEventListener("mei:spa-navigation-complete", () => {
    if (!isSpeakerRoute()) return;
    const step = currentStep();
    if (step) {
      applyActions(step.actions);
      renderPanel();
    }
  });
})();
