(() => {
  const boot = (window.__meiLangBoot = window.__meiLangBoot || {});

  function engine() {
    return boot.presentationStepEngine || null;
  }

  function parseAppIdFromPath() {
    const match = String(window.location.pathname || "").match(
      /^\/apps\/(?:app|access|access-only|access_only|copilot|speaker|run)\/([^/]+)/,
    );
    return match ? String(match[1] || "").trim() : "";
  }

  async function compileEphemeralPresentation(source, options = {}) {
    const appId = String(options.appId || parseAppIdFromPath()).trim();
    if (!appId) {
      throw new Error("compileAndRunPresentation requires appId");
    }
    const payload = {
      source: String(source || ""),
      appId,
      sceneId: String(options.sceneId || "home").trim() || "home",
      presentationId: String(options.presentationId || "ephemeral").trim() || "ephemeral",
      mode: "ephemeral",
    };
    const response = await fetch("/api/presentation/compile", {
      method: "POST",
      credentials: "same-origin",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify(payload),
    });
    const result = await response.json().catch(() => ({}));
    if (!response.ok || !result?.manifest) {
      const message =
        result?.diagnostics?.[0]?.message ||
        `presentation compile failed: ${response.status}`;
      const error = new Error(message);
      error.payload = result;
      throw error;
    }
    return result;
  }

  async function compileAndRunPresentation(source, options = {}) {
    const eng = engine();
    if (!eng || typeof eng.runManifest !== "function") {
      throw new Error("presentation step engine is not ready");
    }
    const result = await compileEphemeralPresentation(source, options);
    const toolbar = boot.copilotToolbar;
    if (toolbar && typeof toolbar.mount === "function") {
      toolbar.mount({ autoStart: false, apply: false, toolbarOpen: true });
    }
    eng.runManifest(result.manifest, {
      source: "ephemeral",
      stepIndex: options.stepIndex,
      apply: options.apply,
    });
    if (toolbar && typeof toolbar.renderAll === "function") {
      toolbar.renderAll();
    }
    return result;
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
    const ctx = boot.copilotFabContext;
    if (ctx && typeof ctx.shouldMountCopilotToolbar === "function") {
      return ctx.shouldMountCopilotToolbar();
    }
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
      loadManifest: (manifest, options) => eng.loadManifest(manifest, options),
      runManifest: (manifest, options) => eng.runManifest(manifest, options),
      replaceManifest: (manifest, options) => eng.replaceManifest(manifest, options),
      clearEphemeralManifest: () => eng.clearEphemeralManifest(),
      compileAndRunPresentation: (source, options) => compileAndRunPresentation(source, options),
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
    boot.compileAndRunPresentation = compileAndRunPresentation;
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
    const eng = engine();
    if (eng && typeof eng.prefetchManifest === "function") {
      void eng.prefetchManifest();
    }
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
    if (eng && typeof eng.prefetchManifest === "function") {
      void eng.prefetchManifest();
    }
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
