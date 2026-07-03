/**
 * 驾驶舱访问态：GIS / 上下文浮层挂入 preview-stage，与 T0/T1 共用 transform scale。
 */

import {
  clientPointToStageLocal,
  focusInsetViewportRect,
  resolveCockpitStageMetrics,
  resolveCockpitStageSurface,
} from "./map-focus-inset.js";

export const COCKPIT_MAP_TOOLS_PLANE_ID = "mei-cockpit-map-tools-plane";
export const WORLD_STAGE_INPUT_PLANE_ID = "mei-world-stage-input-plane";

const mapToolHosts = new Set();

function registerMapToolHost(host) {
  if (!host) return;
  mapToolHosts.add(host);
  ensureGlobalMapToolSync();
}

function unregisterMapToolHost(host) {
  mapToolHosts.delete(host);
}

function ensureGlobalMapToolSync() {
  if (typeof window === "undefined") return;
  const boot = (window.__meiLangBoot = window.__meiLangBoot || {});
  if (boot._cockpitMapToolSyncBound) return;
  boot._cockpitMapToolSyncBound = true;
  const syncAll = () => {
    for (const host of mapToolHosts) {
      if (!host?.isConnected || !host._layout?.cockpitBleed) continue;
      try {
        host.syncCockpitMapToolsLayer?.();
        host.scheduleLayerControlLayout?.();
      } catch (_) {
        /* ignore */
      }
    }
  };
  boot.syncCockpitMapToolsOverlays = syncAll;
  window.addEventListener("meilang:viewport-stage-layout", syncAll);
  window.addEventListener("meilang:viewport-stage-ready", syncAll);
  document.addEventListener("mei:spa-navigation-complete", syncAll);
}

export function trackCockpitMapToolHost(host) {
  registerMapToolHost(host);
  return () => unregisterMapToolHost(host);
}

export function resolveCockpitStageShell(host) {
  return (
    host?.closest?.(".preview-stage-shell") ||
    resolveCockpitStageSurface(host)?.parentElement ||
    document.querySelector(".preview-stage-shell")
  );
}

export function resolveCockpitShellLayout(host) {
  const metrics = resolveCockpitStageMetrics(host);
  const shell = resolveCockpitStageShell(host);
  const stage = resolveCockpitStageSurface(host);
  if (!metrics || !shell || !stage) {
    return null;
  }
  return { ...metrics, shell, stage };
}

export function ensureCockpitMapToolsPlane(host) {
  const stage = resolveCockpitStageSurface(host);
  if (!stage) {
    return null;
  }
  let plane = stage.querySelector(`#${COCKPIT_MAP_TOOLS_PLANE_ID}`);
  if (!plane) {
    plane = document.createElement("div");
    plane.id = COCKPIT_MAP_TOOLS_PLANE_ID;
    plane.className = "mei-cockpit-map-tools-plane";
    stage.appendChild(plane);
  } else if (plane.parentElement !== stage) {
    stage.appendChild(plane);
  }
  return plane;
}

export function mountCockpitMapToolsOverlay(node, host) {
  const plane = ensureCockpitMapToolsPlane(host);
  if (!plane || !node) {
    return null;
  }
  if (node.parentElement !== plane) {
    plane.appendChild(node);
  }
  node.classList.add("mei-cockpit-in-stage-shell");
  node.setAttribute("data-mei-overlay-role", "map_tools");
  return plane;
}

export function positionFocusInsetTopRight(node, host, focusInsetPx, gap = 10) {
  const layout = resolveCockpitShellLayout(host);
  if (!layout || !node || !focusInsetPx) {
    return false;
  }
  const top = Number(focusInsetPx.top) || 0;
  const right = Number(focusInsetPx.right) || 0;
  node.style.position = "absolute";
  node.style.margin = "0";
  node.style.transform = "none";
  node.style.top = `${Math.round(top + gap)}px`;
  node.style.right = `${Math.round(right + gap)}px`;
  node.style.left = "auto";
  node.style.bottom = "auto";
  node.style.zIndex = "";
  return true;
}

