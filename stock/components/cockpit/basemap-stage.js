import { escapeHtml, parseProps } from "./shared.js";
import {
  resolveCockpitBleedLayout,
  resolveMapFocusInset,
  focusFrameGuideStyle,
} from "./map-focus-inset.js";

const TAG = "mei-cockpit-basemap-stage";

function clamp(value, min, max) {
  return Math.min(max, Math.max(min, value));
}

class MeiCockpitBasemapStage extends HTMLElement {
  constructor() {
    super();
    this._zoom = 1;
    this._panX = 0;
    this._panY = 0;
    this._drag = null;
    this._wheelBound = null;
    this._svgViewBox = { width: 800, height: 600 };
    this._activeShapeIds = new Set();
    this._activeHotspotIds = new Set();
    this._hiddenGroups = new Set();
    this._activeGroupId = "";
    this._lastWorldTarget = null;
  }

  connectedCallback() {
    this.props = parseProps(this);
    if (!this.shadowRoot) {
      this.attachShadow({ mode: "open" });
    }
    this.render();
    this.loadContent();
    this.bindInteractions();
  }

  disconnectedCallback() {
    this.unbindInteractions();
  }

  unbindInteractions() {
    const viewport = this.shadowRoot?.querySelector(".viewport");
    if (viewport && this._wheelBound) {
      viewport.removeEventListener("wheel", this._wheelBound);
      this._wheelBound = null;
    }
    this._drag = null;
  }

  bindInteractions() {
    const viewport = this.shadowRoot?.querySelector(".viewport");
    if (!viewport) {
      return;
    }
    this._wheelBound = (event) => {
      event.preventDefault();
      const factor = event.deltaY < 0 ? 1.08 : 1 / 1.08;
      this.zoomBy(factor);
    };
    viewport.addEventListener("wheel", this._wheelBound, { passive: false });

    viewport.addEventListener("pointerdown", (event) => {
      if (event.button !== 0) {
        return;
      }
      this._drag = {
        x: event.clientX,
        y: event.clientY,
        panX: this._panX,
        panY: this._panY,
      };
      viewport.setPointerCapture(event.pointerId);
    });
    viewport.addEventListener("pointermove", (event) => {
      if (!this._drag) {
        return;
      }
      this._panX = this._drag.panX + (event.clientX - this._drag.x);
      this._panY = this._drag.panY + (event.clientY - this._drag.y);
      this.applyTransform();
    });
    viewport.addEventListener("pointerup", () => {
      this._drag = null;
    });
    viewport.addEventListener("pointercancel", () => {
      this._drag = null;
    });

    this.shadowRoot.querySelectorAll("[data-action]").forEach((btn) => {
      btn.addEventListener("click", () => {
        const action = btn.getAttribute("data-action");
        if (action === "zoom-in") {
          this.zoomBy(1.15);
        } else if (action === "zoom-out") {
          this.zoomBy(1 / 1.15);
        } else if (action === "reset") {
          this.resetView();
        }
      });
    });
  }

  zoomBy(factor) {
    this._zoom = clamp(this._zoom * factor, 0.35, 4);
    this.applyTransform();
    this.updateStatus();
  }

  resetView() {
    this._zoom = 1;
    this._panX = 0;
    this._panY = 0;
    this.applyTransform();
    this.updateStatus();
  }

  applyTransform() {
    const layer = this.shadowRoot?.querySelector(".content-layer");
    if (!layer) {
      return;
    }
    layer.style.transform = `translate(${this._panX}px, ${this._panY}px) scale(${this._zoom})`;
  }

  updateStatus() {
    const status = this.shadowRoot?.querySelector(".status");
    if (!status) {
      return;
    }
    const activeGroup = this._activeGroupId ? ` · 分组 ${this._activeGroupId}` : "";
    status.textContent = `缩放 ${Math.round(this._zoom * 100)}% · 拖拽平移${activeGroup}`;
  }

  worldTargetsConfig() {
    const props = this.props || {};
    return props.worldTargets || props.world_targets || {};
  }

  readAnchor(anchorLike) {
    if (!anchorLike) return null;
    const config = this.worldTargetsConfig();
    if (typeof anchorLike === "string") {
      return this.readAnchor(config.anchors?.[anchorLike] || config.anchorMap?.[anchorLike]);
    }
    const x = Number(anchorLike.x);
    const y = Number(anchorLike.y);
    if (!Number.isFinite(x) || !Number.isFinite(y)) {
      return null;
    }
    return { x, y };
  }

