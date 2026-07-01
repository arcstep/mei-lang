(() => {
  const boot = (window.__meiLangBoot = window.__meiLangBoot || {});
  const PRESENTATION_PLANE_ID = "mei-presentation-plane";
  const COPILOT_PLANE_ID = "mei-copilot-plane";
  const PRESENTATION_NODE_IDS = ["mei-copilot-slide-layer"];
  const COPILOT_NODE_IDS = [
    "access-chat-floating-root",
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

  function viewportCopilotActive() {
    return resolveViewportStageHost() !== document.body;
  }

  function ensurePresentationPlane() {
    const host = resolveViewportStageHost();
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
    }
    return plane;
  }

  function ensureCopilotPlane() {
    const host = resolveViewportStageHost();
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
      (fab instanceof HTMLElement && fab.classList.contains("mei-copilot-in-viewport")) ||
        (external instanceof HTMLElement && external.classList.contains("mei-copilot-in-viewport")),
    );
    document.body.classList.toggle("mei-copilot-viewport-mounted", mounted);
  }

  function mountCopilotInViewport(node) {
    if (!(node instanceof HTMLElement)) {
      return false;
    }
    const plane = ensureCopilotPlane();
    if (!plane) {
      node.classList.remove("mei-copilot-in-viewport");
      updateCopilotViewportMountedClass();
      return false;
    }
    if (node.parentElement !== plane) {
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
    const host = resolveViewportStageHost();
    if (host === document.body) {
      return {
        width: Number(window.innerWidth || 0),
        height: Number(window.innerHeight || 0),
      };
    }
    return {
      width: Math.max(0, host.clientWidth || host.offsetWidth || 0),
      height: Math.max(0, host.clientHeight || host.offsetHeight || 0),
    };
  }

  function copilotFloatingOffsetParent(node) {
    if (!(node instanceof HTMLElement)) {
      return null;
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
      COPILOT_NODE_IDS.forEach((id) => {
        const node = document.getElementById(id);
        if (node instanceof HTMLElement) {
          node.classList.remove("mei-copilot-in-viewport");
        }
      });
      updateCopilotViewportMountedClass();
      relocateAccessChatOverlayInViewport();
      return;
    }
    ensureCopilotPlane();
    COPILOT_NODE_IDS.forEach((id) => {
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
    if (typeof bootApi.reclampAccessFloatingInViewport === "function") {
      bootApi.reclampAccessFloatingInViewport();
    }
    if (
      bootApi.copilotFabLayout &&
      typeof bootApi.copilotFabLayout.scheduleCopilotFabToolbarLayout === "function"
    ) {
      bootApi.copilotFabLayout.scheduleCopilotFabToolbarLayout();
    }
  }

  boot.resolveViewportStageHost = resolveViewportStageHost;
  boot.ensurePresentationPlane = ensurePresentationPlane;
  boot.ensureCopilotPlane = ensureCopilotPlane;
  boot.mountPresentationInViewport = mountPresentationInViewport;
  boot.mountCopilotInViewport = mountCopilotInViewport;
  boot.copilotFloatingBoundsSize = copilotFloatingBoundsSize;
  boot.copilotFloatingOffsetParent = copilotFloatingOffsetParent;
  boot.relocatePresentationInViewport = relocatePresentationInViewport;
  boot.relocateCopilotInViewport = relocateCopilotInViewport;
  boot.relocateAccessChatOverlayInViewport = relocateAccessChatOverlayInViewport;
  boot.relocateStageOverlaysInViewport = relocateStageOverlaysInViewport;

  function scheduleRelocate() {
    window.requestAnimationFrame(() => {
      window.requestAnimationFrame(() => {
        relocateStageOverlaysInViewport();
      });
    });
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", scheduleRelocate, { once: true });
  } else {
    scheduleRelocate();
  }
  document.addEventListener("mei:spa-navigation-complete", scheduleRelocate);
  window.addEventListener("meilang:preview-updated", scheduleRelocate);
})();
