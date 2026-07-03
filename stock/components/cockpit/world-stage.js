import { escapeHtml, parseProps, resolveWorldRef } from "./shared.js";
import * as THREE from "../vendor/three/three.module.min.js";
import { OrbitControls } from "../vendor/three/OrbitControls.js";
import {
  ensureWorldStageInputPlane,
  layoutWorldStageInputPlane,
  resolveCockpitMapToolHost,
  setWorldStageInputPlaneActive,
} from "./cockpit-stage-overlay.js";
import { resolveCockpitStageSurface } from "./map-focus-inset.js";
import { createWorldPropScreenMesh } from "./world-prop-screen.js";

const TAG = "mei-world-stage";
const WORLD_RUNTIME_INSTANCES = new Set();
const boot = (window.__meiLangBoot = window.__meiLangBoot || {});
const ORBIT_MIN_DISTANCE = 5;
const ORBIT_MAX_DISTANCE = 96;
const ORBIT_MIN_POLAR_DEG = 18;
const ORBIT_MAX_POLAR_DEG = 80;
const PICK_MOVE_THRESHOLD_PX = 6;
const FOOTPRINT_LAYER = {
  site_outline: { lift: 0.012, renderOrder: 10, opacity: 0.18, color: 0x93c5fd },
  flat_fill: { lift: 0.028, renderOrder: 20, opacity: 0.72, color: 0x2d7fb0 },
  default: { lift: 0.018, renderOrder: 15, opacity: 0.55, color: 0x1f4f74 },
  extrude_shell: { lift: 0.034, renderOrder: 30 },
};

function degToRad(value) {
  return (Number(value) * Math.PI) / 180;
}

function metersPerDegree(lat) {
  const latRad = degToRad(lat);
  return {
    lng: 111320 * Math.cos(latRad),
    lat: 111320,
  };
}

function geoToLocal(lng, lat, origin) {
  const scale = metersPerDegree(origin.lat);
  return {
    x: (Number(lng) - origin.lng) * scale.lng,
    z: (origin.lat - Number(lat)) * scale.lat,
  };
}

function parseHexColor(hex, fallback = 0xffffff) {
  const raw = String(hex || "").trim().replace("#", "");
  if (!raw) return fallback;
  const expanded =
    raw.length === 3 ? raw.split("").map((c) => c + c).join("") : raw;
  const parsed = Number.parseInt(expanded, 16);
  return Number.isFinite(parsed) ? parsed : fallback;
}

function resolveInjectedWorldPlan(worldRef) {
  const ref = String(worldRef || "park_world").trim();
  const worlds = window.__mei?.world_plan?.worlds;
  if (worlds && worlds[ref]) {
    return worlds[ref];
  }
  return null;
}

function featureMapFromGeojson(geojson) {
  const out = new Map();
  for (const feature of geojson?.features || []) {
    const entityId = readEntityId(feature);
    if (entityId) out.set(entityId, feature);
  }
  return out;
}

function footprintLayerFor(renderFamily) {
  if (renderFamily === "site_outline") return FOOTPRINT_LAYER.site_outline;
  if (renderFamily === "flat_fill") return FOOTPRINT_LAYER.flat_fill;
  if (renderFamily === "extrude_shell") return FOOTPRINT_LAYER.extrude_shell;
  return FOOTPRINT_LAYER.default;
}

function createFootprintSurfaceMaterial({ color, opacity }) {
  const transparent = opacity < 0.999;
  return new THREE.MeshStandardMaterial({
    color,
    roughness: transparent ? 0.42 : 0.92,
    metalness: transparent ? 0.04 : 0.02,
    transparent,
    opacity,
    side: THREE.DoubleSide,
    depthWrite: !transparent,
    polygonOffset: transparent,
    polygonOffsetFactor: transparent ? -1 : 0,
    polygonOffsetUnits: transparent ? -1 : 0,
  });
}

function applyFootprintLayer(mesh, layer) {
  if (!mesh || !layer) return;
  mesh.position.y = layer.lift;
  mesh.renderOrder = layer.renderOrder;
}

function ringToShape(ring, origin) {
  const shape = new THREE.Shape();
  ring.forEach((coord, index) => {
    const point = geoToLocal(coord[0], coord[1], origin);
    if (index === 0) {
      shape.moveTo(point.x, point.z);
    } else {
      shape.lineTo(point.x, point.z);
    }
  });
  return shape;
}

function footprintEnvelope(ring, origin) {
  if (!Array.isArray(ring) || ring.length < 3) {
    return {
      center: { x: 0, z: 0 },
      halfW: 2.2,
      halfD: 2.2,
      ring: [],
    };
  }
  let minX = Infinity;
  let maxX = -Infinity;
  let minZ = Infinity;
  let maxZ = -Infinity;
  for (const coord of ring) {
    const point = geoToLocal(coord[0], coord[1], origin);
    minX = Math.min(minX, point.x);
    maxX = Math.max(maxX, point.x);
    minZ = Math.min(minZ, point.z);
    maxZ = Math.max(maxZ, point.z);
  }
  return {
    center: { x: (minX + maxX) / 2, z: (minZ + maxZ) / 2 },
    halfW: Math.max((maxX - minX) / 2, 1.2),
    halfD: Math.max((maxZ - minZ) / 2, 1.2),
    ring,
  };
}

function resolveBuildingIdFromParent(parentId, plan) {
  const parent = String(parentId || "").trim();
  if (!parent) return "";
  const primitives = Array.isArray(plan?.primitives) ? plan.primitives : [];
  const direct = primitives.find((item) => String(item?.id || "") === parent);
  if (!direct) return parent;
  const kind = String(direct.kind || "").trim();
  if (kind === "building") return parent;
  if (kind === "floor") return String(direct.parent || parent).trim();
  return parent;
}

function readEntityId(feature) {
  return String(feature?.properties?.entityId || feature?.id || "").trim();
}

function offsetFromCamera(camera, target) {
  return camera.position.clone().sub(target);
}

function applySphericalOffset(camera, target, mutateSpherical) {
  const offset = offsetFromCamera(camera, target);
  const spherical = new THREE.Spherical().setFromVector3(offset);
  mutateSpherical(spherical);
  offset.setFromSpherical(spherical);
  camera.position.copy(target).add(offset);
  camera.lookAt(target);
}