  resolveCameraPreset(cameraPreset) {
    const config = this.worldTargetsConfig();
    if (!cameraPreset) return null;
    return config.cameraPresets?.[cameraPreset] || config.camera_presets?.[cameraPreset] || null;
  }

  resolveEntityTarget(entityId) {
    const config = this.worldTargetsConfig();
    if (!entityId) return null;
    return config.entities?.[entityId] || null;
  }

  resolveGroupTarget(groupId) {
    const config = this.worldTargetsConfig();
    if (!groupId) return null;
    return config.groups?.[groupId] || null;
  }

  normalizeIdList(values) {
    return (Array.isArray(values) ? values : [])
      .map((value) => String(value || "").trim())
      .filter(Boolean);
  }

  focusAnchor(anchorLike, zoom) {
    const anchor = this.readAnchor(anchorLike);
    if (!anchor) return false;
    const viewport = this.shadowRoot?.querySelector(".viewport");
    if (!(viewport instanceof HTMLElement)) return false;
    const nextZoom = Number.isFinite(Number(zoom)) ? clamp(Number(zoom), 0.35, 4) : this._zoom;
    const width = viewport.clientWidth || 1;
    const height = viewport.clientHeight || 1;
    const box = this._svgViewBox || { width: 800, height: 600 };
    const baseX = (anchor.x / Math.max(1, box.width)) * width;
    const baseY = (anchor.y / Math.max(1, box.height)) * height;
    const centerX = width / 2;
    const centerY = height / 2;
    this._zoom = nextZoom;
    this._panX = (centerX - baseX) * nextZoom;
    this._panY = (centerY - baseY) * nextZoom;
    this.applyTransform();
    this.updateStatus();
    return true;
  }

  setGroupVisible(groupId, visible) {
    const normalized = String(groupId || "").trim();
    if (!normalized) return false;
    if (visible) {
      this._hiddenGroups.delete(normalized);
    } else {
      this._hiddenGroups.add(normalized);
    }
    this.refreshWorldState();
    return true;
  }

  refreshWorldState() {
    const layer = this.shadowRoot?.querySelector(".content-layer");
    if (!layer) {
      return;
    }
    layer.querySelectorAll("[data-shape-id]").forEach((node) => {
      const shapeId = String(node.getAttribute("data-shape-id") || "").trim();
      const hidden = this.isShapeHidden(shapeId);
      node.classList.toggle("world-shape-active", this._activeShapeIds.has(shapeId));
      node.classList.toggle("world-shape-hidden", hidden);
    });
    layer.querySelectorAll("[data-hotspot-id]").forEach((node) => {
      const hotspotId = String(node.getAttribute("data-hotspot-id") || "").trim();
      const hidden = this.isHotspotHidden(hotspotId);
      node.classList.toggle("hotspot-active", this._activeHotspotIds.has(hotspotId));
      node.classList.toggle("hotspot-hidden", hidden);
    });
  }

  isShapeHidden(shapeId) {
    if (!shapeId) return false;
    for (const hiddenGroupId of this._hiddenGroups) {
      const group = this.resolveGroupTarget(hiddenGroupId);
      if (this.normalizeIdList(group?.shapeIds).includes(shapeId)) {
        return true;
      }
    }
    return false;
  }

  isHotspotHidden(hotspotId) {
    if (!hotspotId) return false;
    for (const hiddenGroupId of this._hiddenGroups) {
      const group = this.resolveGroupTarget(hiddenGroupId);
      if (this.normalizeIdList(group?.hotspotIds).includes(hotspotId)) {
        return true;
      }
    }
    return false;
  }

