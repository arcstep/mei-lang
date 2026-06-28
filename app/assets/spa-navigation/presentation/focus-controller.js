  const PRESENTATION_Z_TIERS = {
    basemap: { min: 0, max: 9, default: 1 },
    chrome: { min: 100, max: 199, default: 100 },
    overlay: { min: 1000, max: 1999, default: 1000 },
    slide_layer: { min: 2000, max: 2999, default: 2000 },
    assistant: { min: 3000, max: 3999, default: 3000 },
  };

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

  function clearViewpointFocus() {
    document.querySelectorAll(".mei-viewpoint-focus").forEach((node) => {
      node.classList.remove("mei-viewpoint-focus");
    });
    document.documentElement.classList.remove("mei-tier-dim");
  }

  function focusViewpoint(viewpointId) {
    const map = readPresentationMap();
    const entry = map?.viewpoints?.[viewpointId];
    if (!entry) return false;
    clearViewpointFocus();
    const selector = `[data-mei-viewpoint="${CSS.escape(viewpointId)}"]`;
    const target = document.querySelector(selector);
    if (!(target instanceof HTMLElement)) return false;
    target.classList.add("mei-viewpoint-focus");
    if (entry.tier) {
      document.documentElement.classList.add("mei-tier-dim");
      target.dataset.meiFocusTier = entry.tier;
    }
    target.scrollIntoView({ block: "nearest", behavior: "smooth" });
    return true;
  }

  function installFocusController() {
    if (boot.focusControllerMounted) return;
    boot.focusControllerMounted = true;
    const root = typeof globalThis !== "undefined" ? globalThis : window;
    root.MeiPresentation = root.MeiPresentation || {};
    root.MeiPresentation.focus = focusViewpoint;
    root.MeiPresentation.clearFocus = clearViewpointFocus;
    root.MeiPresentation.map = readPresentationMap;
    root.MeiPresentation.zTiers = PRESENTATION_Z_TIERS;
  }

  installFocusController();