class MeiWorldStage extends HTMLElement {
  constructor() {
    super();
    this._scene = null;
    this._camera = null;
    this._renderer = null;
    this._controls = null;
    this._unbindEntityPick = null;
    this._meshes = new Map();
    this._groups = new Map();
    this._hiddenGroups = new Set();
    this._interiorBuildingIds = new Set();
    this._buildingSemantics = new Map();
    this._footprintsByEntity = new Map();
    this._viewLayers = [];
    this._viewLayerVisibility = new Map();
    this._worldPlan = null;
    this._pendingWorldTarget = null;
    this._propsSignature = "";
    this._animationFrame = 0;
    this._siteOrigin = { lng: 106.38224, lat: 29.62396 };
    this._resizeObserver = null;
    this._inputSurface = null;
    this._controlsDom = null;
    this._onWorldStageEntered = null;
    this._onWorldStageExited = null;
    this._onViewportStageLayout = null;
  }

  connectedCallback() {
    this.refreshFromProps({ forceRender: true });
    WORLD_RUNTIME_INSTANCES.add(this);
    boot.activeWorldStage = this;
    if (!this._onWorldStageEntered) {
      this._onWorldStageEntered = () => this.activateInteractionSurface();
      window.addEventListener("mei:world-stage-entered", this._onWorldStageEntered);
    }
    if (!this._onWorldStageExited) {
      this._onWorldStageExited = () => this.deactivateInteractionSurface();
      window.addEventListener("mei:world-stage-exited", this._onWorldStageExited);
    }
    if (!this._onViewportStageLayout) {
      this._onViewportStageLayout = () => this.syncInteractionSurfaceLayout();
      window.addEventListener("meilang:viewport-stage-layout", this._onViewportStageLayout);
      window.addEventListener("resize", this._onViewportStageLayout, { passive: true });
    }
    if (document.documentElement.classList.contains("mei-world-stage-active")) {
      this.activateInteractionSurface();
    }
    if (!this._onPreviewUpdated) {
      this._onPreviewUpdated = () => this.refreshFromProps({ forceRender: true });
      window.addEventListener("meilang:preview-updated", this._onPreviewUpdated);
    }
  }

  disconnectedCallback() {
    WORLD_RUNTIME_INSTANCES.delete(this);
    if (boot.activeWorldStage === this) {
      boot.activeWorldStage = null;
    }
    if (this._onWorldStageEntered) {
      window.removeEventListener("mei:world-stage-entered", this._onWorldStageEntered);
      this._onWorldStageEntered = null;
    }
    if (this._onWorldStageExited) {
      window.removeEventListener("mei:world-stage-exited", this._onWorldStageExited);
      this._onWorldStageExited = null;
    }
    if (this._onViewportStageLayout) {
      window.removeEventListener("meilang:viewport-stage-layout", this._onViewportStageLayout);
      window.removeEventListener("resize", this._onViewportStageLayout);
      this._onViewportStageLayout = null;
    }
    this.deactivateInteractionSurface();
    this.disposeScene();
    if (this._onPreviewUpdated) {
      window.removeEventListener("meilang:preview-updated", this._onPreviewUpdated);
      this._onPreviewUpdated = null;
    }
  }

  refreshFromProps(options = {}) {
    this.props = parseProps(this);
    const nextSignature = String(this.getAttribute("data-props") || "");
    const shouldRender = options.forceRender === true || nextSignature !== this._propsSignature;
    this._propsSignature = nextSignature;
    if (!this.shadowRoot) {
      this.attachShadow({ mode: "open" });
    }
    if (shouldRender) {
      void this.bootstrapScene();
    } else if (this._pendingWorldTarget) {
      this.applyWorldTarget(this._pendingWorldTarget);
    }
  }

  disposeScene() {
    if (this._animationFrame) {
      cancelAnimationFrame(this._animationFrame);
      this._animationFrame = 0;
    }
    if (this._resizeObserver) {
      this._resizeObserver.disconnect();
      this._resizeObserver = null;
    }
    if (this._unbindEntityPick) {
      this._unbindEntityPick();
      this._unbindEntityPick = null;
    }
    if (this._controls) {
      this._controls.dispose();
      this._controls = null;
    }
    this._controlsDom = null;
    this._inputSurface = null;
    if (this._renderer) {
      this._renderer.dispose();
      this._renderer = null;
    }
    this._scene = null;
    this._camera = null;
    this._controls = null;
    this._meshes.clear();
    this._groups.clear();
  }

  renderChrome() {
    const root = this.shadowRoot;
    if (!root) return;
    root.innerHTML = `
      <style>
        :host {
          display: block;
          width: 100%;
          height: 100%;
          min-height: 320px;
          position: relative;
          background: radial-gradient(circle at 30% 20%, #123a62 0%, #071526 58%, #030a14 100%);
        }
        .viewport {
          width: 100%;
          height: 100%;
          min-height: inherit;
          position: relative;
        }
        canvas {
          display: block;
          width: 100%;
          height: 100%;
          touch-action: none;
          cursor: grab;
        }
        canvas:active {
          cursor: grabbing;
        }
        .status {
          position: absolute;
          left: 12px;
          bottom: 12px;
          z-index: 3;
          padding: 6px 10px;
          border-radius: 8px;
          background: rgba(8, 28, 58, 0.72);
          color: #94a3b8;
          font-size: 12px;
        }
        .error {
          position: absolute;
          inset: 0;
          display: grid;
          place-items: center;
          color: #fca5a5;
          padding: 24px;
          text-align: center;
        }
        .layer-control {
          position: absolute;
          top: 12px;
          right: 12px;
          z-index: 4;
          min-width: 168px;
          padding: 10px 12px;
          border-radius: 10px;
          background: rgba(8, 28, 58, 0.82);
          color: #e2e8f0;
          font-size: 12px;
          box-shadow: 0 8px 24px rgba(2, 8, 23, 0.35);
        }
        .layer-control h4 {
          margin: 0 0 8px;
          font-size: 12px;
          font-weight: 600;
          color: #93c5fd;
        }
        .layer-control label {
          display: flex;
          align-items: center;
          gap: 8px;
          padding: 4px 0;
          cursor: pointer;
        }
      </style>
      <div class="viewport">
        <div class="layer-control" data-role="layer-control" hidden>
          <h4>视口图层</h4>
          <div data-role="layer-list"></div>
        </div>
        <div class="status" data-role="status">world_view</div>
        <div class="error" data-role="error" hidden></div>
      </div>
    `;
  }