/** 观察窗底部居中（舞台设计稿坐标） */
export function positionFocusInsetBottomCenter(node, host, focusInsetPx, gap = 16) {
  const layout = resolveCockpitShellLayout(host);
  if (!layout || !node || !focusInsetPx) {
    return false;
  }
  const left = Number(focusInsetPx.left) || 0;
  const right = Number(focusInsetPx.right) || 0;
  const bottom = Number(focusInsetPx.bottom) || 0;
  const designW = layout.stage.offsetWidth || layout.designW || 1920;
  const designH = layout.stage.offsetHeight || layout.designH || 1080;
  const width = designW - left - right;
  node.style.position = "absolute";
  node.style.margin = "0";
  node.style.transform = "translateX(-50%)";
  node.style.left = `${Math.round(left + width / 2)}px`;
  node.style.bottom = `${Math.round(bottom + gap)}px`;
  node.style.top = "auto";
  node.style.right = "auto";
  node.style.zIndex = "";
  return true;
}

/** shell / stage 不可用时回退到 body + fixed（管理态预览等） */
export function positionFocusInsetTopRightFixed(node, host, focusInsetPx, gap = 10) {
  const metrics = resolveCockpitStageMetrics(host);
  if (!metrics || !node || !focusInsetPx) {
    return false;
  }
  const top = Number(focusInsetPx.top) || 0;
  const right = Number(focusInsetPx.right) || 0;
  node.style.position = "fixed";
  node.style.margin = "0";
  node.style.transform = "none";
  node.style.top = `${Math.round(metrics.offsetY + (top + gap) * metrics.scale)}px`;
  node.style.right = `${Math.round(
    window.innerWidth -
      (metrics.offsetX + metrics.designW * metrics.scale) +
      (right + gap) * metrics.scale,
  )}px`;
  node.style.left = "auto";
  node.style.bottom = "auto";
  return true;
}

export function mountCockpitFloatingControl(node, host) {
  const stage = resolveCockpitStageSurface(host);
  if (stage && mountCockpitMapToolsOverlay(node, host)) {
    return "stage";
  }
  if (node.parentElement !== document.body) {
    document.body.appendChild(node);
  }
  node.classList.remove("mei-cockpit-in-stage-shell");
  return "body";
}

export function positionCockpitFloatingNav(node, host, focusInsetPx, gap = 10) {
  const mount = mountCockpitFloatingControl(node, host);
  if (mount === "stage") {
    positionFocusInsetTopRight(node, host, focusInsetPx, gap);
    return mount;
  }
  positionFocusInsetTopRightFixed(node, host, focusInsetPx, gap);
  return mount;
}

export function positionLayerControlNearAnchor(panel, host, anchorRect, focusInsetPx, gap = 8) {
  const layout = resolveCockpitShellLayout(host);
  if (!layout || !panel || !anchorRect || !focusInsetPx) {
    return false;
  }
  const { stage } = layout;
  const designW = stage.offsetWidth || layout.designW || 1920;
  const designH = stage.offsetHeight || layout.designH || 1080;
  const anchorBR = clientPointToStageLocal(stage, anchorRect.right, anchorRect.bottom);
  const focusTop = Number(focusInsetPx.top) || 0;
  const focusBottom = Number(focusInsetPx.bottom) || 0;
  const focusLeft = Number(focusInsetPx.left) || 0;

  panel.style.position = "absolute";
  panel.style.transform = "none";
  panel.style.right = `${Math.round(designW - anchorBR.left + gap)}px`;
  panel.style.left = "auto";

  let panelTop = anchorBR.top + gap;
  let maxHeight = designH - focusBottom - panelTop - gap;
  if (maxHeight < 160) {
    panelTop = Math.max(focusTop + gap, panelTop);
    maxHeight = designH - focusBottom - panelTop - gap;
  }
  panel.style.top = `${Math.round(panelTop)}px`;
  panel.style.bottom = "auto";
  panel.style.maxHeight = `${Math.max(120, Math.round(maxHeight))}px`;

  const panelWidth = panel.offsetWidth || 260;
  const panelLeft = anchorBR.left - gap - panelWidth;
  if (panelLeft < focusLeft + gap) {
    panel.style.right = "auto";
    panel.style.left = `${Math.round(focusLeft + gap)}px`;
    panel.style.maxWidth = `${Math.max(
      180,
      Math.round(anchorBR.left - focusLeft - gap * 2),
    )}px`;
  } else {
    panel.style.maxWidth = "";
  }
  return true;
}

