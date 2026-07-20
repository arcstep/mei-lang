(() => {
  const boot = (window.__meiLangBoot = window.__meiLangBoot || {});

  function isExternalAiFab() {
    return Boolean(document.getElementById("access-external-ai-fab"));
  }

  /** `features.copilotFab`（runtime capabilities / body data-mei-copilot-fab）；缺省启用。 */
  function isCopilotFabEnabled() {
    if (isExternalAiFab()) return true;
    const bodyFlag = document.body?.getAttribute("data-mei-copilot-fab");
    if (bodyFlag === "0" || bodyFlag === "false") return false;
    if (bodyFlag === "1" || bodyFlag === "true") return true;
    try {
      const el = document.getElementById("mei-host-runtime-capabilities");
      const raw = el?.textContent?.trim();
      if (raw) {
        const caps = JSON.parse(raw);
        if (
          caps?.features &&
          Object.prototype.hasOwnProperty.call(caps.features, "copilotFab")
        ) {
          return caps.features.copilotFab !== false;
        }
      }
    } catch (_) {
      /* ignore */
    }
    return true;
  }

  function routeUtils() {
    return boot.presentationRouteUtils || window.MeiPresentationRouteUtils || null;
  }

  function isPresentationSurfaceRoute() {
    const utils = routeUtils();
    if (utils?.isPresentationSurfaceRoute) return utils.isPresentationSurfaceRoute();
    const path = String(window.location.pathname || "");
    return /^\/apps\/[^/]+(?:\/[^/]+)?(?:\/|$)/.test(path);
  }

  function floatingRoot() {
    return document.getElementById("access-chat-floating-root");
  }

  function fabButton() {
    return document.getElementById("access-chat-fab");
  }

  /** Thin Access shell 若缺 FAB DOM，补挂最小结构（与 host thin shell SSR 对齐）。 */
  function ensureFabDom() {
    if (!isCopilotFabEnabled()) return null;
    if (isExternalAiFab() || fabButton()) return fabButton();
    let root = floatingRoot();
    if (!(root instanceof HTMLElement)) {
      root = document.createElement("div");
      root.id = "access-chat-floating-root";
      root.className = "access-chat-floating-root";
      root.setAttribute("data-open", "false");
      root.setAttribute("data-mei-stage-kind", "scene");
      root.setAttribute("data-mei-fab-policy", "required");
      document.body.appendChild(root);
    }
    const fab = document.createElement("button");
    fab.id = "access-chat-fab";
    fab.className = "access-chat-fab";
    fab.type = "button";
    fab.setAttribute("aria-label", "展开 Copilot 工具条");
    fab.title = "展开 Copilot 工具条";
    fab.setAttribute("data-mei-fab-policy", "required");
    const icon = document.createElement("img");
    icon.className = "access-chat-fab-icon";
    icon.src = "/app-assets/favicon.svg";
    icon.alt = "";
    fab.appendChild(icon);
    root.appendChild(fab);
    return fab;
  }

  function parseSceneIdFromPath() {
    const path = String(window.location.pathname || "");
    const stageMatch = path.match(/^\/apps\/[^/]+\/([^/?#]+)/);
    if (stageMatch) {
      const seg = String(stageMatch[1] || "").trim();
      const reserved = new Set([
        "view",
        "layout",
        "prototype",
        "app",
        "access",
        "build",
        "manage",
      ]);
      if (seg && !reserved.has(seg.toLowerCase())) return seg;
    }
    const match = path.match(/\/scene\/([^/?#]+)/);
    if (match) return String(match[1] || "").trim();
    const mei = window.__mei;
    return String(mei?.active_stage_id || mei?.active_stage || mei?.active_scene_id || mei?.activeSceneId || "home").trim() || "home";
  }

  function resolveStageKind() {
    const mei = window.__mei;
    const sceneId = parseSceneIdFromPath();
    // Phase 5: prefer stage_registry, then scene_routes.
    const stages = Array.isArray(mei?.stage_registry?.stages)
      ? mei.stage_registry.stages
      : [];
    const reg = stages.find((entry) => String(entry?.stage_id || "") === sceneId);
    if (reg) {
      const profile = String(reg.profile || "").toLowerCase();
      if (profile === "slides") return "presentation";
      if (profile === "cockpit") return "scene";
      const surface = String(reg.surface || "").toLowerCase();
      if (surface === "paged") return "presentation";
      return "scene";
    }
    const programs = mei?.stage_programs || {};
    const program = programs[sceneId];
    if (program) {
      if (String(program.profile || "") === "slides") return "presentation";
      if (String(program.surface || "") === "paged") return "presentation";
      return "scene";
    }
    const routes = Array.isArray(mei?.scene_routes) ? mei.scene_routes : [];
    const route = routes.find((entry) => String(entry?.scene_id || "") === sceneId) || null;
    if (route) {
      const kind = String(route?.kind || "").trim().toLowerCase();
      if (kind === "presentation" || kind === "scene") return kind;
      const target = String(route?.target_file || "").replace(/\\/g, "/").toLowerCase();
      if (target.includes("/presentation/")) return "presentation";
      return "scene";
    }
    const path = String(window.location.pathname || "");
    if (/\/presentation\//.test(path)) return "presentation";
    const targetFile = String(
      document.querySelector("[data-target-file]")?.getAttribute("data-target-file") || "",
    )
      .replace(/\\/g, "/")
      .toLowerCase();
    if (targetFile.includes("/presentation/")) return "presentation";
    return "scene";
  }

  /** 与讲稿路径推导的 target 对齐：scene/home | presentation/supervision */
  function resolveStageTargetKey() {
    const sceneId = parseSceneIdFromPath();
    const kind = resolveStageKind();
    return `${kind}/${sceneId}`;
  }

  function fabPolicy() {
    return isCopilotFabEnabled() ? "required" : "off";
  }

  function syncFabVisibility() {
    if (!isCopilotFabEnabled()) {
      const fab = fabButton();
      if (fab instanceof HTMLElement) {
        fab.hidden = true;
        fab.setAttribute("hidden", "");
      }
      const root = floatingRoot();
      if (root) {
        root.setAttribute("data-mei-fab-policy", "off");
        root.setAttribute("data-mei-fab-visible", "false");
      }
      return;
    }
    ensureFabDom();
    const fab = fabButton();
    if (!(fab instanceof HTMLElement) || isExternalAiFab()) return;
    fab.hidden = false;
    fab.removeAttribute("hidden");
    const root = floatingRoot();
    if (root) {
      root.setAttribute("data-mei-stage-kind", resolveStageKind());
      root.setAttribute("data-mei-fab-policy", "required");
      root.setAttribute("data-mei-stage-target", resolveStageTargetKey());
      root.setAttribute("data-mei-fab-visible", "true");
    }
  }

  /** Access 面：FAB 常显，工具条可点开；讲稿可选。 */
  function copilotFabContextActive() {
    if (isExternalAiFab()) return false;
    if (!ensureFabDom()) return false;
    syncFabVisibility();
    return isPresentationSurfaceRoute();
  }

  function shouldMountCopilotToolbar() {
    return copilotFabContextActive();
  }

  function revealFabForScript() {
    syncFabVisibility();
  }

  function parseAppIdFromPath() {
    const utils = routeUtils();
    if (utils?.parsePresentationAppId) {
      return String(utils.parsePresentationAppId() || "").trim();
    }
    const match = String(window.location.pathname || "").match(/^\/apps\/([^/]+)(?:\/|$)/);
    return match && match[1] ? match[1] : "";
  }

  function fabPositionStorageKey() {
    const appId = parseAppIdFromPath() || "default";
    return `mei-lang.agent.access-floating-position.${appId}`;
  }

  /** agent-panel 已接管 FAB 时不再重复绑定（完整 shell 有 meilang-author-panel）。 */
  function agentPanelOwnsFab() {
    return Boolean(boot.agentPanelState) || Boolean(document.getElementById("meilang-author-panel"));
  }

  function activateFabTap() {
    const toolbar = boot.copilotToolbar;
    if (toolbar && typeof toolbar.mount === "function" && !toolbar.uiState?.mounted) {
      toolbar.mount({ autoStart: false, apply: false, toolbarOpen: false });
    }
    if (toolbar && typeof toolbar.toggleToolbar === "function") {
      toolbar.toggleToolbar();
    }
  }

  let fabDragState = null;
  const FAB_DRAG_THRESHOLD_PX = 4;

  function floatingBoundsHost(root) {
    if (typeof boot.copilotFloatingOffsetParent === "function") {
      const host = boot.copilotFloatingOffsetParent(root);
      if (host) return host;
    }
    return root?.parentElement || null;
  }

  function applyFabShellPosition(root, left, top) {
    if (!(root instanceof HTMLElement)) return null;
    if (root.classList.contains("mei-copilot-in-viewport")) {
      const toDesign =
        typeof boot.shellToViewportFabDesign === "function"
          ? boot.shellToViewportFabDesign(left, top)
          : { left, top };
      if (typeof boot.applyViewportFabDesignPosition === "function") {
        return boot.applyViewportFabDesignPosition(root, toDesign.left, toDesign.top);
      }
    }
    const width = Math.max(48, Number(root.offsetWidth || 68));
    const height = Math.max(48, Number(root.offsetHeight || 68));
    const host = floatingBoundsHost(root);
    const hostRect = host
      ? host.getBoundingClientRect()
      : { left: 0, top: 0, width: window.innerWidth || 0, height: window.innerHeight || 0 };
    const margin = 10;
    const maxLeft = Math.max(margin, Number(hostRect.width || 0) - width - margin);
    const maxTop = Math.max(margin, Number(hostRect.height || 0) - height - margin);
    const nextLeft = Math.min(maxLeft, Math.max(margin, Math.round(Number(left) || 0)));
    const nextTop = Math.min(maxTop, Math.max(margin, Math.round(Number(top) || 0)));
    root.style.left = `${nextLeft}px`;
    root.style.top = `${nextTop}px`;
    root.style.right = "auto";
    root.style.bottom = "auto";
    root.dataset.positioned = "true";
    return { left: nextLeft, top: nextTop };
  }

  function rememberFabPosition(root) {
    if (!(root instanceof HTMLElement)) return;
    try {
      if (root.classList.contains("mei-copilot-in-viewport")) {
        const designLeft = Number(root.dataset.fabDesignLeft);
        const designTop = Number(root.dataset.fabDesignTop);
        if (!Number.isFinite(designLeft) || !Number.isFinite(designTop)) return;
        localStorage.setItem(
          fabPositionStorageKey(),
          JSON.stringify({ viewportDesign: true, designLeft, designTop }),
        );
        return;
      }
      const left = Number.parseFloat(root.style.left);
      const top = Number.parseFloat(root.style.top);
      if (!Number.isFinite(left) || !Number.isFinite(top)) return;
      localStorage.setItem(fabPositionStorageKey(), JSON.stringify({ left, top }));
    } catch (_) {
      /* ignore */
    }
  }

  function restoreFabPosition() {
    const root = floatingRoot();
    if (!(root instanceof HTMLElement)) return;
    try {
      const raw = localStorage.getItem(fabPositionStorageKey());
      if (!raw) return;
      const parsed = JSON.parse(raw);
      if (root.classList.contains("mei-copilot-in-viewport") && parsed?.viewportDesign === true) {
        if (typeof boot.applyViewportFabDesignPosition === "function") {
          boot.applyViewportFabDesignPosition(root, parsed.designLeft, parsed.designTop);
        }
        return;
      }
      if (Number.isFinite(Number(parsed?.left)) && Number.isFinite(Number(parsed?.top))) {
        applyFabShellPosition(root, parsed.left, parsed.top);
      }
    } catch (_) {
      /* ignore */
    }
  }

  function onFabPointerDown(event) {
    if (agentPanelOwnsFab()) return;
    if (event && event.button != null && event.button !== 0) return;
    const root = floatingRoot();
    const fab = fabButton();
    if (!(root instanceof HTMLElement) || !(fab instanceof HTMLElement)) return;
    const host = floatingBoundsHost(root);
    const hostRect = host ? host.getBoundingClientRect() : { left: 0, top: 0 };
    const rect = root.getBoundingClientRect();
    const baseLeft = Number(rect.left || 0) - Number(hostRect.left || 0);
    const baseTop = Number(rect.top || 0) - Number(hostRect.top || 0);
    fabDragState = {
      pointerId: event?.pointerId ?? null,
      startX: Number(event?.clientX),
      startY: Number(event?.clientY),
      baseLeft,
      baseTop,
      moved: false,
      lastLeft: baseLeft,
      lastTop: baseTop,
    };
    root.dataset.dragging = "true";
  }

  function onFabPointerMove(event) {
    if (!fabDragState || agentPanelOwnsFab()) return;
    if (
      fabDragState.pointerId != null &&
      event?.pointerId != null &&
      event.pointerId !== fabDragState.pointerId
    ) {
      return;
    }
    const nextX = Number(event?.clientX);
    const nextY = Number(event?.clientY);
    if (!Number.isFinite(nextX) || !Number.isFinite(nextY)) return;
    const dx = nextX - fabDragState.startX;
    const dy = nextY - fabDragState.startY;
    if (!fabDragState.moved && Math.hypot(dx, dy) < FAB_DRAG_THRESHOLD_PX) return;
    const root = floatingRoot();
    const fab = fabButton();
    if (!(root instanceof HTMLElement)) return;
    if (!fabDragState.moved) {
      fabDragState.moved = true;
      try {
        if (fab && event?.pointerId != null) fab.setPointerCapture(event.pointerId);
      } catch (_) {
        /* ignore */
      }
    }
    const pos = applyFabShellPosition(
      root,
      fabDragState.baseLeft + dx,
      fabDragState.baseTop + dy,
    );
    if (!pos) return;
    fabDragState.lastLeft = pos.left;
    fabDragState.lastTop = pos.top;
    if (typeof event?.preventDefault === "function") event.preventDefault();
  }

  function onFabPointerUp(event) {
    if (!fabDragState) return;
    if (
      fabDragState.pointerId != null &&
      event?.pointerId != null &&
      event.pointerId !== fabDragState.pointerId
    ) {
      return;
    }
    const moved = !!fabDragState.moved;
    const root = floatingRoot();
    const fab = fabButton();
    fabDragState = null;
    if (root) delete root.dataset.dragging;
    try {
      if (fab && event?.pointerId != null) fab.releasePointerCapture(event.pointerId);
    } catch (_) {
      /* ignore */
    }
    if (moved) {
      rememberFabPosition(root);
      const layout = boot.copilotFabLayout;
      if (layout && typeof layout.scheduleCopilotFabToolbarLayout === "function") {
        layout.scheduleCopilotFabToolbarLayout();
      }
      return;
    }
    activateFabTap();
  }

  function installFabInteraction() {
    if (boot.copilotFabInteractionBound || isExternalAiFab()) return false;
    if (agentPanelOwnsFab()) return false;
    const fab = ensureFabDom();
    if (!(fab instanceof HTMLElement)) return false;
    boot.copilotFabInteractionBound = true;
    fab.addEventListener("pointerdown", onFabPointerDown);
    document.addEventListener("pointermove", onFabPointerMove);
    document.addEventListener("pointerup", onFabPointerUp);
    document.addEventListener("pointercancel", onFabPointerUp);
    restoreFabPosition();
    if (typeof boot.activateAccessFabTap !== "function") {
      boot.activateAccessFabTap = activateFabTap;
    }
    return true;
  }

  boot.copilotFabContext = {
    isExternalAiFab,
    isCopilotFabEnabled,
    resolveStageKind,
    resolveStageTargetKey,
    parseSceneIdFromPath,
    fabPolicy,
    syncFabVisibility,
    revealFabForScript,
    copilotFabContextActive,
    shouldMountCopilotToolbar,
    ensureFabDom,
    installFabInteraction,
    activateFabTap,
    restoreFabPosition,
  };

  function bootFabChrome() {
    syncFabVisibility();
    installFabInteraction();
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", bootFabChrome, { once: true });
  } else {
    bootFabChrome();
  }
  window.addEventListener("mei:spa-navigated", bootFabChrome);
  document.addEventListener("mei:spa-navigation-complete", () => {
    if (typeof boot.ensureDeckPageVisibility === "function") {
      boot.ensureDeckPageVisibility();
    }
    bootFabChrome();
  });
  window.addEventListener("meilang:viewport-stage-ready", () => {
    restoreFabPosition();
    installFabInteraction();
  });
})();