  async bootstrapScene() {
    this.renderChrome();
    const errorEl = this.shadowRoot?.querySelector('[data-role="error"]');
    const viewport = this.shadowRoot?.querySelector(".viewport");
    if (!viewport) return;
    try {
      this.disposeScene();
      const worldRef = resolveWorldRef(this.props, this) || "park_world";
      this._worldPlan = resolveInjectedWorldPlan(worldRef) || this.props?.worldPlan || null;
      const site = this._worldPlan?.site || this.props?.worldSpec?.site || {};
      const origin = site.origin || site;
      this._siteOrigin = {
        lng: Number(origin.lng ?? site.originLng ?? site.origin_lng ?? 106.38224),
        lat: Number(origin.lat ?? site.originLat ?? site.origin_lat ?? 29.62396),
      };
      this._scene = new THREE.Scene();
      this._scene.background = new THREE.Color(0x071526);
      this._scene.fog = new THREE.Fog(0x071526, 80, 260);
      const width = Math.max(320, this.clientWidth || viewport.clientWidth || 320);
      const height = Math.max(240, this.clientHeight || viewport.clientHeight || 240);
      this._camera = new THREE.PerspectiveCamera(52, width / height, 0.1, 500);
      this._camera.position.set(26, 34, 38);
      this._renderer = new THREE.WebGLRenderer({
        antialias: true,
        alpha: false,
        logarithmicDepthBuffer: true,
      });
      this._renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
      this._renderer.setSize(width, height, false);
      viewport.appendChild(this._renderer.domElement);
      this._controls = this.createOrbitControls(this._renderer.domElement);
      this._controlsDom = this._renderer.domElement;
      this._unbindEntityPick = this.bindEntityPicking(this._renderer.domElement);
      const ambient = new THREE.AmbientLight(0xb9d7ff, 0.72);
      const sun = new THREE.DirectionalLight(0xffffff, 0.95);
      sun.position.set(40, 80, 20);
      this._scene.add(ambient, sun);
      const ground = new THREE.Mesh(
        new THREE.PlaneGeometry(180, 180),
        new THREE.MeshStandardMaterial({ color: 0x12324f, roughness: 0.92, metalness: 0.04 }),
      );
      ground.rotation.x = -Math.PI / 2;
      ground.position.y = -0.045;
      ground.renderOrder = 0;
      ground.receiveShadow = true;
      this._scene.add(ground);
      await this.loadWorldContent();
      this.bindResize(viewport);
      this.animate();
      if (errorEl) {
        errorEl.hidden = true;
        errorEl.textContent = "";
      }
      if (this._pendingWorldTarget) {
        this.applyWorldTarget(this._pendingWorldTarget);
      } else {
        this.applyCameraPreset(this.resolveWorldTargetPreset("park_world_overview"));
      }
      if (document.documentElement.classList.contains("mei-world-stage-active")) {
        this.activateInteractionSurface();
      }
    } catch (error) {
      if (errorEl) {
        errorEl.hidden = false;
        errorEl.textContent = String(error?.message || error);
      }
    }
  }

  connectControlsToDom(domElement) {
    if (!this._controls || !(domElement instanceof HTMLElement)) {
      return;
    }
    if (this._controlsDom === domElement) {
      return;
    }
    this._controls.disconnect();
    this._controls.connect(domElement);
    this._controlsDom = domElement;
    domElement.tabIndex = 0;
  }

  syncInteractionSurfaceLayout() {
    if (!this._inputSurface || !document.documentElement.classList.contains("mei-world-stage-active")) {
      return;
    }
    const stage = resolveCockpitStageSurface(this);
    if (!stage) return;
    const mapHost = resolveCockpitMapToolHost();
    layoutWorldStageInputPlane(this._inputSurface, stage, mapHost?._layout?.focusInsetPx);
  }

  activateInteractionSurface() {
    if (!this._controls) return;
    setWorldStageInputPlaneActive(true);
    const surface = ensureWorldStageInputPlane(this);
    if (surface instanceof HTMLElement) {
      this._inputSurface = surface;
      this.connectControlsToDom(surface);
      this.syncInteractionSurfaceLayout();
      if (typeof this._unbindEntityPick === "function") {
        this._unbindEntityPick();
      }
      this._unbindEntityPick = this.bindEntityPicking(surface);
      surface.focus({ preventScroll: true });
      return;
    }
    const canvas = this._renderer?.domElement;
    if (canvas instanceof HTMLElement) {
      this.connectControlsToDom(canvas);
      canvas.focus({ preventScroll: true });
    }
  }

  deactivateInteractionSurface() {
    setWorldStageInputPlaneActive(false);
    const canvas = this._renderer?.domElement;
    if (canvas instanceof HTMLElement && this._controls) {
      this.connectControlsToDom(canvas);
      if (typeof this._unbindEntityPick === "function") {
        this._unbindEntityPick();
      }
      this._unbindEntityPick = this.bindEntityPicking(canvas);
    }
    this._inputSurface = null;
  }

  createOrbitControls(domElement) {
    const controls = new OrbitControls(this._camera, domElement);
    controls.target.set(0, 0.8, 0);
    controls.minDistance = ORBIT_MIN_DISTANCE;
    controls.maxDistance = ORBIT_MAX_DISTANCE;
    controls.minPolarAngle = degToRad(ORBIT_MIN_POLAR_DEG);
    controls.maxPolarAngle = degToRad(ORBIT_MAX_POLAR_DEG);
    controls.enableDamping = true;
    controls.dampingFactor = 0.08;
    controls.screenSpacePanning = true;
    controls.zoomSpeed = 1.05;
    controls.rotateSpeed = 0.85;
    controls.panSpeed = 0.9;
    controls.addEventListener("change", () => this.clampControlsTarget());
    return controls;
  }

