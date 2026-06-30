(() => {
  const boot = (window.__meiLangBoot = window.__meiLangBoot || {});

  function engine() {
    return boot.presentationStepEngine || null;
  }

  function isCopilotRoute() {
    return /^\/apps\/(copilot|speaker)\//.test(String(window.location.pathname || ""));
  }

  function hasCopilotShell() {
    return Boolean(
      document.getElementById("copilot-shell") ||
        document.getElementById("speaker-shell") ||
        document.getElementById("mei-presentation-manifest") ||
        document.getElementById("mei-copilot-tour") ||
        document.getElementById("mei-speaker-tour"),
    );
  }

  function shouldAutoStart() {
    const eng = engine();
    return Boolean((eng && eng.hasManifest()) || isCopilotRoute() || hasCopilotShell());
  }

  function exposeConsoleApi() {
    const eng = engine();
    if (!eng) return;
    const api = {
      start: (options) => eng.start(options),
      applyStep: (index) => eng.applyStepAt(index),
      highlight: (viewpointId) => eng.highlight(viewpointId),
      next: () => eng.next(),
      prev: () => eng.prev(),
      stepIndex: () => eng.stepIndex,
      steps: () => eng.steps,
      isActive: () => eng.isActive(),
      isPaused: () => eng.isPaused(),
      pause: () => eng.pause(),
      resume: () => eng.resume(),
      stop: () => eng.stop(),
      context: () => eng.contextSnapshot(),
      hasManifest: () => eng.hasManifest(),
    };
    window.MeiCopilot = api;
    window.MeiSpeaker = api;
    boot.startCopilot = (options) => {
      const toolbar = boot.copilotToolbar;
      if (toolbar && typeof toolbar.mount === "function") {
        toolbar.mount({ autoStart: false, apply: false });
      }
      return eng.start(options);
    };
    boot.startPresenterAssistant = boot.startCopilot;
  }

  function init() {
    exposeConsoleApi();
    const toolbar = boot.copilotToolbar;
    if (toolbar && typeof toolbar.mount === "function" && shouldAutoStart()) {
      toolbar.mount({ autoStart: false, apply: false, toolbarOpen: false });
      exposeConsoleApi();
    }
  }

  function onSpaNavigationComplete() {
    const eng = engine();
    const toolbar = boot.copilotToolbar;
    if (!eng) return;
    if (toolbar && typeof toolbar.mount === "function" && shouldAutoStart()) {
      toolbar.mount({ autoStart: false, toolbarOpen: false });
      exposeConsoleApi();
    }
    if (!eng.isActive()) return;
    if (toolbar && typeof toolbar.renderAll === "function") {
      toolbar.renderAll();
    }
    const step = eng.currentStep();
    if (step) {
      void eng.applyStep();
    }
  }

  exposeConsoleApi();

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", init, { once: true });
  } else {
    init();
  }

  document.addEventListener("mei:spa-navigation-complete", onSpaNavigationComplete);
})();
