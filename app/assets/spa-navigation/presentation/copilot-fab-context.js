(() => {
  const boot = (window.__meiLangBoot = window.__meiLangBoot || {});

  function isExternalAiFab() {
    return Boolean(document.getElementById("access-external-ai-fab"));
  }

  function routeUtils() {
    return boot.presentationRouteUtils || global.MeiPresentationRouteUtils || null;
  }

  function isPresentationSurfaceRoute() {
    const utils = routeUtils();
    if (utils?.isPresentationSurfaceRoute) return utils.isPresentationSurfaceRoute();
    const path = String(window.location.pathname || "");
    return /^\/apps\/[^/]+\/app(?:\/|$)/.test(path);
  }

  function isAccessLikeRoute() {
    return isPresentationSurfaceRoute();
  }

  function hasCopilotShellMarkers() {
    return Boolean(
      document.getElementById("copilot-shell") ||
        document.getElementById("speaker-shell") ||
        document.getElementById("mei-presentation-manifest") ||
        document.getElementById("mei-copilot-tour") ||
        document.getElementById("mei-speaker-tour"),
    );
  }

  /** 内置 FAB 默认走 Copilot 演说工具条；仅外链 FAB（access_ai_external）例外。 */
  function copilotFabContextActive() {
    if (isExternalAiFab()) return false;
    if (!document.getElementById("access-chat-fab")) return false;
    if (isAccessLikeRoute() || hasCopilotShellMarkers()) return true;
    const eng = boot.presentationStepEngine;
    return !!(eng && typeof eng.hasManifest === "function" && eng.hasManifest());
  }

  function shouldMountCopilotToolbar() {
    return copilotFabContextActive();
  }

  boot.copilotFabContext = {
    isExternalAiFab,
    copilotFabContextActive,
    shouldMountCopilotToolbar,
  };
})();