  clampControlsTarget() {
    if (!this._controls) return;
    const radius = 48;
    this._controls.target.x = THREE.MathUtils.clamp(this._controls.target.x, -radius, radius);
    this._controls.target.z = THREE.MathUtils.clamp(this._controls.target.z, -radius, radius);
    this._controls.target.y = THREE.MathUtils.clamp(this._controls.target.y, 0, 16);
  }

  syncControlsFromCamera(saveHome = true) {
    if (!this._controls || !this._camera) return;
    this._controls.update();
    if (saveHome) {
      this._controls.saveState();
    }
  }

  navZoomBy(factor) {
    if (!this._controls || !this._camera) return;
    const target = this._controls.target;
    applySphericalOffset(this._camera, target, (spherical) => {
      spherical.radius = THREE.MathUtils.clamp(
        spherical.radius * factor,
        this._controls.minDistance,
        this._controls.maxDistance,
      );
    });
    this._controls.update();
  }

  navRotateBearing(deg) {
    if (!this._controls || !this._camera) return;
    applySphericalOffset(this._camera, this._controls.target, (spherical) => {
      spherical.theta -= degToRad(deg);
    });
    this._controls.update();
  }

  navAdjustPitch(deg) {
    if (!this._controls || !this._camera) return;
    applySphericalOffset(this._camera, this._controls.target, (spherical) => {
      spherical.phi = THREE.MathUtils.clamp(
        spherical.phi - degToRad(deg),
        this._controls.minPolarAngle,
        this._controls.maxPolarAngle,
      );
    });
    this._controls.update();
  }

  bindEntityPicking(domElement) {
    if (!domElement || !this._camera || !this._scene) {
      return () => {};
    }
    const raycaster = new THREE.Raycaster();
    const pointer = new THREE.Vector2();
    const canvas = this._renderer?.domElement;
    let pointerDown = null;

    const resolvePointerCoords = (event) => {
      const rect = (canvas || domElement).getBoundingClientRect();
      if (rect.width <= 0 || rect.height <= 0) {
        return null;
      }
      return {
        x: ((event.clientX - rect.left) / rect.width) * 2 - 1,
        y: -((event.clientY - rect.top) / rect.height) * 2 + 1,
      };
    };

    const onPointerDown = (event) => {
      if (event.button !== 0) return;
      pointerDown = {
        x: event.clientX,
        y: event.clientY,
        id: event.pointerId,
      };
    };
    const clearPointerDown = () => {
      pointerDown = null;
    };
    const onPointerUp = (event) => {
      if (!pointerDown || pointerDown.id !== event.pointerId) return;
      const dx = event.clientX - pointerDown.x;
      const dy = event.clientY - pointerDown.y;
      clearPointerDown();
      if (dx * dx + dy * dy > PICK_MOVE_THRESHOLD_PX * PICK_MOVE_THRESHOLD_PX) {
        return;
      }
      const coords = resolvePointerCoords(event);
      if (!coords) return;
      pointer.x = coords.x;
      pointer.y = coords.y;
      raycaster.setFromCamera(pointer, this._camera);
      const meshes = [...this._meshes.values()].filter((mesh) => mesh.visible);
      const hits = raycaster.intersectObjects(meshes, false);
      if (!hits.length) return;
      const mesh = hits[0].object;
      const entityId = String(mesh.userData?.entityId || mesh.name || "").trim();
      if (!entityId) return;
      this.handleEntityPick(entityId, mesh);
    };

    domElement.addEventListener("pointerdown", onPointerDown);
    domElement.addEventListener("pointerup", onPointerUp);
    domElement.addEventListener("pointercancel", clearPointerDown);
    return () => {
      domElement.removeEventListener("pointerdown", onPointerDown);
      domElement.removeEventListener("pointerup", onPointerUp);
      domElement.removeEventListener("pointercancel", clearPointerDown);
    };
  }

  handleEntityPick(entityId, mesh) {
    const entity = this.resolveWorldTargetEntity(entityId);
    window.dispatchEvent(
      new CustomEvent("mei:world-entity-pick", {
        detail: {
          entityId,
          meshId: mesh?.name || entityId,
          device: Boolean(mesh?.userData?.device),
        },
      }),
    );
    if (entity?.cameraPreset || entity?.camera_preset) {
      const preset = this.resolveWorldTargetPreset(entity.cameraPreset || entity.camera_preset);
      if (preset) {
        this.applyCameraPreset({ ...preset, targetEntity: entityId });
        return;
      }
    }
    this.focusEntity(entityId);
  }

  bindResize(viewport) {
    if (typeof ResizeObserver === "undefined" || !this._renderer || !this._camera) {
      return;
    }
    this._resizeObserver = new ResizeObserver(() => {
      const width = Math.max(320, viewport.clientWidth || 320);
      const height = Math.max(240, viewport.clientHeight || 240);
      this._renderer.setSize(width, height, false);
      this._camera.aspect = width / height;
      this._camera.updateProjectionMatrix();
    });
    this._resizeObserver.observe(viewport);
  }

  animate() {
    if (!this._renderer || !this._scene || !this._camera) return;
    this._animationFrame = requestAnimationFrame(() => this.animate());
    this._controls?.update();
    this._renderer.render(this._scene, this._camera);
  }

  async loadWorldContent() {
    const worldRef = resolveWorldRef(this.props, this) || "park_world";
    const plan = this._worldPlan || resolveInjectedWorldPlan(worldRef);
    const worldSpec = this.props?.worldSpec || {};
    const inlineFootprint = plan?.emittedFootprint;
    const geoUrl = String(
      plan?.emittedFootprintUrl ||
        plan?.spatialSources?.[0]?.url ||
        worldSpec.footprintGeoJsonUrl ||
        worldSpec.footprint_geojson_url ||
        "",
    ).trim();
    if (!inlineFootprint && !geoUrl) {
      throw new Error("world_plan 缺少 emittedFootprint 或 footprint GeoJSON URL");
    }
    const geojson = inlineFootprint || (await this.fetchJson(geoUrl));
    if (!plan) {
      throw new Error(`world_plan 未注入：${worldRef}`);
    }
    this.buildFromWorldPlan(plan, geojson);
  }