export function positionLayerControlNearAnchorFixed(
  panel,
  host,
  anchorRect,
  focusInsetPx,
  gap = 8,
) {
  const metrics = resolveCockpitStageMetrics(host);
  const focusRect = focusInsetViewportRect(metrics, focusInsetPx);
  if (!metrics || !panel || !anchorRect || !focusRect) {
    return false;
  }

  panel.style.position = "fixed";
  panel.style.transform = "none";
  panel.style.right = `${Math.round(window.innerWidth - anchorRect.right + gap)}px`;
  panel.style.left = "auto";

  let panelTop = Math.round(anchorRect.bottom + gap);
  let maxHeight = focusRect.bottom - panelTop - gap;
  if (maxHeight < 160) {
    panelTop = Math.max(Math.round(focusRect.top + gap), panelTop);
    maxHeight = focusRect.bottom - panelTop - gap;
  }
  panel.style.top = `${panelTop}px`;
  panel.style.bottom = "auto";
  panel.style.maxHeight = `${Math.max(120, Math.round(maxHeight))}px`;

  const panelWidth = panel.offsetWidth || 260;
  const panelLeft = anchorRect.right - gap - panelWidth;
  const focusLeft = focusRect.left;
  if (panelLeft < focusLeft + gap) {
    panel.style.right = "auto";
    panel.style.left = `${Math.round(focusLeft + gap)}px`;
    panel.style.maxWidth = `${Math.max(
      180,
      Math.round(anchorRect.left - focusLeft - gap * 2),
    )}px`;
  } else {
    panel.style.maxWidth = "";
  }
  return true;
}

