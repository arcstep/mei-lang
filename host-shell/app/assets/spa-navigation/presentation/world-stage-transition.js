(() => {
  const boot = (window.__meiLangBoot = window.__meiLangBoot || {});
  const MIN_TRANSITION_MS = 520;
  const OVERLAY_ID = "mei-world-stage-transition";
  const BACK_NAV_ID = "mei-world-stage-back-nav";
  const FLOAT_NAV_ID = "mei-world-stage-floating-nav";
  let transitionInFlight = false;
  let worldChromeActive = false;

  function resolveMapCockpitHost() {
    const instances = boot.worldMapInstances;
    if (!instances || typeof instances.forEach !== "function") {
      return null;
    }
    let mapInstance = null;
    instances.forEach((instance) => {
      if (!mapInstance && instance?._layout?.cockpitBleed && instance?._layout?.focusInsetPx) {
        mapInstance = instance;
      }
    });
    return mapInstance;
  }

  function resolveMapCockpitLayout() {
    const mapInstance = resolveMapCockpitHost();
    if (!mapInstance || typeof window === "undefined") {
      return null;
    }
    const stage =
      mapInstance.closest?.(".preview-stage.preview-surface") ||
      document.querySelector(".preview-stage.preview-surface");
    if (!stage) {
      return null;
    }
    const rect = stage.getBoundingClientRect();
    const stageStyle = window.getComputedStyle(stage);
    const rootStyle = window.getComputedStyle(document.documentElement);
    const designW =
      Number.parseFloat(stageStyle.getPropertyValue("--mei-viewport-design-width")) ||
      Number.parseFloat(rootStyle.getPropertyValue("--mei-viewport-design-width")) ||
      1920;
    const designH =
      Number.parseFloat(stageStyle.getPropertyValue("--mei-viewport-design-height")) ||
      Number.parseFloat(rootStyle.getPropertyValue("--mei-viewport-design-height")) ||
      1080;
    const scale = Math.min(rect.width / designW, rect.height / designH);
    const contentW = designW * scale;
    const contentH = designH * scale;
    const offsetX = rect.left + (rect.width - contentW) / 2;
    const offsetY = rect.top + (rect.height - contentH) / 2;
    const focus = mapInstance._layout.focusInsetPx;
    const gap = 10;
    const apertureTop = offsetY + Number(focus.top) * scale;
    const apertureLeft = offsetX + Number(focus.left) * scale;
    const width = contentW - (Number(focus.left) + Number(focus.right)) * scale;
    const height = contentH - (Number(focus.top) + Number(focus.bottom)) * scale;
    const navTop = apertureTop + gap * scale;
    const right =
      window.innerWidth - (offsetX + contentW) + (Number(focus.right) + gap) * scale;
    const bottom = window.innerHeight - (apertureTop + height);
    return {
      apertureTop,
      navTop,
      right,
      left: apertureLeft,
      width,
      height,
      bottom,
      scale,
    };
  }

  function releaseMapToolFocus() {
    const active = document.activeElement;
    if (active instanceof HTMLElement) {
      if (
        active.closest("#mei-cockpit-map-tools-plane") ||
        active.closest(".mei-cockpit-floating-map-tools")
      ) {
        active.blur();
      }
    }
    if (typeof boot.setWorldStageInputPlaneActive === "function") {
      boot.setWorldStageInputPlaneActive(true);
    }
  }

  function hideMapFloatingChrome() {
    releaseMapToolFocus();
    document.querySelectorAll(".mei-cockpit-floating-map-tools").forEach((node) => {
      node.style.display = "none";
      node.style.pointerEvents = "none";
    });
    document.querySelectorAll("body > .maplibregl-popup.mei-cockpit-floating-tip").forEach((node) => {
      node.style.display = "none";
      node.style.pointerEvents = "none";
    });
  }

  function showMapFloatingChrome() {
    if (document.documentElement.classList.contains("mei-world-stage-active")) {
      return;
    }
    if (typeof boot.setWorldStageInputPlaneActive === "function") {
      boot.setWorldStageInputPlaneActive(false);
    }
    document.querySelectorAll(".mei-cockpit-floating-map-tools").forEach((node) => {
      node.style.display = "";
      node.style.pointerEvents = "";
    });
    boot.syncCockpitMapToolsOverlays?.();
  }

  function ensureOverlay() {
    let overlay = document.getElementById(OVERLAY_ID);
    if (overlay instanceof HTMLElement) {
      return overlay;
    }
    overlay = document.createElement("div");
    overlay.id = OVERLAY_ID;
    overlay.className = "mei-world-stage-transition";
    overlay.setAttribute("data-mei-overlay-role", "spa_loading");
    overlay.hidden = true;
    overlay.innerHTML = `
      <div class="mei-world-stage-transition-card" role="status" aria-live="polite">
        <div class="mei-world-stage-transition-spinner" aria-hidden="true"></div>
        <p class="mei-world-stage-transition-message"></p>
      </div>
    `;
    document.body.appendChild(overlay);
    return overlay;
  }

  function ensureFloatingNav() {
    let nav = document.getElementById(FLOAT_NAV_ID);
    if (nav instanceof HTMLElement) {
      return nav;
    }
    nav = document.createElement("div");
    nav.id = FLOAT_NAV_ID;
    nav.className = "mei-world-stage-floating-nav";
    nav.hidden = true;
    nav.innerHTML = `
      <div class="nav-group" role="group" aria-label="缩放">
        <button type="button" data-nav="zoom-in" title="放大" aria-label="放大">+</button>
        <button type="button" data-nav="zoom-out" title="缩小" aria-label="缩小">−</button>
      </div>
      <div class="nav-group" role="group" aria-label="旋转">
        <button type="button" data-nav="bearing-left" title="左转" aria-label="左转">↶</button>
        <button type="button" data-nav="bearing-right" title="右转" aria-label="右转">↷</button>
      </div>
      <div class="nav-group" role="group" aria-label="俯仰">
        <button type="button" data-nav="pitch-up" title="更俯视" aria-label="更俯视">⌃</button>
        <button type="button" data-nav="pitch-down" title="更平视" aria-label="更平视">⌄</button>
      </div>
      <div class="nav-group" role="group" aria-label="复位">
        <button type="button" data-nav="reset" title="复原视角" aria-label="复原视角">◎</button>
      </div>
      <p class="nav-hint">左拖有限平移 · 右键旋转俯仰 · 滚轮缩放（按键职责固定；也可用上方按钮）</p>
    `;
    nav.addEventListener("click", (event) => {
      const btn = event.target?.closest?.("[data-nav]");
      if (!btn) return;
      event.preventDefault();
      event.stopPropagation();
      const action = btn.getAttribute("data-nav");
      const api = boot.worldStageCameraNav;
      if (!api) return;
      switch (action) {
        case "zoom-in":
          api.zoomIn();
          break;
        case "zoom-out":
          api.zoomOut();
          break;
        case "bearing-left":
          api.rotateLeft();
          break;
        case "bearing-right":
          api.rotateRight();
          break;
        case "pitch-up":
          api.pitchUp();
          break;
        case "pitch-down":
          api.pitchDown();
          break;
        case "reset":
          api.reset();
          break;
        default:
          break;
      }
    });
    document.body.appendChild(nav);
    return nav;
  }

  function ensureBackNav() {
    let nav = document.getElementById(BACK_NAV_ID);
    if (nav instanceof HTMLElement) {
      return nav;
    }
    nav = document.createElement("div");
    nav.id = BACK_NAV_ID;
    nav.className = "mei-world-stage-back-nav";
    nav.hidden = true;
    nav.innerHTML = `
      <button type="button" class="mei-world-stage-back-btn" data-action="exit-map">
        返回地图总览
      </button>
    `;
    nav.querySelector(".mei-world-stage-back-btn")?.addEventListener("click", () => {
      const dispatch = boot.dispatchPresentationAction || window.MeiPresentation?.dispatch;
      if (typeof dispatch !== "function") return;
      dispatch({
        type: "exit_world_view",
        viewpoint: "park_overview_stage",
        viewFamily: "map",
        stageKind: "map-stage",
        cameraPreset: "park_overview_orbit",
      });
    });
    document.body.appendChild(nav);
    return nav;
  }

  function mountWorldStageChromeOnBody(node) {
    if (!(node instanceof HTMLElement)) {
      return;
    }
    if (node.parentElement !== document.body) {
      document.body.appendChild(node);
    }
    node.classList.remove("mei-cockpit-in-stage-shell");
    node.style.zIndex = "calc(var(--mei-z-cockpit-map-tools) + 4)";
  }

  function positionWorldChrome() {
    const host = resolveMapCockpitHost();
    const focus = host?._layout?.focusInsetPx;
    const gap = 10;
    const floatNav = ensureFloatingNav();
    floatNav.setAttribute("data-mei-overlay-role", "world_stage_tools");
    mountWorldStageChromeOnBody(floatNav);
    if (host && focus && typeof boot.positionFocusInsetTopRightFixed === "function") {
      boot.positionFocusInsetTopRightFixed(floatNav, host, focus, gap);
    } else if (host && focus && typeof boot.positionCockpitFloatingNav === "function") {
      boot.positionCockpitFloatingNav(floatNav, host, focus, gap);
    } else {
      const resolved = resolveMapCockpitLayout();
      if (!resolved) return;
      floatNav.style.position = "fixed";
      floatNav.style.top = `${Math.round(resolved.navTop)}px`;
      floatNav.style.right = `${Math.round(resolved.right)}px`;
      floatNav.style.left = "auto";
      floatNav.style.bottom = "auto";
    }
    const backNav = ensureBackNav();
    backNav.setAttribute("data-mei-overlay-role", "world_stage_tools");
    mountWorldStageChromeOnBody(backNav);
    const resolved = resolveMapCockpitLayout();
    if (resolved) {
      backNav.style.position = "fixed";
      backNav.style.left = `${Math.round(resolved.left + resolved.width / 2)}px`;
      backNav.style.bottom = `${Math.round(resolved.bottom + 16)}px`;
      backNav.style.transform = "translateX(-50%)";
      backNav.style.top = "auto";
      backNav.style.right = "auto";
    } else if (host && focus && typeof boot.positionFocusInsetBottomCenter === "function") {
      boot.positionFocusInsetBottomCenter(backNav, host, focus, 16);
    }
  }

  function activateWorldChrome() {
    if (worldChromeActive) {
      return;
    }
    worldChromeActive = true;
    hideMapFloatingChrome();
    positionWorldChrome();
    const floatNav = ensureFloatingNav();
    floatNav.hidden = false;
    floatNav.classList.add("is-visible");
    showBackNav();
  }

  function deactivateWorldChrome() {
    if (!worldChromeActive) {
      return;
    }
    worldChromeActive = false;
    const floatNav = document.getElementById(FLOAT_NAV_ID);
    if (floatNav instanceof HTMLElement) {
      floatNav.classList.remove("is-visible");
      floatNav.hidden = true;
    }
    hideBackNav();
    showMapFloatingChrome();
  }

  function setOverlayMessage(message) {
    const overlay = ensureOverlay();
    const text = overlay.querySelector(".mei-world-stage-transition-message");
    if (text) {
      text.textContent = String(message || "").trim() || "正在切换场景…";
    }
  }

  function showOverlay(message) {
    const overlay = ensureOverlay();
    setOverlayMessage(message);
    overlay.hidden = false;
    overlay.classList.add("is-visible");
    document.documentElement.classList.add("mei-world-stage-transitioning");
  }

  function hideOverlay() {
    const overlay = document.getElementById(OVERLAY_ID);
    if (!(overlay instanceof HTMLElement)) return;
    overlay.classList.remove("is-visible");
    overlay.hidden = true;
    document.documentElement.classList.remove("mei-world-stage-transitioning");
  }

  function showBackNav() {
    const nav = ensureBackNav();
    nav.hidden = false;
    nav.classList.add("is-visible");
    positionWorldChrome();
  }

  function hideBackNav() {
    const nav = document.getElementById(BACK_NAV_ID);
    if (!(nav instanceof HTMLElement)) return;
    nav.classList.remove("is-visible");
    nav.hidden = true;
  }

  function waitMinDuration(startedAt, minMs = MIN_TRANSITION_MS) {
    const elapsed = Date.now() - startedAt;
    const remain = Math.max(0, minMs - elapsed);
    if (!remain) {
      return Promise.resolve();
    }
    return new Promise((resolve) => {
      window.setTimeout(resolve, remain);
    });
  }

  async function runTransition(message, work) {
    if (transitionInFlight) {
      return false;
    }
    transitionInFlight = true;
    const startedAt = Date.now();
    showOverlay(message);
    try {
      const result = await Promise.resolve(work());
      await waitMinDuration(startedAt);
      return result;
    } finally {
      hideOverlay();
      transitionInFlight = false;
    }
  }

  async function runEnter(action, performEnter) {
    const label = String(action?.worldEnterLabel || action?.label || action?.entityId || "空间场景").trim();
    const message = `正在进入${label}…`;
    return runTransition(message, () => {
      const ok = performEnter(action);
      if (ok !== false) {
        window.requestAnimationFrame(() => activateWorldChrome());
      }
      return ok;
    });
  }

  async function runExit(action, performExit) {
    return runTransition("正在返回地图总览…", () => {
      deactivateWorldChrome();
      return performExit(action);
    });
  }

  function onResize() {
    if (!document.documentElement.classList.contains("mei-world-stage-active")) {
      return;
    }
    positionWorldChrome();
  }

  function installWorldStageTransition() {
    if (boot.worldStageTransitionMounted) return;
    boot.worldStageTransitionMounted = true;
    ensureBackNav();
    ensureFloatingNav();
    boot.worldStageTransition = {
      MIN_TRANSITION_MS,
      get transitionInFlight() {
        return transitionInFlight;
      },
      runTransition,
      runEnter,
      runExit,
      showBackNav,
      hideBackNav,
      activateWorldChrome,
      deactivateWorldChrome,
    };
    window.addEventListener("mei:world-stage-entered", () => {
      activateWorldChrome();
    });
    window.addEventListener("mei:world-stage-exited", () => {
      deactivateWorldChrome();
    });
    window.addEventListener("resize", onResize);
    window.addEventListener("pageshow", onResize);
  }

  installWorldStageTransition();
})();