  buildFromWorldPlan(plan, geojson) {
    this._worldPlan = plan;
    const featuresByEntity = featureMapFromGeojson(geojson);
    this._footprintsByEntity = featuresByEntity;
    const primitives = Array.isArray(plan.primitives) ? plan.primitives : [];
    const interiorParents = new Set();
    this._buildingSemantics = new Map();
    for (const prim of primitives) {
      const kind = String(prim.kind || "").trim();
      if (kind === "floor" || kind === "wall_ring" || kind === "roof") {
        const parent = String(prim.parent || "").trim();
        const buildingId = resolveBuildingIdFromParent(parent, plan);
        if (buildingId) interiorParents.add(buildingId);
      }
      if (kind === "building") {
        this._buildingSemantics.set(String(prim.id || ""), {
          height: Number(prim.height ?? prim.worldView?.shellHeight ?? 8.6),
          shellMaterial: prim.shellMaterial || prim.material || {},
          hasInterior: prim.hasInterior === true,
          interiorProfile: prim.interiorProfile || null,
          worldView: prim.worldView || {},
        });
      }
    }
    this._interiorBuildingIds = interiorParents;
    const deferred = [];
    for (const prim of primitives) {
      const kind = String(prim.kind || "").trim();
      if (kind === "floor" || kind === "wall_ring" || kind === "roof" || kind === "prop") {
        deferred.push(prim);
        continue;
      }
      this.mountPrimitiveMeshes(prim, featuresByEntity);
    }
    for (const prim of deferred) {
      this.mountPrimitiveMeshes(prim, featuresByEntity);
    }
    this.initViewLayers(plan.viewLayers || []);
  }

  mountPrimitiveMeshes(prim, featuresByEntity) {
    const built = this.buildPrimitiveMeshes(prim, featuresByEntity);
    for (const entry of built) {
      const mesh = entry.mesh;
      mesh.name = entry.meshId;
      mesh.userData.entityId = String(prim.id || entry.meshId);
      mesh.userData.layerTags = entry.layerTags || [entry.meshId];
      if (entry.shellMesh) mesh.userData.shellMesh = true;
      if (entry.roofMesh) mesh.userData.roofMesh = true;
      if (entry.device) mesh.userData.device = true;
      this._scene.add(mesh);
      this._meshes.set(entry.meshId, mesh);
      const groupIds = this.resolveEntityGroups(prim.id);
      groupIds.forEach((groupId) => {
        if (!this._groups.has(groupId)) {
          this._groups.set(groupId, new Set());
        }
        this._groups.get(groupId).add(entry.meshId);
      });
    }
  }