  applyWorldTarget(target) {
    if (!target || typeof target !== "object") {
      return false;
    }
    this._lastWorldTarget = target;
    const group = this.resolveGroupTarget(target.groupId);
    const entity = this.resolveEntityTarget(target.entityId);
    const presetFromEntity = entity?.cameraPreset || entity?.camera_preset || "";
    const preset =
      this.resolveCameraPreset(target.cameraPreset || presetFromEntity) || null;
    const resolved = {
      ...(group && typeof group === "object" ? group : {}),
      ...(entity && typeof entity === "object" ? entity : {}),
      ...(preset && typeof preset === "object" ? preset : {}),
      type: String(target.type || "").trim(),
      groupId:
        String(target.groupId || preset?.groupId || entity?.groupId || group?.id || "").trim(),
      entityId: String(target.entityId || entity?.id || "").trim(),
      cameraPreset:
        String(target.cameraPreset || preset?.id || presetFromEntity || "").trim(),
    };
    const shapeIds = this.normalizeIdList(
      resolved.shapeIds || resolved.shape_ids || entity?.shapeIds || group?.shapeIds,
    );
    const hotspotIds = this.normalizeIdList(
      resolved.hotspotIds || resolved.hotspot_ids || entity?.hotspotIds || group?.hotspotIds,
    );
    this._activeShapeIds = new Set(shapeIds);
    this._activeHotspotIds = new Set(hotspotIds);
    this._activeGroupId = resolved.groupId;
    if (resolved.type === "show_group" || resolved.type === "showGroup") {
      this.setGroupVisible(resolved.groupId, true);
      return true;
    }
    if (resolved.type === "hide_group" || resolved.type === "hideGroup") {
      this.setGroupVisible(resolved.groupId, false);
      return true;
    }
    if (resolved.groupId) {
      this._hiddenGroups.delete(resolved.groupId);
    }
    const didFocus = this.focusAnchor(
      resolved.anchor || resolved.anchorId || resolved.anchor_id,
      resolved.zoom,
    );
    if (!didFocus && Number.isFinite(Number(resolved.zoom))) {
      this._zoom = clamp(Number(resolved.zoom), 0.35, 4);
      this.applyTransform();
      this.updateStatus();
    }
    this.refreshWorldState();
    return true;
  }

  async loadContent() {
    const props = this.props || {};
    const kind = String(props.kind || "svg").trim().toLowerCase();
    const src = String(props.src || "").trim();
    const layer = this.shadowRoot?.querySelector(".content-layer");
    if (!layer || !src) {
      return;
    }
    if (kind === "image") {
      layer.innerHTML = `<img class="media" src="${escapeHtml(src)}" alt="" draggable="false" />`;
      this.renderHotspots(layer);
      return;
    }
    if (kind === "map" || kind === "gl") {
      layer.innerHTML = `<div class="placeholder">kind=${escapeHtml(kind)} 请使用 map.maplibre 或未来 3D 宿主</div>`;
      return;
    }
    try {
      const response = await fetch(src, { credentials: "same-origin" });
      if (!response.ok) {
        throw new Error(String(response.status));
      }
      const text = await response.text();
      layer.innerHTML = `<div class="svg-wrap">${text}</div>`;
      const svg = layer.querySelector("svg");
      if (svg) {
        svg.setAttribute("width", "100%");
        svg.setAttribute("height", "100%");
        svg.setAttribute("preserveAspectRatio", "xMidYMid meet");
        const viewBox = String(svg.getAttribute("viewBox") || "").trim().split(/\s+/);
        if (viewBox.length === 4) {
          const width = Number(viewBox[2]);
          const height = Number(viewBox[3]);
          if (Number.isFinite(width) && Number.isFinite(height) && width > 0 && height > 0) {
            this._svgViewBox = { width, height };
          }
        }
        svg.querySelectorAll("[id]").forEach((node) => {
          node.setAttribute("data-shape-id", String(node.getAttribute("id") || "").trim());
        });
      }
      this.renderHotspots(layer);
      this.refreshWorldState();
    } catch (error) {
      layer.innerHTML = `<div class="placeholder">无法加载底图：${escapeHtml(String(error))}</div>`;
    }
    this.resetView();
    if (this._lastWorldTarget) {
      this.applyWorldTarget(this._lastWorldTarget);
    }
  }

  renderHotspots(layer) {
    const props = this.props || {};
    const hotspots = Array.isArray(props.hotspots) ? props.hotspots : [];
    if (!hotspots.length) {
      return;
    }
    const box = document.createElement("div");
    box.className = "hotspots";
    for (const spot of hotspots) {
      const x = Number(spot.x);
      const y = Number(spot.y);
      if (!Number.isFinite(x) || !Number.isFinite(y)) {
        continue;
      }
      const label = String(spot.label || spot.id || "").trim();
      const pin = document.createElement("div");
      pin.className = "hotspot";
      pin.setAttribute("data-hotspot-id", String(spot.id || "").trim());
      pin.style.left = `${x}%`;
      pin.style.top = `${y}%`;
      pin.textContent = label;
      box.appendChild(pin);
    }
    layer.appendChild(box);
  }

