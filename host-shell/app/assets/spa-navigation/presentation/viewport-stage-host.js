(() => {
  const boot = (window.__meiLangBoot = window.__meiLangBoot || {});
  const PRESENTATION_PLANE_ID = "mei-presentation-plane";
  const COPILOT_PLANE_ID = "mei-copilot-plane";
  const PRESENTATION_NODE_IDS = ["mei-copilot-slide-layer"];
  const COPILOT_PLANE_NODE_IDS = [
    "access-external-ai-floating-root",
    "mei-copilot-caption",
    "copilot-script-drawer",
  ];
  const ACCESS_CHAT_ROOT_ID = "access-chat-floating-root";
  const ACCESS_CHAT_OVERLAY_ID = "access-chat-overlay-panel";

  function accessChatOverlayOpen() {
    const root = document.getElementById(ACCESS_CHAT_ROOT_ID);
    const panel = document.getElementById(ACCESS_CHAT_OVERLAY_ID);
    if (!(root instanceof HTMLElement) || !(panel instanceof HTMLElement)) {
      return false;
    }
    return root.dataset.open === "true" && !panel.hidden;
  }

  function relocateAccessChatOverlayInViewport() {
    const root = document.getElementById(ACCESS_CHAT_ROOT_ID);
    const panel = document.getElementById(ACCESS_CHAT_OVERLAY_ID);
    const active = viewportCopilotActive();
    const open = accessChatOverlayOpen();
    const plane = active ? ensureCopilotPlane() : null;

    document.body.classList.toggle("mei-copilot-ai-overlay-open", active && open);
    document.body.classList.toggle("mei-copilot-fab-mounted", active && open);

    if (!(panel instanceof HTMLElement) || !(root instanceof HTMLElement)) {
      return;
    }

    if (active && open && plane) {
      panel.classList.add("mei-copilot-in-viewport", "mei-copilot-ai-overlay-panel");
      if (panel.parentElement !== plane) {
        plane.appendChild(panel);
      }
      return;
    }

    panel.classList.remove("mei-copilot-ai-overlay-panel");
    if (root.classList.contains("mei-copilot-in-viewport")) {
      panel.classList.add("mei-copilot-in-viewport");
    } else {
      panel.classList.remove("mei-copilot-in-viewport");
    }
    if (panel.parentElement !== root) {
      root.appendChild(panel);
    }
  }

  function resolveViewportStageHost() {
    const viewport = document.querySelector('[data-mei-frame-viewport="true"]');
    if (viewport instanceof HTMLElement) {
      const stage = viewport.querySelector(".preview-stage-shell");
      if (stage instanceof HTMLElement) {
        return stage;
      }
    }
    return document.body;
  }

  function resolveViewportStageSurface() {
    const shell = resolveViewportStageHost();
    if (shell === document.body) {
      return document.body;
    }
    const stage = shell.querySelector(".preview-stage.preview-surface");
    return stage instanceof HTMLElement ? stage : shell;
  }

  function viewportCopilotActive() {
    return resolveViewportStageHost() !== document.body;
  }

  /** P/C 浮层挂在 shell（信纸框像素），避免 stage transform 下全屏 plane 吞掉指针事件。 */
  function resolveCopilotOverlayHost() {
    return resolveViewportStageHost();
  }

  function ensurePresentationPlane() {
    const host = resolveCopilotOverlayHost();
    if (host === document.body) {
      return null;
    }
    let plane = document.getElementById(PRESENTATION_PLANE_ID);
    if (!plane) {
      plane = document.createElement("div");
      plane.id = PRESENTATION_PLANE_ID;
      plane.className = "mei-presentation-plane";
      const copilotPlane = document.getElementById(COPILOT_PLANE_ID);
      if (copilotPlane && copilotPlane.parentElement === host) {
        host.insertBefore(plane, copilotPlane);
      } else {
        host.appendChild(plane);
      }
    } else if (plane.parentElement !== host) {
      host.appendChild(plane);
    } else {
      host.appendChild(plane);
    }
    return plane;
  }

  function ensureCopilotPlane() {
    const host = resolveCopilotOverlayHost();
    if (host === document.body) {
      return null;
    }
    ensurePresentationPlane();
    let plane = document.getElementById(COPILOT_PLANE_ID);
    if (!plane) {
      plane = document.createElement("div");
      plane.id = COPILOT_PLANE_ID;
      plane.className = "mei-copilot-plane";
      host.appendChild(plane);
    } else if (plane.parentElement !== host) {
      host.appendChild(plane);
    } else {
      host.appendChild(plane);
    }
    return plane;
  }

  function mountPresentationInViewport(node) {
    if (!(node instanceof HTMLElement)) {
      return false;
    }
    const plane = ensurePresentationPlane();
    if (!plane) {
      node.classList.remove("mei-presentation-in-viewport");
      return false;
    }
    if (node.parentElement !== plane) {
      plane.appendChild(node);
    } else {
      plane.appendChild(node);
    }
    node.classList.add("mei-presentation-in-viewport");
    document.body.classList.toggle(
      "mei-presentation-viewport-mounted",
      Boolean(document.querySelector(".mei-presentation-in-viewport")),
    );
    return true;
  }

  function updateCopilotViewportMountedClass() {
    const fab = document.getElementById("access-chat-floating-root");
    const external = document.getElementById("access-external-ai-floating-root");
    const mounted = Boolean(
      (fab instanceof HTMLElement &&
        (fab.classList.contains("mei-copilot-in-viewport") ||
          fab.classList.contains("mei-copilot-letterbox-fixed"))) ||
        (external instanceof HTMLElement && external.classList.contains("mei-copilot-in-viewport")),
    );
    document.body.classList.toggle("mei-copilot-viewport-mounted", mounted);
  }

  function readLetterboxScale() {
    const viewport = document.querySelector('[data-mei-frame-viewport="true"]');
    const scale = Number(viewport?.dataset?.meiFrameScale || 1);
    return Number.isFinite(scale) && scale > 0 ? scale : 1;
  }

  const FAB_MARGIN_DESIGN_PX = 24;
  const FAB_SIZE_DESIGN_PX = 64;

  /** 访问态 FAB：坐标系 = preview-stage-shell 信纸框（已缩放后的 viewport 像素）。 */
  function resolveAccessFabLetterboxLayout(root) {
    const shell = resolveViewportStageHost();
    if (!(shell instanceof HTMLElement) || shell === document.body) {
      return null;
    }
    const shellRect = shell.getBoundingClientRect();
    const scale = readLetterboxScale();
    const width = Math.max(0, shell.clientWidth || shellRect.width || 0);
    const height = Math.max(0, shell.clientHeight || shellRect.height || 0);
    const margin = FAB_MARGIN_DESIGN_PX * scale;
    const fabSize = FAB_SIZE_DESIGN_PX * scale;
    const node =
      root instanceof HTMLElement ? root : document.getElementById(ACCESS_CHAT_ROOT_ID);
    const nodeW = Math.max(
      fabSize,
      Number(node?.offsetWidth || 0) || fabSize,
    );
    const nodeH = Math.max(
      fabSize,
      Number(node?.offsetHeight || 0) || fabSize,
    );

    function localToScreen(left, top) {
      return {
        left: shellRect.left + left,
        top: shellRect.top + top,
      };
    }

    function screenToLocal(screenLeft, screenTop) {
      return {
        left: screenLeft - shellRect.left,
        top: screenTop - shellRect.top,
      };
    }

    function clampLocal(left, top, boxW = nodeW, boxH = nodeH, pad = margin) {
      const min = Math.max(0, pad);
      const maxLeft = Math.max(min, width - boxW - min);
      const maxTop = Math.max(min, height - boxH - min);
      return {
        left: Math.min(maxLeft, Math.max(min, Math.round(Number(left) || 0))),
        top: Math.min(maxTop, Math.max(min, Math.round(Number(top) || 0))),
      };
    }

    function defaultLocal() {
      return clampLocal(width - nodeW - margin, height - nodeH - margin);
    }

    return {
      scale,
      shell,
      shellRect,
      width,
      height,
      margin,
      fabSize,
      marginDesign: FAB_MARGIN_DESIGN_PX,
      fabDesign: FAB_SIZE_DESIGN_PX,
      localToScreen,
      screenToLocal,
      clampLocal,
      defaultLocal,
    };
  }

  function applyAccessFabLetterboxPosition(root, localLeft, localTop) {
    if (!(root instanceof HTMLElement)) {
      return null;
    }
    const layout = resolveAccessFabLetterboxLayout(root);
    if (!layout) {
      return null;
    }
    const pos = layout.clampLocal(localLeft, localTop);
    const screen = layout.localToScreen(pos.left, pos.top);
    root.dataset.positioned = "true";
    root.dataset.letterboxLeft = String(pos.left);
    root.dataset.letterboxTop = String(pos.top);
    root.style.position = "fixed";
    root.style.left = `${Math.round(screen.left)}px`;
    root.style.top = `${Math.round(screen.top)}px`;
    root.style.right = "auto";
    root.style.bottom = "auto";
    root.style.transform = "none";
    const fab = document.getElementById("access-chat-fab");
    if (fab instanceof HTMLElement) {
      fab.style.width = `${Math.round(layout.fabSize)}px`;
      fab.style.height = `${Math.round(layout.fabSize)}px`;
    }
    return pos;
  }

  function resolveViewportFabDesignCanvas() {
    const stage = resolveViewportStageSurface();
    if (stage instanceof HTMLElement) {
      return {
        width: Math.max(1, stage.offsetWidth || 0),
        height: Math.max(1, stage.offsetHeight || 0),
      };
    }
    const layout = resolveAccessFabLetterboxLayout();
    const scale = readLetterboxScale();
    if (layout) {
      return {
        width: Math.max(1, layout.width / scale),
        height: Math.max(1, layout.height / scale),
      };
    }
    return { width: 1920, height: 1080 };
  }

  function clampViewportFabDesignPosition(
    designLeft,
    designTop,
    boxDesignW = FAB_SIZE_DESIGN_PX,
    boxDesignH = FAB_SIZE_DESIGN_PX,
  ) {
    const canvas = resolveViewportFabDesignCanvas();
    const pad = FAB_MARGIN_DESIGN_PX;
    const min = pad;
    const maxLeft = Math.max(min, canvas.width - boxDesignW - pad);
    const maxTop = Math.max(min, canvas.height - boxDesignH - pad);
    return {
      left: Math.min(maxLeft, Math.max(min, Math.round(Number(designLeft) || 0))),
      top: Math.min(maxTop, Math.max(min, Math.round(Number(designTop) || 0))),
    };
  }

  function shellToViewportFabDesign(shellLeft, shellTop) {
    const scale = readLetterboxScale();
    return {
      left: (Number(shellLeft) || 0) / scale,
      top: (Number(shellTop) || 0) / scale,
    };
  }

  function designToViewportFabShell(designLeft, designTop) {
    const scale = readLetterboxScale();
    return {
      left: Math.round((Number(designLeft) || 0) * scale),
      top: Math.round((Number(designTop) || 0) * scale),
    };
  }

  /** 设计稿坐标 → shell 像素；resize 时仅按 scale 重算，保持与 T0/T1/T2 同比例。 */
  function applyViewportFabDesignPosition(root, designLeft, designTop) {
    if (!(root instanceof HTMLElement)) {
      return null;
    }
    const design = clampViewportFabDesignPosition(designLeft, designTop);
    const shell = designToViewportFabShell(design.left, design.top);
    root.dataset.positioned = "true";
    root.dataset.fabDesignLeft = String(design.left);
    root.dataset.fabDesignTop = String(design.top);
    delete root.dataset.letterboxLeft;
    delete root.dataset.letterboxTop;
    root.style.left = `${shell.left}px`;
    root.style.top = `${shell.top}px`;
    root.style.right = "auto";
    root.style.bottom = "auto";
    return design;
  }

  function resyncViewportFabDesignPosition(root) {
    if (!(root instanceof HTMLElement) || root.dataset.positioned !== "true") {
      return null;
    }
    let designLeft = Number(root.dataset.fabDesignLeft);
    let designTop = Number(root.dataset.fabDesignTop);
    if (!Number.isFinite(designLeft) || !Number.isFinite(designTop)) {
      const scale = readLetterboxScale();
      const shellLeft = Number(root.style.left);
      const shellTop = Number(root.style.top);
      if (!Number.isFinite(shellLeft) || !Number.isFinite(shellTop) || scale <= 0) {
        return null;
      }
      designLeft = shellLeft / scale;
      designTop = shellTop / scale;
    }
    return applyViewportFabDesignPosition(root, designLeft, designTop);
  }

  /** FAB 挂入 copilot-plane，与 T0/T1/T2 共用 shell 信纸框坐标系（--mei-frame-scale）。 */
  function relocateAccessFabInLetterbox() {
    const root = document.getElementById(ACCESS_CHAT_ROOT_ID);
    if (!(root instanceof HTMLElement)) {
      return;
    }
    if (!viewportCopilotActive()) {
      root.classList.remove("mei-copilot-in-viewport", "mei-copilot-letterbox-fixed");
      root.style.position = "";
      root.style.left = "";
      root.style.top = "";
      root.style.right = "";
      root.style.bottom = "";
      root.style.zIndex = "";
      root.style.transform = "";
      root.style.pointerEvents = "";
      return;
    }
    const plane = ensureCopilotPlane();
    if (!plane) {
      return;
    }
    if (root.parentElement !== plane) {
      plane.appendChild(root);
    }
    root.classList.add("mei-copilot-in-viewport");
    root.classList.remove("mei-copilot-letterbox-fixed");
    root.style.zIndex = "";
    root.style.pointerEvents = "";
    root.style.transform = "";

    const fab = document.getElementById("access-chat-fab");
    if (fab instanceof HTMLElement) {
      fab.style.pointerEvents = "auto";
      fab.style.width = "";
      fab.style.height = "";
    }

    if (root.dataset.letterboxLeft != null || root.dataset.letterboxTop != null) {
      const migratedLeft = Number(root.dataset.letterboxLeft);
      const migratedTop = Number(root.dataset.letterboxTop);
      delete root.dataset.letterboxLeft;
      delete root.dataset.letterboxTop;
      if (Number.isFinite(migratedLeft) && Number.isFinite(migratedTop)) {
        const scale = readLetterboxScale();
        applyViewportFabDesignPosition(root, migratedLeft / scale, migratedTop / scale);
        return;
      }
    }

    if (root.dataset.positioned === "true") {
      resyncViewportFabDesignPosition(root);
      return;
    }

    root.style.left = "";
    root.style.top = "";
    root.style.right = "";
    root.style.bottom = "";
    delete root.dataset.positioned;
    delete root.dataset.fabDesignLeft;
    delete root.dataset.fabDesignTop;
  }

  function mountCopilotInViewport(node) {
    if (!(node instanceof HTMLElement)) {
      return false;
    }
    if (node.id === ACCESS_CHAT_ROOT_ID) {
      relocateAccessFabInLetterbox();
      updateCopilotViewportMountedClass();
      return true;
    }
    const plane = ensureCopilotPlane();
    if (!plane) {
      node.classList.remove("mei-copilot-in-viewport");
      updateCopilotViewportMountedClass();
      return false;
    }
    if (node.parentElement !== plane) {
      plane.appendChild(node);
    } else {
      plane.appendChild(node);
    }
    node.classList.add("mei-copilot-in-viewport");
    if (node.dataset.positioned === "true") {
      node.style.right = "";
      node.style.bottom = "";
    } else {
      node.style.left = "";
      node.style.top = "";
    }
    updateCopilotViewportMountedClass();
    return true;
  }

  function copilotFloatingBoundsSize() {
    const shell = resolveCopilotOverlayHost();
    if (shell === document.body) {
      return {
        width: Number(window.innerWidth || 0),
        height: Number(window.innerHeight || 0),
      };
    }
    return {
      width: Math.max(0, shell.clientWidth || shell.offsetWidth || 0),
      height: Math.max(0, shell.clientHeight || shell.offsetHeight || 0),
    };
  }

  function copilotFloatingOffsetParent(node) {
    if (!(node instanceof HTMLElement)) {
      return null;
    }
    if (node.classList.contains("mei-copilot-letterbox-fixed")) {
      return resolveViewportStageHost();
    }
    return (
      node.closest(".mei-copilot-plane") ||
      node.closest(".preview-stage-shell") ||
      null
    );
  }

  function relocatePresentationInViewport() {
    if (!viewportCopilotActive()) {
      return;
    }
    ensurePresentationPlane();
    PRESENTATION_NODE_IDS.forEach((id) => {
      const node = document.getElementById(id);
      if (node) {
        mountPresentationInViewport(node);
      }
    });
  }

  function relocateCopilotInViewport() {
    if (!viewportCopilotActive()) {
      [...COPILOT_PLANE_NODE_IDS, ACCESS_CHAT_ROOT_ID].forEach((id) => {
        const node = document.getElementById(id);
        if (node instanceof HTMLElement) {
          node.classList.remove("mei-copilot-in-viewport", "mei-copilot-letterbox-fixed");
        }
      });
      updateCopilotViewportMountedClass();
      relocateAccessChatOverlayInViewport();
      return;
    }
    ensureCopilotPlane();
    relocateAccessFabInLetterbox();
    COPILOT_PLANE_NODE_IDS.forEach((id) => {
      const node = document.getElementById(id);
      if (node) {
        mountCopilotInViewport(node);
      }
    });
    updateCopilotViewportMountedClass();
    relocateAccessChatOverlayInViewport();
  }

  function relocateStageOverlaysInViewport() {
    relocatePresentationInViewport();
    relocateCopilotInViewport();
    const bootApi = window.__meiLangBoot || {};
    if (typeof bootApi.ensureLayer2WorkspaceRoot === "function") {
      bootApi.ensureLayer2WorkspaceRoot();
    }
    const layer2 = document.getElementById("mei-layer2-workspace");
    if (layer2 instanceof HTMLElement) {
      const surface = resolveViewportStageSurface();
      layer2.classList.toggle("mei-layer2-in-viewport", surface !== document.body);
    }
    if (typeof bootApi.reclampAccessFloatingInViewport === "function") {
      bootApi.reclampAccessFloatingInViewport();
    }
    if (
      bootApi.copilotFabLayout &&
      typeof bootApi.copilotFabLayout.scheduleCopilotFabToolbarLayout === "function"
    ) {
      bootApi.copilotFabLayout.scheduleCopilotFabToolbarLayout();
    }
    if (typeof bootApi.syncCockpitMapToolsOverlays === "function") {
      bootApi.syncCockpitMapToolsOverlays();
    }
  }

  boot.readViewportFrameScale = readLetterboxScale;
  boot.resolveViewportStageHost = resolveViewportStageHost;
  boot.resolveViewportStageSurface = resolveViewportStageSurface;
  boot.ensurePresentationPlane = ensurePresentationPlane;
  boot.ensureCopilotPlane = ensureCopilotPlane;
  boot.mountPresentationInViewport = mountPresentationInViewport;
  boot.mountCopilotInViewport = mountCopilotInViewport;
  boot.copilotFloatingBoundsSize = copilotFloatingBoundsSize;
  boot.copilotFloatingOffsetParent = copilotFloatingOffsetParent;
  boot.relocatePresentationInViewport = relocatePresentationInViewport;
  boot.relocateCopilotInViewport = relocateCopilotInViewport;
  boot.relocateAccessChatOverlayInViewport = relocateAccessChatOverlayInViewport;
  boot.relocateAccessFabInLetterbox = relocateAccessFabInLetterbox;
  boot.resolveAccessFabLetterboxLayout = resolveAccessFabLetterboxLayout;
  boot.applyAccessFabLetterboxPosition = applyAccessFabLetterboxPosition;
  boot.applyViewportFabDesignPosition = applyViewportFabDesignPosition;
  boot.resyncViewportFabDesignPosition = resyncViewportFabDesignPosition;
  boot.shellToViewportFabDesign = shellToViewportFabDesign;
  boot.designToViewportFabShell = designToViewportFabShell;
  boot.clampViewportFabDesignPosition = clampViewportFabDesignPosition;
  boot.relocateStageOverlaysInViewport = relocateStageOverlaysInViewport;

  function scheduleRelocate() {
    window.requestAnimationFrame(() => {
      window.requestAnimationFrame(() => {
        relocateStageOverlaysInViewport();
      });
    });
  }

  function isViewportStageLayoutReady() {
    const viewport = document.querySelector('[data-mei-frame-viewport="true"]');
    if (!(viewport instanceof HTMLElement)) {
      return null;
    }
    const shell = viewport.querySelector(".preview-stage-shell");
    const stage = shell?.querySelector(".preview-stage.preview-surface");
    if (!(shell instanceof HTMLElement) || !(stage instanceof HTMLElement)) {
      return null;
    }
    const scale = String(viewport.dataset.meiFrameScale || "").trim();
    if (!scale) {
      return null;
    }
    const shellW = shell.clientWidth || shell.offsetWidth || 0;
    const shellH = shell.clientHeight || shell.offsetHeight || 0;
    if (shellW <= 0 || shellH <= 0) {
      return null;
    }
    return { viewport, shell, stage, scale: Number(scale) || 1 };
  }

  function waitForViewportStageReady(attemptsLeft = 240) {
    const ready = isViewportStageLayoutReady();
    if (ready) {
      scheduleRelocate();
      return;
    }
    if (attemptsLeft > 0) {
      requestAnimationFrame(() => waitForViewportStageReady(attemptsLeft - 1));
    }
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", () => waitForViewportStageReady(), {
      once: true,
    });
  } else {
    waitForViewportStageReady();
  }
  document.addEventListener("mei:spa-navigation-complete", () => waitForViewportStageReady());
  window.addEventListener("meilang:preview-updated", scheduleRelocate);
  window.addEventListener("meilang:viewport-stage-layout", scheduleRelocate);
  window.addEventListener("pageshow", scheduleRelocate);
  window.addEventListener("resize", scheduleRelocate, { passive: true });
  if (window.visualViewport) {
    window.visualViewport.addEventListener("resize", scheduleRelocate);
  }
})();