  buildPrimitiveMeshes(prim, featuresByEntity) {
    const kind = String(prim.kind || "").trim();
    const worldView = prim.worldView || {};
    const wvKind = String(worldView.kind || "").trim();
    const feature = featuresByEntity.get(String(prim.featureEntityId || prim.id || ""));
    const material = prim.material || {};
    const color = parseHexColor(material.color, 0x5d8fd6);
    const opacity = Number(material.opacity ?? 0.72);
    const results = [];

    if (kind === "building" && feature?.geometry?.type === "Polygon") {
      const layer = footprintLayerFor("extrude_shell");
      const ring = feature.geometry.coordinates?.[0] || [];
      const shape = ringToShape(ring, this._siteOrigin);
      const semantics = this._buildingSemantics.get(String(prim.id || "")) || {};
      const worldView = prim.worldView || semantics.worldView || {};
      const height = Number(
        prim.height ?? worldView.shellHeight ?? semantics.height ?? 8.6,
      );
      const geom = new THREE.ExtrudeGeometry(shape, { depth: height, bevelEnabled: false });
      geom.rotateX(-Math.PI / 2);
      const shellMaterial = prim.shellMaterial || semantics.shellMaterial || material;
      const mesh = new THREE.Mesh(
        geom,
        createFootprintSurfaceMaterial({
          color: parseHexColor(shellMaterial.color, 0xffd36b),
          opacity: Number(shellMaterial.opacity ?? 0.9),
        }),
      );
      applyFootprintLayer(mesh, {
        ...layer,
        lift: Number(worldView.lift ?? layer.lift),
      });
      results.push({
        meshId: `${prim.id}:shell`,
        mesh,
        layerTags: [`${prim.id}:shell`, prim.id],
        shellMesh: true,
      });
      return results;
    }

    if (
      (kind === "ground" || kind === "pool" || kind === "green") &&
      feature?.geometry?.type === "Polygon"
    ) {
      const renderFamily =
        wvKind === "site_outline"
          ? "site_outline"
          : wvKind === "flat_fill"
            ? "flat_fill"
            : "default";
      const layer = footprintLayerFor(renderFamily);
      const ring = feature.geometry.coordinates?.[0] || [];
      const shape = ringToShape(ring, this._siteOrigin);
      const geom = new THREE.ShapeGeometry(shape);
      geom.rotateX(-Math.PI / 2);
      const mesh = new THREE.Mesh(
        geom,
        createFootprintSurfaceMaterial({
          color: parseHexColor(material.color, layer.color),
          opacity: Number(material.opacity ?? worldView.opacity ?? layer.opacity),
        }),
      );
      applyFootprintLayer(mesh, {
        ...layer,
        lift: Number(worldView.lift ?? layer.lift),
      });
      results.push({ meshId: prim.id, mesh, layerTags: [prim.id] });
      return results;
    }

    if (kind === "route" && feature?.geometry?.type === "LineString") {
      const elevation = Number(worldView.elevation ?? 0.35);
      const radius = Number(worldView.radius ?? 0.45);
      const segments = Number(worldView.segments ?? 48);
      const points = (feature.geometry.coordinates || []).map((coord) => {
        const local = geoToLocal(coord[0], coord[1], this._siteOrigin);
        return new THREE.Vector3(local.x, elevation, local.z);
      });
      const curve = new THREE.CatmullRomCurve3(points);
      const geom = new THREE.TubeGeometry(curve, segments, radius, 8, false);
      const mesh = new THREE.Mesh(
        geom,
        new THREE.MeshStandardMaterial({
          color: parseHexColor(material.color, 0xfde68a),
          emissive: 0x5c4b12,
          emissiveIntensity: 0.2,
        }),
      );
      mesh.position.y = elevation;
      mesh.renderOrder = 40;
      results.push({ meshId: prim.id, mesh, layerTags: [prim.id] });
      return results;
    }

    if (kind === "floor") {
      const buildingId = resolveBuildingIdFromParent(prim.parent, this._worldPlan);
      const envelope = this.resolveBuildingFootprint(buildingId);
      if (!envelope) return results;
      const semantics = this._buildingSemantics.get(buildingId) || {};
      const profile = semantics.interiorProfile || {};
      const elevation = Number(
        prim.elevation ?? worldView.elevation ?? profile.floorElevation ?? 0.05,
      );
      const slab = prim.slab || material;
      const floorShape =
        envelope.ring.length >= 3
          ? ringToShape(envelope.ring, this._siteOrigin)
          : null;
      const floorGeom = floorShape
        ? new THREE.ShapeGeometry(floorShape)
        : new THREE.PlaneGeometry(envelope.halfW * 2, envelope.halfD * 2);
      floorGeom.rotateX(-Math.PI / 2);
      const floor = new THREE.Mesh(
        floorGeom,
        new THREE.MeshStandardMaterial({
          color: parseHexColor(slab.color ?? material.color, 0xd9c7a2),
          side: THREE.DoubleSide,
        }),
      );
      floor.position.set(envelope.center.x, elevation, envelope.center.z);
      results.push({ meshId: prim.id, mesh: floor, layerTags: [prim.id] });
      return results;
    }

    if (kind === "wall_ring") {
      const buildingId = resolveBuildingIdFromParent(prim.parent, this._worldPlan);
      const envelope = this.resolveBuildingFootprint(buildingId);
      if (!envelope) return results;
      const semantics = this._buildingSemantics.get(buildingId) || {};
      const profile = semantics.interiorProfile || {};
      const wallHeight = Number(
        prim.height ?? worldView.height ?? profile.wallHeight ?? 3.2,
      );
      const thickness = Number(
        prim.thickness ?? worldView.thickness ?? profile.wallThickness ?? 0.12,
      );
      const wallMaterial = new THREE.MeshStandardMaterial({
        color: parseHexColor(material.color, 0xf5f0e6),
        transparent: true,
        opacity: Number(material.opacity ?? 0.82),
        side: THREE.DoubleSide,
      });
      const { center, halfW, halfD } = envelope;
      const wallSpecs = [
        [center.x, wallHeight / 2, center.z - halfD, halfW * 2, wallHeight, thickness],
        [center.x, wallHeight / 2, center.z + halfD, halfW * 2, wallHeight, thickness],
        [center.x - halfW, wallHeight / 2, center.z, thickness, wallHeight, halfD * 2],
        [center.x + halfW, wallHeight / 2, center.z, thickness, wallHeight, halfD * 2],
      ];
      wallSpecs.forEach((spec, index) => {
        const [x, y, z, w, h, d] = spec;
        const wall = new THREE.Mesh(new THREE.BoxGeometry(w, h, d), wallMaterial.clone());
        wall.position.set(x, y, z);
        results.push({
          meshId: `${prim.id}_${index + 1}`,
          mesh: wall,
          layerTags: [prim.id],
        });
      });
      return results;
    }

    if (kind === "roof") {
      const buildingId = resolveBuildingIdFromParent(prim.parent, this._worldPlan);
      const envelope = this.resolveBuildingFootprint(buildingId);
      if (!envelope) return results;
      const semantics = this._buildingSemantics.get(buildingId) || {};
      const profile = semantics.interiorProfile || {};
      const slab = prim.slab || {};
      const slabMaterial = slab.material || material;
      const wallHeight = Number(profile.wallHeight ?? 3.2);
      const thickness = Number(slab.thickness ?? profile.roofThickness ?? 0.18);
      const elevation = Number(
        worldView.elevation ?? profile.roofElevation ?? wallHeight + thickness * 0.5,
      );
      const { center, halfW, halfD } = envelope;
      const roof = new THREE.Mesh(
        new THREE.BoxGeometry(halfW * 2, thickness, halfD * 2),
        new THREE.MeshStandardMaterial({
          color: parseHexColor(slabMaterial.color ?? material.color, 0x8b5e34),
          transparent: true,
          opacity: Number(slabMaterial.opacity ?? material.opacity ?? 0.88),
        }),
      );
      roof.position.set(center.x, elevation, center.z);
      results.push({
        meshId: prim.id,
        mesh: roof,
        layerTags: [prim.id],
        roofMesh: true,
      });
      return results;
    }

    if (kind === "prop") {
      const buildingId = resolveBuildingIdFromParent(prim.parent, this._worldPlan);
      const envelope = this.resolveBuildingFootprint(buildingId);
      const at = Array.isArray(prim.at) ? prim.at : [0, 1.4, 0];
      const center = envelope?.center || { x: 0, z: 0 };
      const screen = createWorldPropScreenMesh(prim.props || {});
      screen.position.set(
        center.x + Number(at[0] ?? 0),
        Number(at[1] ?? 1.4),
        center.z + Number(at[2] ?? 0),
      );
      results.push({
        meshId: prim.id,
        mesh: screen,
        layerTags: [prim.id],
        device: true,
      });
      return results;
    }

    return results;
  }

  resolveBuildingFootprint(buildingId) {
    const id = String(buildingId || "").trim();
    if (!id) return null;
    const feature = this._footprintsByEntity.get(id);
    const ring = feature?.geometry?.coordinates?.[0] || [];
    if (ring.length >= 3) {
      return footprintEnvelope(ring, this._siteOrigin);
    }
    const shell = this._meshes.get(`${id}:shell`) || this._meshes.get(id);
    if (!shell) return null;
    const box = new THREE.Box3().setFromObject(shell);
    const center = box.getCenter(new THREE.Vector3());
    const size = box.getSize(new THREE.Vector3());
    return {
      center: { x: center.x, z: center.z },
      halfW: Math.max(size.x / 2, 1.2),
      halfD: Math.max(size.z / 2, 1.2),
      ring: [],
    };
  }

  resolveInteriorAnchor(parentId) {
    const buildingId = resolveBuildingIdFromParent(parentId, this._worldPlan);
    const envelope = this.resolveBuildingFootprint(buildingId);
    if (!envelope) return null;
    return {
      center: new THREE.Vector3(envelope.center.x, 0, envelope.center.z),
      size: new THREE.Vector3(envelope.halfW * 2, 0, envelope.halfD * 2),
    };
  }