  render() {
    const props = this.props || {};
    const layout = resolveCockpitBleedLayout(props);
    const focusInset = resolveMapFocusInset(props);
    const bg =
      props.backgroundColor ||
      props.background_color ||
      (props.background && props.background.color) ||
      "#0a2848";
    const showGuide = layout.showFocusGuide === true;
    const frameBorder = layout.focusFrameBorder || focusInset?.focusFrameBorder || "";
    const frameRadius =
      layout.focusFrameRadius || focusInset?.focusFrameRadius || "4px";
    const guideStyle =
      showGuide && frameBorder
        ? focusFrameGuideStyle(frameBorder, frameRadius)
        : "";

    this.shadowRoot.innerHTML = `
      <style>
        :host {
          ${layout.host}
          box-sizing: border-box;
        }
        .wrap {
          ${layout.wrap}
          background: ${escapeHtml(String(bg))};
        }
        .viewport {
          ${layout.content}
          cursor: grab;
          touch-action: none;
        }
        .viewport:active { cursor: grabbing; }
        .content-layer {
          position: absolute;
          inset: 0;
          display: flex;
          align-items: center;
          justify-content: center;
          transform-origin: center center;
          will-change: transform;
        }
        .svg-wrap, .svg-wrap svg {
          width: 100%;
          height: 100%;
          display: block;
        }
        .media {
          max-width: 100%;
          max-height: 100%;
          object-fit: contain;
          user-select: none;
          pointer-events: none;
        }
        .hotspots {
          position: absolute;
          inset: 0;
          pointer-events: none;
        }
        .hotspot {
          position: absolute;
          transform: translate(-50%, -50%);
          min-width: 28px;
          height: 28px;
          padding: 0 6px;
          border-radius: 14px;
          background: rgba(56, 160, 240, 0.92);
          color: #fff;
          font: 600 14px/28px system-ui, sans-serif;
          text-align: center;
          box-shadow: 0 0 12px rgba(56, 160, 240, 0.55);
          transition: transform 160ms ease, opacity 160ms ease, box-shadow 160ms ease;
        }
        .hotspot-active {
          transform: translate(-50%, -50%) scale(1.12);
          box-shadow: 0 0 18px rgba(125, 211, 252, 0.85);
          background: rgba(14, 165, 233, 1);
        }
        .hotspot-hidden {
          opacity: 0.18;
        }
        .svg-wrap [data-shape-id] {
          transition: opacity 160ms ease, filter 160ms ease, stroke-width 160ms ease;
          transform-box: fill-box;
          transform-origin: center;
        }
        .svg-wrap .world-shape-active {
          filter: drop-shadow(0 0 10px rgba(125, 211, 252, 0.6));
        }
        .svg-wrap .world-shape-hidden {
          opacity: 0.18;
        }
        .toolbar {
          position: absolute;
          ${layout.toolbarPos}
          display: flex;
          gap: 6px;
          z-index: 3;
        }
        .toolbar button {
          border: 1px solid rgba(56, 160, 240, 0.45);
          background: rgba(10, 36, 72, 0.82);
          color: #e8f0ff;
          border-radius: 4px;
          padding: 4px 10px;
          font: 500 13px/1.4 system-ui, sans-serif;
          cursor: pointer;
        }
        .toolbar button:hover {
          background: rgba(14, 52, 96, 0.95);
        }
        .status {
          position: absolute;
          ${layout.statusPos}
          z-index: 3;
          padding: 4px 10px;
          border-radius: 4px;
          background: rgba(10, 36, 72, 0.78);
          border: 1px solid rgba(56, 160, 240, 0.28);
          color: #a8c8e6;
          font: 500 12px/1.4 system-ui, sans-serif;
          pointer-events: none;
        }
        .focus-guide {
          position: absolute;
          top: var(--map-focus-top, 0);
          right: var(--map-focus-right, 0);
          bottom: var(--map-focus-bottom, 0);
          left: var(--map-focus-left, 0);
          pointer-events: none;
          z-index: 2;
          box-sizing: border-box;
        }
        .placeholder {
          color: rgba(255,255,255,0.55);
          font: 14px/1.5 system-ui, sans-serif;
          padding: 24px;
        }
      </style>
      <div class="wrap">
        <div class="viewport">
          <div class="content-layer"></div>
        </div>
        ${showGuide && focusInset ? `<div class="focus-guide" style="${guideStyle}" aria-hidden="true"></div>` : ""}
        <div class="toolbar">
          <button type="button" data-action="zoom-in" title="放大">＋</button>
          <button type="button" data-action="zoom-out" title="缩小">－</button>
          <button type="button" data-action="reset" title="复位">复位</button>
        </div>
        <div class="status">缩放 100% · 拖拽平移</div>
      </div>
    `;
  }
}

if (!customElements.get(TAG)) {
  customElements.define(TAG, MeiCockpitBasemapStage);
}

export { MeiCockpitBasemapStage };