export function resolveCockpitMapToolHost() {
  if (typeof window === "undefined") return null;
  const boot = window.__meiLangBoot || {};
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

export function layoutWorldStageInputPlane(surface, stage, focusInsetPx) {
  if (!surface || !stage) return false;
  const designW = stage.offsetWidth || 1920;
  const designH = stage.offsetHeight || 1080;
  surface.style.position = "absolute";
  surface.style.boxSizing = "border-box";
  surface.style.margin = "0";
  surface.style.transform = "none";
  if (focusInsetPx) {
    const left = Number(focusInsetPx.left) || 0;
    const top = Number(focusInsetPx.top) || 0;
    const right = Number(focusInsetPx.right) || 0;
    const bottom = Number(focusInsetPx.bottom) || 0;
    surface.style.left = `${Math.round(left)}px`;
    surface.style.top = `${Math.round(top)}px`;
    surface.style.width = `${Math.max(0, Math.round(designW - left - right))}px`;
    surface.style.height = `${Math.max(0, Math.round(designH - top - bottom))}px`;
    surface.style.right = "auto";
    surface.style.bottom = "auto";
  } else {
    surface.style.inset = "0";
    surface.style.left = "0";
    surface.style.top = "0";
    surface.style.width = "100%";
    surface.style.height = "100%";
  }
  return true;
}

export function ensureWorldStageInputPlane(host) {
  const stage = resolveCockpitStageSurface(host);
  if (!stage) {
    return null;
  }
  let plane = stage.querySelector(`#${WORLD_STAGE_INPUT_PLANE_ID}`);
  if (!plane) {
    plane = document.createElement("div");
    plane.id = WORLD_STAGE_INPUT_PLANE_ID;
    plane.className = "mei-world-stage-input-plane";
    plane.setAttribute("role", "presentation");
    plane.setAttribute("aria-label", "3D 漫游交互区");
    stage.appendChild(plane);
  } else if (plane.parentElement !== stage) {
    stage.appendChild(plane);
  }
  const mapHost = resolveCockpitMapToolHost();
  layoutWorldStageInputPlane(plane, stage, mapHost?._layout?.focusInsetPx);
  return plane;
}

export function setWorldStageInputPlaneActive(active) {
  if (typeof document === "undefined") return;
  document.querySelectorAll(`#${WORLD_STAGE_INPUT_PLANE_ID}`).forEach((node) => {
    if (!(node instanceof HTMLElement)) return;
    node.hidden = !active;
    node.style.display = active ? "block" : "none";
    node.style.pointerEvents = active ? "auto" : "none";
  });
  const mapPlane = document.getElementById(COCKPIT_MAP_TOOLS_PLANE_ID);
  if (mapPlane instanceof HTMLElement) {
    if (active) {
      mapPlane.setAttribute("inert", "");
      mapPlane.hidden = true;
    } else {
      mapPlane.removeAttribute("inert");
      mapPlane.hidden = false;
    }
  }
}

const layoutSyncEntries = new WeakMap();

export function bindCockpitStageLayoutSync(host, callback) {
  if (!host || typeof callback !== "function") {
    return () => {};
  }
  let entry = layoutSyncEntries.get(host);
  if (!entry) {
    entry = { callbacks: new Set(), bound: false };
    layoutSyncEntries.set(host, entry);
  }
  entry.callbacks.add(callback);
  if (!entry.bound) {
    entry.bound = true;
    const onLayout = () => {
      for (const cb of entry.callbacks) {
        try {
          cb();
        } catch (_) {
          /* ignore */
        }
      }
    };
    entry.onLayout = onLayout;
    window.addEventListener("resize", onLayout, { passive: true });
    window.addEventListener("meilang:preview-updated", onLayout);
    window.addEventListener("meilang:viewport-stage-layout", onLayout);
    window.addEventListener("meilang:viewport-stage-ready", onLayout);
    if (window.visualViewport) {
      window.visualViewport.addEventListener("resize", onLayout);
    }
    const stage = resolveCockpitStageSurface(host);
    if (stage && typeof ResizeObserver !== "undefined") {
      entry.ro = new ResizeObserver(onLayout);
      entry.ro.observe(stage);
      const shell = stage.parentElement;
      if (shell instanceof HTMLElement) {
        entry.ro.observe(shell);
      }
    }
  }
  return () => {
    entry.callbacks.delete(callback);
    if (entry.callbacks.size === 0 && entry.bound) {
      window.removeEventListener("resize", entry.onLayout);
      window.removeEventListener("meilang:preview-updated", entry.onLayout);
      window.removeEventListener("meilang:viewport-stage-layout", entry.onLayout);
      window.removeEventListener("meilang:viewport-stage-ready", entry.onLayout);
      if (window.visualViewport) {
        window.visualViewport.removeEventListener("resize", entry.onLayout);
      }
      entry.ro?.disconnect();
      layoutSyncEntries.delete(host);
    }
  };
}

if (typeof window !== "undefined") {
  const boot = (window.__meiLangBoot = window.__meiLangBoot || {});
  boot.ensureCockpitMapToolsPlane = ensureCockpitMapToolsPlane;
  boot.mountCockpitFloatingControl = mountCockpitFloatingControl;
  boot.mountCockpitMapToolsOverlay = mountCockpitMapToolsOverlay;
  boot.positionCockpitFloatingNav = positionCockpitFloatingNav;
  boot.positionFocusInsetTopRight = positionFocusInsetTopRight;
  boot.positionFocusInsetTopRightFixed = positionFocusInsetTopRightFixed;
  boot.positionFocusInsetBottomCenter = positionFocusInsetBottomCenter;
  boot.positionLayerControlNearAnchor = positionLayerControlNearAnchor;
  boot.positionLayerControlNearAnchorFixed = positionLayerControlNearAnchorFixed;
  boot.bindCockpitStageLayoutSync = bindCockpitStageLayoutSync;
  boot.trackCockpitMapToolHost = trackCockpitMapToolHost;
  boot.ensureWorldStageInputPlane = ensureWorldStageInputPlane;
  boot.setWorldStageInputPlaneActive = setWorldStageInputPlaneActive;
  boot.layoutWorldStageInputPlane = layoutWorldStageInputPlane;
}