  initViewLayers(layers) {
    this._viewLayers = Array.isArray(layers) ? layers : [];
    this._viewLayerVisibility = new Map();
    for (const layer of this._viewLayers) {
      this._viewLayerVisibility.set(String(layer.id || ""), true);
    }
    this.renderViewLayerControl();
    this.applyViewLayerVisibility();
  }

  renderViewLayerControl() {
    const panel = this.shadowRoot?.querySelector('[data-role="layer-control"]');
    const list = this.shadowRoot?.querySelector('[data-role="layer-list"]');
    if (!panel || !list) return;
    if (!this._viewLayers.length) {
      panel.hidden = true;
      list.innerHTML = "";
      return;
    }
    panel.hidden = false;
    list.innerHTML = this._viewLayers
      .map((layer) => {
        const id = escapeHtml(String(layer.id || ""));
        const label = escapeHtml(String(layer.label || layer.id || ""));
        const checked = this._viewLayerVisibility.get(layer.id) !== false;
        return `<label><input type="checkbox" data-layer-id="${id}" ${checked ? "checked" : ""}/>${label}</label>`;
      })
      .join("");
    list.querySelectorAll("input[data-layer-id]").forEach((input) => {
      input.addEventListener("change", () => {
        const layerId = String(input.getAttribute("data-layer-id") || "");
        this._viewLayerVisibility.set(layerId, input.checked);
        this.applyViewLayerVisibility();
      });
    });
  }

  applyViewLayerPreset(layerIds) {
    const active = new Set((Array.isArray(layerIds) ? layerIds : []).map((id) => String(id)));
    for (const layer of this._viewLayers) {
      this._viewLayerVisibility.set(String(layer.id || ""), active.has(String(layer.id || "")));
    }
    this.renderViewLayerControl();
    this.applyViewLayerVisibility();
  }

  applyViewLayerVisibility() {
    if (!this._viewLayers.length) {
      for (const mesh of this._meshes.values()) {
        mesh.visible = true;
      }
      return;
    }
    for (const [meshId, mesh] of this._meshes.entries()) {
      const tags = mesh.userData.layerTags || [meshId];
      let visible = false;
      for (const layer of this._viewLayers) {
        if (this._viewLayerVisibility.get(layer.id) === false) continue;
        const members = Array.isArray(layer.members) ? layer.members : [];
        if (
          members.some(
            (member) =>
              tags.includes(member) ||
              meshId === member ||
              mesh.userData.entityId === member,
          )
        ) {
          visible = true;
          break;
        }
      }
      const interiorMode = this._viewLayerVisibility.get("floor_1") === true;
      if (
        visible &&
        interiorMode &&
        mesh.userData.shellMesh &&
        this._interiorBuildingIds.has(String(mesh.userData.entityId || ""))
      ) {
        visible = false;
      }
      mesh.visible = visible;
    }
  }

  applyCutawayState(cutaway) {
    const defaultLayers =
      cutaway?.default_layers ||
      cutaway?.defaultLayers ||
      (cutaway?.hideRoof === false
        ? ["site", "shell", "floor_1", "roof", "props"]
        : ["site", "shell", "floor_1", "props"]);
    this.applyViewLayerPreset(defaultLayers);
  }

  async fetchJson(url) {
    const response = await fetch(url, { credentials: "same-origin" });
    if (!response.ok) {
      throw new Error(`加载 ${url} 失败 (${response.status})`);
    }
    return response.json();
  }

  resolveEntityGroups(entityId) {
    const groups = [];
    const worldTargets = this.props?.worldTargets || {};
    const entities = worldTargets.entities || {};
    const entity = entities[entityId] || {};
    if (entity.groupId) groups.push(String(entity.groupId));
    for (const [groupId, group] of Object.entries(worldTargets.groups || {})) {
      const meshIds = group.meshIds || group.meshes || [];
      if (Array.isArray(meshIds) && meshIds.includes(entityId)) {
        groups.push(groupId);
      }
    }
    return groups;
  }

  resolveWorldTargetPreset(presetId) {
    const id = String(presetId || "").trim();
    if (!id) return null;
    const presets = this.props?.worldTargets?.cameraPresets || {};
    return presets[id] || null;
  }

  resolveWorldTargetEntity(entityId) {
    const id = String(entityId || "").trim();
    if (!id) return null;
    return this.props?.worldTargets?.entities?.[id] || null;
  }

  resolveWorldTargetGroup(groupId) {
    const id = String(groupId || "").trim();
    if (!id) return null;
    const group = this.props?.worldTargets?.groups?.[id];
    return group ? { id, ...group } : null;
  }

  setMeshGroupVisible(meshIds, visible) {
    const ids = Array.isArray(meshIds) ? meshIds : [];
    ids.forEach((meshId) => {
      const mesh = this._meshes.get(String(meshId));
      if (mesh) mesh.visible = visible;
    });
  }

  setGroupVisible(groupId, visible) {
    const id = String(groupId || "").trim();
    if (!id) return;
    if (visible) {
      this._hiddenGroups.delete(id);
    } else {
      this._hiddenGroups.add(id);
    }
    const entityIds = this._groups.get(id) || new Set();
    entityIds.forEach((entityId) => {
      const mesh = this._meshes.get(entityId);
      if (mesh) mesh.visible = visible;
    });
    const group = this.resolveWorldTargetGroup(id);
    if (group?.meshIds || group?.meshes) {
      this.setMeshGroupVisible(group.meshIds || group.meshes, visible);
    }
  }

  syncCameraNav(target, options = {}) {
    if (!this._controls || !this._camera) return;
    if (target) {
      this._controls.target.copy(target);
      this.clampControlsTarget();
    }
    this.syncControlsFromCamera(options.saveHome !== false);
  }

  focusEntity(entityId) {
    const id = String(entityId || "").trim();
    const mesh =
      this._meshes.get(id) ||
      this._meshes.get(`${id}:shell`) ||
      null;
    if (!mesh || !this._camera) return;
    const box = new THREE.Box3().setFromObject(mesh);
    const center = box.getCenter(new THREE.Vector3());
    const size = box.getSize(new THREE.Vector3());
    const distance = Math.max(size.x, size.z, size.y) * 2.4 + 8;
    this._camera.position.set(center.x + distance * 0.55, distance * 0.72, center.z + distance * 0.55);
    this._camera.lookAt(center);
    this.syncCameraNav(center);
    this.updateStatus(`focus ${entityId}`);
  }

  applyCameraPreset(preset) {
    if (!preset || !this._camera) return;
    const mode = String(preset.mode || "layout").trim();
    const targetEntity = String(preset.targetEntity || preset.target_entity || "").trim();
    const targetMesh =
      this._meshes.get(targetEntity) ||
      this._meshes.get(`${targetEntity}:shell`) ||
      this._meshes.get("lake_pavilion:shell") ||
      this._meshes.values().next().value;
    const box = targetMesh ? new THREE.Box3().setFromObject(targetMesh) : null;
    const center = box
      ? box.getCenter(new THREE.Vector3())
      : new THREE.Vector3(0, 0, 0);
    if (preset.cutaway || preset.default_layers || preset.defaultLayers) {
      this.applyCutawayState(preset);
    }
    if (mode === "inspect") {
      const eyeHeight = Number(preset.eyeHeight ?? preset.eye_height ?? 1.6);
      const fov = Number(preset.fov ?? 55);
      const targetMesh =
        this._meshes.get(targetEntity) ||
        this._meshes.get("info_screen_1") ||
        null;
      const lookAt = targetMesh
        ? targetMesh.position.clone()
        : new THREE.Vector3(
            Number((preset.lookAt || preset.look_at || [0, 1.4, 0])[0]),
            Number((preset.lookAt || preset.look_at || [0, 1.4, 0])[1]),
            Number((preset.lookAt || preset.look_at || [0, 1.4, 0])[2]),
          );
      this._camera.fov = fov;
      this._camera.updateProjectionMatrix();
      this._camera.position.set(lookAt.x - 1.5, eyeHeight, lookAt.z + 1.8);
      this._camera.lookAt(lookAt.x, lookAt.y, lookAt.z);
      this.syncCameraNav(lookAt);
      this.updateStatus(`inspect ${targetEntity || "device"}`);
      return;
    }
    const distance = Number(preset.distance ?? 36);
    const pitch = Number(preset.pitch ?? 68);
    const bearing = Number(preset.bearing ?? 24);
    const pitchRad = degToRad(pitch);
    const bearingRad = degToRad(bearing);
    const y = distance * Math.sin(pitchRad);
    const planar = distance * Math.cos(pitchRad);
    const x = center.x + planar * Math.sin(bearingRad);
    const z = center.z + planar * Math.cos(bearingRad);
    if (String(preset.projection || "").trim() === "orthographic") {
      this._camera.fov = 38;
      this._camera.updateProjectionMatrix();
    }
    this._camera.position.set(x, Math.max(y, 12), z);
    this._camera.lookAt(center.x, 0.8, center.z);
    this.syncCameraNav(new THREE.Vector3(center.x, 0.8, center.z));
    this.updateStatus(`layout ${targetEntity || "site"}`);
  }

  updateStatus(text) {
    const status = this.shadowRoot?.querySelector('[data-role="status"]');
    if (status) status.textContent = String(text || "world_view");
  }

  applyWorldTarget(target) {
    if (!target || typeof target !== "object") return false;
    this._pendingWorldTarget = target;
    if (!this._scene) return true;
    const entity = this.resolveWorldTargetEntity(target.entityId);
    const group = this.resolveWorldTargetGroup(target.groupId);
    const presetFromEntity = entity?.cameraPreset || entity?.camera_preset || "";
    const preset =
      this.resolveWorldTargetPreset(target.cameraPreset || presetFromEntity) || null;
    const resolved = {
      ...(group && typeof group === "object" ? group : {}),
      ...(entity && typeof entity === "object" ? entity : {}),
      ...(preset && typeof preset === "object" ? preset : {}),
      type: String(target.type || "").trim(),
      entityId: String(target.entityId || entity?.entityId || "").trim(),
      groupId: String(target.groupId || entity?.groupId || group?.id || "").trim(),
    };
    if (resolved.type === "show_group" || resolved.type === "showGroup") {
      if (resolved.groupId) this.setGroupVisible(resolved.groupId, true);
      this.setMeshGroupVisible(resolved.meshIds || resolved.meshes, true);
      return true;
    }
    if (resolved.type === "hide_group" || resolved.type === "hideGroup") {
      if (resolved.groupId) this.setGroupVisible(resolved.groupId, false);
      this.setMeshGroupVisible(resolved.meshIds || resolved.meshes, false);
      return true;
    }
    if (resolved.type === "cutaway_toggle" || resolved.type === "cutawayToggle") {
      const roofOn = this._viewLayerVisibility.get("roof") === true;
      this.applyViewLayerPreset(
        roofOn
          ? ["site", "floor_1", "props"]
          : ["site", "floor_1", "roof", "props"],
      );
      return true;
    }
    if (resolved.entityId) {
      this.focusEntity(resolved.entityId);
    }
    if (preset) {
      this.applyCameraPreset(preset);
    }
    if (resolved.groupId) {
      this.setGroupVisible(resolved.groupId, true);
    }
    return true;
  }
}

if (!customElements.get(TAG)) {
  customElements.define(TAG, MeiWorldStage);
}

function resolveActiveWorldStage() {
  if (boot.activeWorldStage?._controls) {
    return boot.activeWorldStage;
  }
  for (const instance of WORLD_RUNTIME_INSTANCES) {
    if (instance?._controls) {
      return instance;
    }
  }
  return null;
}

boot.worldStageCameraNav = {
  zoomIn() {
    resolveActiveWorldStage()?.navZoomBy(0.84);
  },
  zoomOut() {
    resolveActiveWorldStage()?.navZoomBy(1.19);
  },
  rotateLeft() {
    resolveActiveWorldStage()?.navRotateBearing(-15);
  },
  rotateRight() {
    resolveActiveWorldStage()?.navRotateBearing(15);
  },
  pitchUp() {
    resolveActiveWorldStage()?.navAdjustPitch(-6);
  },
  pitchDown() {
    resolveActiveWorldStage()?.navAdjustPitch(6);
  },
  reset() {
    resolveActiveWorldStage()?._controls?.reset();
  },
};
