import { escapeHtml, parseProps, resolveWorldRef } from "./shared.js";
import {
  ensureWorldStageInputPlane,
  layoutWorldStageInputPlane,
  resolveCockpitMapToolHost,
  setWorldStageInputPlaneActive,
} from "./cockpit-stage-overlay.js";
import { resolveCockpitStageSurface } from "./map-focus-inset.js";
import { createWorldPropScreenMesh } from "./world-prop-screen.js";
import { normalizeImportedFootprint } from "../gis/layer-spec.js";
import { buildLocalHeroFacadeOverlay } from "../map/maplibre/map-hero-facade.js";

let THREE = null;
let OrbitControls = null;

async function ensureThreeRuntime() {
  if (THREE && OrbitControls) {
    return { THREE, OrbitControls };
  }
  const [threeModule, controlsModule] = await Promise.all([
    import("../vendor/three/three.module.min.js"),
    import("../vendor/three/OrbitControls.js"),
  ]);
  THREE = threeModule;
  OrbitControls = controlsModule.OrbitControls;
  return { THREE, OrbitControls };
}

const TAG = "mei-world-stage";
const WORLD_RUNTIME_INSTANCES = new Set();
const boot = (window.__meiLangBoot = window.__meiLangBoot || {});

function isWorldStageActive() {
  return (
    typeof document !== "undefined" &&
    document.documentElement.classList.contains("mei-world-stage-active")
  );
}
const ORBIT_MIN_DISTANCE = 5;
const ORBIT_MAX_DISTANCE = 960;
const ORBIT_MIN_POLAR_DEG = 12;
const ORBIT_MAX_POLAR_DEG = 85;
const PICK_MOVE_THRESHOLD_PX = 6;
const FOOTPRINT_LAYER = {
  site_outline: { lift: 0.012, renderOrder: 10, opacity: 0.18, color: 0x93c5fd },
  flat_fill: { lift: 0.028, renderOrder: 20, opacity: 0.72, color: 0x2d7fb0 },
  default: { lift: 0.018, renderOrder: 15, opacity: 0.55, color: 0x1f4f74 },
  extrude_shell: { lift: 0.034, renderOrder: 30 },
};
const RESIZE_DEBOUNCE_MS = 80;

function resolveWorldQuality(props) {
  const raw = String(props?.quality || props?.renderQuality || "park")
    .trim()
    .toLowerCase();
  if (raw === "high") {
    return {
      id: "high",
      pixelRatioCap: 2,
      antialias: true,
      logarithmicDepthBuffer: true,
    };
  }
  if (raw === "low") {
    return {
      id: "low",
      pixelRatioCap: 1,
      antialias: false,
      logarithmicDepthBuffer: false,
    };
  }
  return {
    id: "park",
    pixelRatioCap: 1.5,
    antialias: true,
    // Tall towers (100m+) need log depth or shell/floor edges shimmer while orbiting.
    logarithmicDepthBuffer: true,
  };
}

function computeStructureSignature(props, el) {
  const worldRef = resolveWorldRef(props, el) || "park_world";
  const plan = resolveInjectedWorldPlan(worldRef) || props?.worldPlan || null;
  const primCount = Array.isArray(plan?.primitives) ? plan.primitives.length : 0;
  const planKey = String(plan?.id || plan?.worldId || plan?.name || worldRef);
  const quality = resolveWorldQuality(props).id;
  return `${worldRef}|${planKey}|${primCount}|${quality}`;
}

function isDocumentHidden() {
  return typeof document !== "undefined" && document.visibilityState === "hidden";
}

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

function resolveWorldFootprintGeojson(worldRef, plan) {
  const ref = String(worldRef || "park_world").trim();
  const candidates = [
    plan?.emittedFootprint,
    window.__mei?.map_projection?.worlds?.[ref]?.emittedFootprint,
    window.__mei?.world_plan?.worlds?.[ref]?.emittedFootprint,
  ];
  for (const item of candidates) {
    if (item && typeof item === "object" && Array.isArray(item.features)) {
      return normalizeImportedFootprint(item);
    }
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

function createFootprintSurfaceMaterial({ color, opacity, doubleSide = false }) {
  const clamped = Math.min(1, Math.max(Number(opacity) || 1, 0));
  const opaque = clamped >= 0.9;
  const transparent = !opaque && clamped < 0.999;
  return new THREE.MeshStandardMaterial({
    color,
    roughness: opaque ? 0.78 : 0.42,
    metalness: opaque ? 0.04 : 0.06,
    transparent,
    opacity: opaque ? 1 : clamped,
    side: doubleSide ? THREE.DoubleSide : THREE.FrontSide,
    depthWrite: opaque || clamped >= 0.55,
    depthTest: true,
    polygonOffset: transparent,
    polygonOffsetFactor: transparent ? 1 : 0,
    polygonOffsetUnits: transparent ? 1 : 0,
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
    // Shape lies in XY; after rotateX(-π/2), shapeY becomes -worldZ.
    // Store -local.z so the mesh lands on the same +Z as geoToLocal walls/roof.
    if (index === 0) {
      shape.moveTo(point.x, -point.z);
    } else {
      shape.lineTo(point.x, -point.z);
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

/** Vertical wall segments along footprint edges (absolute local meters). */
function wallSegmentsFromRing(ring, origin, thickness, insetMeters = 0.32) {
  if (!Array.isArray(ring) || ring.length < 3) return [];
  const points = [];
  for (const coord of ring) {
    if (!Array.isArray(coord) || coord.length < 2) continue;
    const local = geoToLocal(coord[0], coord[1], origin);
    points.push({ x: local.x, z: local.z });
  }
  if (points.length < 3) return [];
  const first = points[0];
  const last = points[points.length - 1];
  if (first.x !== last.x || first.z !== last.z) {
    points.push({ x: first.x, z: first.z });
  }
  let cx = 0;
  let cz = 0;
  const n = points.length - 1;
  for (let i = 0; i < n; i += 1) {
    cx += points[i].x;
    cz += points[i].z;
  }
  cx /= Math.max(n, 1);
  cz /= Math.max(n, 1);
  const inset = Math.max(Number(insetMeters) || 0, 0);
  const segments = [];
  for (let i = 0; i < points.length - 1; i += 1) {
    const a = points[i];
    const b = points[i + 1];
    const dx = b.x - a.x;
    const dz = b.z - a.z;
    const length = Math.hypot(dx, dz);
    if (length < 0.4) continue;
    let mx = (a.x + b.x) * 0.5;
    let mz = (a.z + b.z) * 0.5;
    // Pull segment inward so it does not coplanar-fight the extruded shell.
    const ix = cx - mx;
    const iz = cz - mz;
    const ilen = Math.hypot(ix, iz) || 1;
    mx += (ix / ilen) * inset;
    mz += (iz / ilen) * inset;
    segments.push({
      x: mx,
      z: mz,
      length: Math.max(length - inset * 0.15, 0.5),
      yaw: Math.atan2(dx, dz),
      thickness,
    });
  }
  return segments;
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

function resolveWorldStageRegistry(plan) {
  const rows = Array.isArray(plan?.worldStageEntities) ? plan.worldStageEntities : [];
  const byEntity = new Map();
  for (const row of rows) {
    const entityId = String(row?.entityId || row?.entity_id || "").trim();
    if (!entityId) continue;
    const members = Array.isArray(row?.members)
      ? row.members.map((member) => String(member || "").trim()).filter(Boolean)
      : [];
    byEntity.set(entityId, { entityId, members, worldEnterable: row?.worldEnterable === true });
  }
  return byEntity;
}

function resolveStageBuildingId(prim, plan) {
  const kind = String(prim?.kind || "").trim();
  if (kind === "building") {
    return String(prim.featureEntityId || prim.id || "").trim();
  }
  return resolveBuildingIdFromParent(prim?.parent, plan);
}

function shouldRenderWorldPrimitive(prim, plan, stageRegistry) {
  const kind = String(prim?.kind || "").trim();
  if (prim?.mapOnly === true || kind === "building_import") {
    return false;
  }
  if (!stageRegistry || stageRegistry.size === 0) {
    return kind !== "building_import";
  }
  if (kind === "ground" || kind === "pool" || kind === "green" || kind === "route" || kind === "road") {
    return true;
  }
  const buildingId = resolveStageBuildingId(prim, plan);
  return Boolean(buildingId && stageRegistry.has(buildingId));
}

function meshBelongsToStageEntity(mesh, entityId, stageRegistry) {
  const id = String(entityId || "").trim();
  if (!id) return true;
  const entry = stageRegistry?.get(id);
  const members = new Set(entry?.members || []);
  const tags = mesh?.userData?.layerTags || [];
  const meshEntity = String(mesh?.userData?.entityId || "");
  if (meshEntity === id) return true;
  if (members.has(meshEntity)) return true;
  return tags.some((tag) => members.has(String(tag)) || String(tag) === id || String(tag) === `${id}:shell`);
}

function readEntityId(feature) {
  return String(
    feature?.properties?.entityId ||
      feature?.properties?.entity_id ||
      feature?.id ||
      "",
  ).trim();
}

function featureMatchesProperties(feature, matcher) {
  if (!matcher || typeof matcher !== "object" || Array.isArray(matcher)) {
    return true;
  }
  const props = feature?.properties && typeof feature.properties === "object" ? feature.properties : {};
  return Object.entries(matcher).every(([key, expected]) => {
    const actual = props[key];
    if (Array.isArray(expected)) {
      return expected.some((value) => String(actual ?? "").trim() === String(value ?? "").trim());
    }
    return String(actual ?? "").trim() === String(expected ?? "").trim();
  });
}

function polygonRingsFromFeature(feature) {
  const geom = feature?.geometry;
  if (!geom) return [];
  if (geom.type === "Polygon") {
    const ring = geom.coordinates?.[0] || [];
    return ring.length >= 3 ? [ring] : [];
  }
  if (geom.type === "MultiPolygon") {
    return (geom.coordinates || [])
      .map((poly) => poly?.[0] || [])
      .filter((ring) => ring.length >= 3);
  }
  return [];
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
    this._structureSignature = "";
    this._quality = resolveWorldQuality({});
    this._animationFrame = 0;
    this._renderingActive = false;
    this._needsRender = false;
    this._softSuspended = false;
    this._renderCount = 0;
    this._lastFrameMs = 0;
    this._bootstrapPromise = null;
    this._siteOrigin = { lng: 106.38224, lat: 29.62396 };
    this._resizeObserver = null;
    this._resizeDebounceTimer = 0;
    this._inputSurface = null;
    this._controlsDom = null;
    this._orbitAnchor = null;
    this._orbitPanRadius = 16;
    this._orbitPanVertical = 20;
    this._onWorldStageEntered = null;
    this._onWorldStageExited = null;
    this._onViewportStageLayout = null;
    this._onVisibilityChange = null;
  }

  connectedCallback() {
    WORLD_RUNTIME_INSTANCES.add(this);
    boot.activeWorldStage = this;
    this.props = parseProps(this);
    this._propsSignature = String(this.getAttribute("data-props") || "");
    this._quality = resolveWorldQuality(this.props);
    if (!this.shadowRoot) {
      this.attachShadow({ mode: "open" });
    }
    this.renderChrome();
    if (!this._onWorldStageEntered) {
      this._onWorldStageEntered = () => {
        this._softSuspended = false;
        if (this._controls) {
          this._controls.enabled = true;
        }
        void this.ensureSceneBootstrapped().then(() => {
          this.activateInteractionSurface();
          this.resumeRendering();
        });
      };
      window.addEventListener("mei:world-stage-entered", this._onWorldStageEntered);
    }
    if (!this._onWorldStageExited) {
      this._onWorldStageExited = (event) => {
        this.deactivateInteractionSurface();
        this._pendingWorldTarget = null;
        const reason = String(event?.detail?.reason || "").trim();
        if (reason === "stage-switch" || reason === "full-dispose") {
          this.fullDispose();
        } else {
          this.softSuspend();
        }
      };
      window.addEventListener("mei:world-stage-exited", this._onWorldStageExited);
    }
    if (!this._onViewportStageLayout) {
      this._onViewportStageLayout = () => this.syncInteractionSurfaceLayout();
      window.addEventListener("meilang:viewport-stage-layout", this._onViewportStageLayout);
      window.addEventListener("resize", this._onViewportStageLayout, { passive: true });
    }
    if (!this._onVisibilityChange) {
      this._onVisibilityChange = () => {
        if (isDocumentHidden()) {
          this.pauseRendering();
          return;
        }
        if (isWorldStageActive() && !this._softSuspended && this._renderer) {
          this.invalidate();
        }
      };
      document.addEventListener("visibilitychange", this._onVisibilityChange);
    }
    if (isWorldStageActive()) {
      this._softSuspended = false;
      void this.ensureSceneBootstrapped().then(() => {
        this.activateInteractionSurface();
        this.resumeRendering();
      });
    } else if (this._renderer) {
      this.pauseRendering();
    }
    if (!this._onPreviewUpdated) {
      this._previewUpdatedTimer = 0;
      this._onPreviewUpdated = () => {
        if (!isWorldStageActive() && !this._renderer) {
          return;
        }
        if (this._previewUpdatedTimer) {
          clearTimeout(this._previewUpdatedTimer);
        }
        this._previewUpdatedTimer = setTimeout(() => {
          this._previewUpdatedTimer = 0;
          this.refreshFromProps();
        }, 120);
      };
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
    if (this._onVisibilityChange) {
      document.removeEventListener("visibilitychange", this._onVisibilityChange);
      this._onVisibilityChange = null;
    }
    this.deactivateInteractionSurface();
    this.fullDispose();
    if (this._onPreviewUpdated) {
      window.removeEventListener("meilang:preview-updated", this._onPreviewUpdated);
      this._onPreviewUpdated = null;
    }
    if (this._previewUpdatedTimer) {
      clearTimeout(this._previewUpdatedTimer);
      this._previewUpdatedTimer = 0;
    }
  }

  refreshFromProps(options = {}) {
    if (!isWorldStageActive() && !this._renderer) {
      return;
    }
    this.props = parseProps(this);
    const nextSignature = String(this.getAttribute("data-props") || "");
    const propsChanged = nextSignature !== this._propsSignature;
    this._propsSignature = nextSignature;
    const nextStructure = computeStructureSignature(this.props, this);
    const structureChanged = nextStructure !== this._structureSignature;
    this._quality = resolveWorldQuality(this.props);
    if (!this.shadowRoot) {
      this.attachShadow({ mode: "open" });
      this.renderChrome();
    }
    if (!propsChanged && options.forceRender !== true) {
      if (this._pendingWorldTarget && this._renderer) {
        this.applyWorldTarget(this._pendingWorldTarget);
      }
      return;
    }
    if (!this._renderer) {
      if (isWorldStageActive()) {
        void this.ensureSceneBootstrapped();
      }
      return;
    }
    if (!structureChanged && options.forceBootstrap !== true) {
      this.applyPixelRatioCap();
      if (this._pendingWorldTarget) {
        this.applyWorldTarget(this._pendingWorldTarget);
      }
      this.invalidate();
      return;
    }
    this._structureSignature = nextStructure;
    void this.bootstrapScene();
  }

  ensureSceneBootstrapped() {
    if (this._renderer) {
      return Promise.resolve();
    }
    if (this._bootstrapPromise) {
      return this._bootstrapPromise;
    }
    this._bootstrapPromise = this.bootstrapScene().finally(() => {
      this._bootstrapPromise = null;
    });
    return this._bootstrapPromise;
  }

  canRenderFrame() {
    return (
      Boolean(this._renderer) &&
      isWorldStageActive() &&
      !this._softSuspended &&
      !isDocumentHidden()
    );
  }

  invalidate() {
    this._needsRender = true;
    if (!this._renderingActive) {
      this.kickRenderLoop();
    }
  }

  kickRenderLoop() {
    if (!this.canRenderFrame()) {
      return;
    }
    if (this._animationFrame) {
      return;
    }
    this._renderingActive = true;
    this._animationFrame = requestAnimationFrame(() => this.animate());
  }

  pauseRendering() {
    this._renderingActive = false;
    this._needsRender = false;
    if (this._animationFrame) {
      cancelAnimationFrame(this._animationFrame);
      this._animationFrame = 0;
    }
  }

  resumeRendering() {
    if (!this.canRenderFrame()) {
      return;
    }
    this.invalidate();
  }

  softSuspend() {
    this._softSuspended = true;
    this.pauseRendering();
    if (this._controls) {
      this._controls.enabled = false;
    }
    window.__meiBrowserRuntimeDiag?.record?.("world_scene_soft_suspended", {
      renderCount: this._renderCount,
      hadRenderer: Boolean(this._renderer),
    });
  }

  publishRenderDiag() {
    const info = this._renderer?.info;
    window.__meiBrowserRuntimeDiag?.recordWorldRender?.({
      renderCount: this._renderCount,
      lastFrameMs: this._lastFrameMs,
      softSuspended: this._softSuspended,
      renderingActive: this._renderingActive,
      quality: this._quality?.id || "park",
      rendererInfo: info
        ? {
            geometries: info.memory?.geometries ?? 0,
            textures: info.memory?.textures ?? 0,
            calls: info.render?.calls ?? 0,
            triangles: info.render?.triangles ?? 0,
            points: info.render?.points ?? 0,
            lines: info.render?.lines ?? 0,
          }
        : null,
    });
  }

  getPerfSnapshot() {
    const info = this._renderer?.info;
    return {
      renderCount: this._renderCount,
      lastFrameMs: this._lastFrameMs,
      softSuspended: this._softSuspended,
      renderingActive: this._renderingActive,
      needsRender: this._needsRender,
      quality: this._quality?.id || "park",
      hasRenderer: Boolean(this._renderer),
      pixelRatio: this._renderer?.getPixelRatio?.() ?? null,
      rendererInfo: info
        ? {
            geometries: info.memory?.geometries ?? 0,
            textures: info.memory?.textures ?? 0,
            calls: info.render?.calls ?? 0,
            triangles: info.render?.triangles ?? 0,
          }
        : null,
    };
  }

  applyPixelRatioCap() {
    if (!this._renderer || !this._quality) return;
    const next = Math.min(window.devicePixelRatio || 1, this._quality.pixelRatioCap);
    if (Math.abs((this._renderer.getPixelRatio?.() || 0) - next) > 0.001) {
      this._renderer.setPixelRatio(next);
      this.invalidate();
    }
  }

  disposeScene() {
    this.fullDispose();
  }

  fullDispose() {
    this._softSuspended = false;
    this.pauseRendering();
    if (this._resizeDebounceTimer) {
      clearTimeout(this._resizeDebounceTimer);
      this._resizeDebounceTimer = 0;
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
    this._orbitAnchor = null;
    if (this._scene) {
      this._scene.traverse((obj) => {
        if (obj.geometry) {
          obj.geometry.dispose();
        }
        const material = obj.material;
        if (!material) return;
        const materials = Array.isArray(material) ? material : [material];
        for (const mat of materials) {
          if (!mat) continue;
          for (const key of Object.keys(mat)) {
            const value = mat[key];
            if (value && typeof value.dispose === "function" && value.isTexture) {
              value.dispose();
            }
          }
          mat.dispose();
        }
      });
    }
    this._scene = null;
    this._camera = null;
    this._meshes.clear();
    this._groups.clear();
    this._structureSignature = "";
    if (this._renderer) {
      const canvas = this._renderer.domElement;
      const loseContext = this._renderer.forceContextLoss?.bind(this._renderer);
      this._renderer.dispose();
      if (canvas && typeof canvas.remove === "function") {
        canvas.remove();
      }
      if (typeof loseContext === "function") {
        loseContext();
      }
      this._renderer = null;
      window.__meiBrowserRuntimeDiag?.record?.("world_scene_disposed", {
        hadRenderer: true,
        mode: "full",
      });
    } else {
      window.__meiBrowserRuntimeDiag?.record?.("world_scene_disposed", {
        hadRenderer: false,
        mode: "full",
      });
    }
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
    if (!isWorldStageActive()) {
      return;
    }
    this.renderChrome();
    const errorEl = this.shadowRoot?.querySelector('[data-role="error"]');
    const viewport = this.shadowRoot?.querySelector(".viewport");
    if (!viewport) return;
    try {
      await ensureThreeRuntime();
      this.fullDispose();
      const worldRef = resolveWorldRef(this.props, this) || "park_world";
      this._worldPlan = resolveInjectedWorldPlan(worldRef) || this.props?.worldPlan || null;
      this._quality = resolveWorldQuality(this.props);
      this._structureSignature = computeStructureSignature(this.props, this);
      const site = this._worldPlan?.site || this.props?.worldSpec?.site || {};
      const origin = site.origin || site;
      this._siteOrigin = {
        lng: Number(origin.lng ?? site.originLng ?? site.origin_lng ?? 106.38224),
        lat: Number(origin.lat ?? site.originLat ?? site.origin_lat ?? 29.62396),
      };
      this._scene = new THREE.Scene();
      this._scene.background = new THREE.Color(0x071526);
      this._scene.fog = new THREE.Fog(0x071526, 160, 1800);
      const width = Math.max(320, this.clientWidth || viewport.clientWidth || 320);
      const height = Math.max(240, this.clientHeight || viewport.clientHeight || 240);
      this._camera = new THREE.PerspectiveCamera(52, width / height, 0.1, 2400);
      this._camera.position.set(26, 34, 38);
      this._renderer = new THREE.WebGLRenderer({
        antialias: this._quality.antialias,
        alpha: false,
        logarithmicDepthBuffer: this._quality.logarithmicDepthBuffer,
      });
      this._renderer.setPixelRatio(
        Math.min(window.devicePixelRatio || 1, this._quality.pixelRatioCap),
      );
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
        new THREE.PlaneGeometry(1600, 1600),
        new THREE.MeshStandardMaterial({ color: 0x12324f, roughness: 0.92, metalness: 0.04 }),
      );
      ground.rotation.x = -Math.PI / 2;
      ground.position.y = -0.045;
      ground.renderOrder = 0;
      ground.receiveShadow = true;
      this._scene.add(ground);
      await this.loadWorldContent();
      this.bindResize(viewport);
      this._softSuspended = false;
      if (isWorldStageActive()) {
        this.resumeRendering();
      }
      if (errorEl) {
        errorEl.hidden = true;
        errorEl.textContent = "";
      }
      if (this._pendingWorldTarget) {
        this.applyWorldTarget(this._pendingWorldTarget);
      }
      if (isWorldStageActive()) {
        this.activateInteractionSurface();
      }
      window.__meiBrowserRuntimeDiag?.record?.("world_scene_bootstrapped", {
        worldRef: resolveWorldRef(this.props, this) || "park_world",
        meshCount: this._meshes.size,
        featureCount: this._footprintsByEntity?.size ?? 0,
        quality: this._quality.id,
        pixelRatio: this._renderer.getPixelRatio(),
      });
      this.publishRenderDiag();
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
    if (!domElement.dataset.meiOrbitCtxBound) {
      domElement.dataset.meiOrbitCtxBound = "1";
      domElement.addEventListener("contextmenu", (event) => {
        event.preventDefault();
      });
    }
    this.bindStableGestureGuards(domElement);
  }

  bindStableGestureGuards(domElement) {
    if (!domElement || domElement.dataset.meiStableGesturesBound) return;
    domElement.dataset.meiStableGesturesBound = "1";
    // OrbitControls swaps ROTATE/PAN when Ctrl/Meta/Shift is held. That makes
    // one drag unexpectedly change modes. World-stage keeps fixed buttons:
    // left=pan, right=rotate; modifier drags are ignored.
    domElement.addEventListener(
      "pointerdown",
      (event) => {
        if (!(event.ctrlKey || event.metaKey || event.shiftKey)) return;
        event.preventDefault();
        event.stopImmediatePropagation();
      },
      true,
    );
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
    controls.enableRotate = true;
    controls.enablePan = true;
    controls.enableZoom = true;
    controls.zoomSpeed = 1.15;
    controls.rotateSpeed = 0.92;
    controls.panSpeed = 0.95;
    // Match MapLibre city navigation: left-drag pan, right / Ctrl+left rotate+pitch.
    const mouse = THREE.MOUSE || {};
    const touch = THREE.TOUCH || {};
    if (mouse.PAN != null && mouse.ROTATE != null) {
      controls.mouseButtons = {
        LEFT: mouse.PAN,
        MIDDLE: mouse.DOLLY ?? mouse.PAN,
        RIGHT: mouse.ROTATE,
      };
    }
    if (touch.PAN != null) {
      controls.touches = {
        ONE: touch.PAN,
        TWO: touch.DOLLY_ROTATE ?? touch.DOLLY_PAN,
      };
    }
    // Right-drag rotate needs context menu suppressed on the input surface.
    if (domElement && !domElement.dataset.meiOrbitCtxBound) {
      domElement.dataset.meiOrbitCtxBound = "1";
      domElement.addEventListener("contextmenu", (event) => {
        event.preventDefault();
      });
    }
    this.bindStableGestureGuards(domElement);
    controls.addEventListener("change", () => {
      this.clampControlsTarget();
      this.invalidate();
    });
    return controls;
  }

  /**
   * Pan bounds scale with focused building / scene size so tall towers (200m+)
   * are not stuck in the old park-scale clamp (±48m / y≤16).
   */
  clampControlsTarget() {
    if (!this._controls || !this._camera) return;
    const bounds = this.resolveOrbitPanBounds();
    const next = this._controls.target.clone();
    next.x = THREE.MathUtils.clamp(
      this._controls.target.x,
      bounds.minX,
      bounds.maxX,
    );
    next.z = THREE.MathUtils.clamp(
      this._controls.target.z,
      bounds.minZ,
      bounds.maxZ,
    );
    next.y = THREE.MathUtils.clamp(
      this._controls.target.y,
      bounds.minY,
      bounds.maxY,
    );
    const correction = next.sub(this._controls.target);
    if (correction.lengthSq() < 1e-10) return;
    // OrbitControls pans camera and target together. Clamping only target breaks
    // that invariant and repeated drags eventually throw the model off-screen.
    this._controls.target.add(correction);
    this._camera.position.add(correction);
  }

  resolveOrbitPanBounds() {
    const fallback = { minX: -80, maxX: 80, minZ: -80, maxZ: 80, minY: 0, maxY: 40 };
    if (this._orbitAnchor) {
      const anchor = this._orbitAnchor;
      const radius = Math.max(Number(this._orbitPanRadius) || 0, 2);
      const vertical = Math.max(Number(this._orbitPanVertical) || 0, 4);
      return {
        minX: anchor.x - radius,
        maxX: anchor.x + radius,
        minZ: anchor.z - radius,
        maxZ: anchor.z + radius,
        minY: Math.max(0, anchor.y - vertical),
        maxY: anchor.y + vertical,
      };
    }
    if (!this._scene || !this._meshes?.size) return fallback;
    try {
      const box = new THREE.Box3();
      let has = false;
      for (const mesh of this._meshes.values()) {
        if (!mesh?.visible) continue;
        box.expandByObject(mesh);
        has = true;
      }
      if (!has || box.isEmpty()) return fallback;
      const size = box.getSize(new THREE.Vector3());
      const pad = Math.max(size.x, size.z, size.y) * 0.65 + 40;
      const center = box.getCenter(new THREE.Vector3());
      return {
        minX: center.x - pad,
        maxX: center.x + pad,
        minZ: center.z - pad,
        maxZ: center.z + pad,
        minY: 0,
        maxY: Math.max(center.y + size.y * 0.85, 24),
      };
    } catch {
      return fallback;
    }
  }

  syncControlsFromCamera(saveHome = true) {
    if (!this._controls || !this._camera) return;
    this._controls.update();
    if (saveHome) {
      this._controls.saveState();
    }
  }

  updateOrbitDistanceBounds(box) {
    if (!this._controls || !box || box.isEmpty()) {
      return ORBIT_MIN_DISTANCE;
    }
    const sphere = box.getBoundingSphere(new THREE.Sphere());
    const size = box.getSize(new THREE.Vector3());
    const radius = Math.max(Number(sphere.radius) || 1, 1);
    const minDistance = THREE.MathUtils.clamp(
      radius * 1.08,
      ORBIT_MIN_DISTANCE,
      ORBIT_MAX_DISTANCE * 0.48,
    );
    this._controls.minDistance = minDistance;
    this._controls.maxDistance = Math.max(ORBIT_MAX_DISTANCE, radius * 10);
    this._orbitAnchor = sphere.center.clone();
    // Isolated building inspection only needs modest framing adjustment.
    // Keeping this tight prevents repeated left-pans from losing the model.
    this._orbitPanRadius = THREE.MathUtils.clamp(
      Math.max(size.x, size.z) * 0.28,
      3,
      24,
    );
    this._orbitPanVertical = THREE.MathUtils.clamp(size.y * 0.12, 8, 28);
    return minDistance;
  }

  distanceToFitBox(box, margin = 1.16) {
    if (!this._camera || !box || box.isEmpty()) return ORBIT_MIN_DISTANCE;
    const sphere = box.getBoundingSphere(new THREE.Sphere());
    const radius = Math.max(Number(sphere.radius) || 1, 1);
    const verticalFov = degToRad(
      THREE.MathUtils.clamp(Number(this._camera.fov) || 50, 10, 120),
    );
    const aspect = Math.max(Number(this._camera.aspect) || 1, 0.1);
    const horizontalFov = 2 * Math.atan(Math.tan(verticalFov / 2) * aspect);
    const limitingFov = Math.max(Math.min(verticalFov, horizontalFov), 0.1);
    return THREE.MathUtils.clamp(
      (radius / Math.sin(limitingFov / 2)) * margin,
      ORBIT_MIN_DISTANCE,
      ORBIT_MAX_DISTANCE * 0.92,
    );
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
    this.invalidate();
  }

  navRotateBearing(deg) {
    if (!this._controls || !this._camera) return;
    applySphericalOffset(this._camera, this._controls.target, (spherical) => {
      spherical.theta -= degToRad(deg);
    });
    this._controls.update();
    this.invalidate();
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
    this.invalidate();
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
      if (event.button !== 0) {
        clearPointerDown();
        return;
      }
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
      if (event.button !== 0) {
        clearPointerDown();
        return;
      }
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
    window.dispatchEvent(
      new CustomEvent("mei:world-entity-pick", {
        detail: {
          entityId,
          meshId: mesh?.name || entityId,
          device: Boolean(mesh?.userData?.device),
        },
      }),
    );
    // Picking is selection-only. Reframing here made a stationary left click
    // unexpectedly replace the current orbit camera, while right-click belongs
    // exclusively to rotation. Explicit buttons/actions still call focusEntity.
    this.updateStatus(`selected ${entityId}`);
    this.invalidate();
  }

  bindResize(viewport) {
    if (typeof ResizeObserver === "undefined" || !this._renderer || !this._camera) {
      return;
    }
    this._resizeObserver = new ResizeObserver(() => {
      if (this._resizeDebounceTimer) {
        clearTimeout(this._resizeDebounceTimer);
      }
      this._resizeDebounceTimer = setTimeout(() => {
        this._resizeDebounceTimer = 0;
        if (!this._renderer || !this._camera) return;
        const width = Math.max(320, viewport.clientWidth || 320);
        const height = Math.max(240, viewport.clientHeight || 240);
        this._renderer.setSize(width, height, false);
        this._camera.aspect = width / height;
        this._camera.updateProjectionMatrix();
        this.invalidate();
      }, RESIZE_DEBOUNCE_MS);
    });
    this._resizeObserver.observe(viewport);
  }

  animate() {
    this._animationFrame = 0;
    if (!this._renderer || !this._scene || !this._camera) {
      this._renderingActive = false;
      return;
    }
    if (!this.canRenderFrame()) {
      this._renderingActive = false;
      return;
    }

    const startedAt = performance.now();
    let controlsMoving = false;
    if (this._controls) {
      controlsMoving = this._controls.update() === true;
    }

    const shouldDraw = this._needsRender || controlsMoving;
    if (shouldDraw) {
      this._renderer.render(this._scene, this._camera);
      this._renderCount += 1;
      this._lastFrameMs = performance.now() - startedAt;
      this._needsRender = false;
      this.publishRenderDiag();
    }

    if (controlsMoving || this._needsRender) {
      this._renderingActive = true;
      this._animationFrame = requestAnimationFrame(() => this.animate());
      return;
    }
    this._renderingActive = false;
  }

  async loadWorldContent() {
    const worldRef = resolveWorldRef(this.props, this) || "park_world";
    const plan = this._worldPlan || resolveInjectedWorldPlan(worldRef);
    const worldSpec = this.props?.worldSpec || {};
    const inlineFootprint = resolveWorldFootprintGeojson(worldRef, plan);
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
    const geojson = normalizeImportedFootprint(
      inlineFootprint || (await this.fetchJson(geoUrl)),
    );
    if (!plan) {
      throw new Error(`world_plan 未注入：${worldRef}`);
    }
    this.buildFromWorldPlan(plan, geojson);
  }

  buildFromWorldPlan(plan, geojson) {
    this._worldPlan = plan;
    this._stageRegistry = resolveWorldStageRegistry(plan);
    this._activeStageEntity = "";
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
      if (!shouldRenderWorldPrimitive(prim, plan, this._stageRegistry)) {
        continue;
      }
      if (kind === "floor" || kind === "wall_ring" || kind === "roof" || kind === "prop") {
        deferred.push(prim);
        continue;
      }
      this.mountPrimitiveMeshes(prim, featuresByEntity);
    }
    for (const prim of deferred) {
      if (!shouldRenderWorldPrimitive(prim, plan, this._stageRegistry)) {
        continue;
      }
      this.mountPrimitiveMeshes(prim, featuresByEntity);
    }
    this.initViewLayers(plan.viewLayers || []);
    if (this._activeStageEntity) {
      this.applyStageEntityVisibility(this._activeStageEntity);
    }
  }

  applyStageEntityVisibility(entityId) {
    const id = String(entityId || "").trim();
    this._activeStageEntity = id;
    if (!id || !this._stageRegistry?.size) {
      return;
    }
    for (const mesh of this._meshes.values()) {
      mesh.visible = meshBelongsToStageEntity(mesh, id, this._stageRegistry);
    }
    this.applyViewLayerVisibility();
  }

  mountPrimitiveMeshes(prim, featuresByEntity) {
    const built = this.buildPrimitiveMeshes(prim, featuresByEntity);
    for (const entry of built) {
      const mesh = entry.mesh;
      mesh.name = entry.meshId;
      mesh.userData.entityId = String(entry.entityId || prim.id || entry.meshId);
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

    if (kind === "building_import") {
      // 批量导入仅用于 map_projection（MapLibre fill-extrusion），不在 Three.js 挤出。
      return results;
    }

    if (kind === "building" && (feature?.geometry?.type === "Polygon" || feature?.geometry?.type === "MultiPolygon")) {
      const rings = polygonRingsFromFeature(feature);
      if (!rings.length) return results;
      const ring = rings[0];
      const buildingId = String(prim.id || "");
      const heroLike = prim.worldEnterable === true || Boolean(prim.mapHero);
      const envelope = footprintEnvelope(ring, this._siteOrigin);
      const semantics = this._buildingSemantics.get(buildingId) || {};
      const worldView = prim.worldView || semantics.worldView || {};
      const height = Number(
        prim.height ??
          feature?.properties?.height ??
          worldView.shellHeight ??
          semantics.height ??
          8.6,
      );
      const layer = footprintLayerFor("extrude_shell");
      const shape = ringToShape(ring, this._siteOrigin);
      const geom = new THREE.ExtrudeGeometry(shape, {
        depth: height,
        bevelEnabled: false,
      });
      geom.rotateX(-Math.PI / 2);
      const shellMaterial = prim.shellMaterial || semantics.shellMaterial || material;
      // Hero towers need an opaque mass (MapLibre L4 look). Author glass opacity
      // is for cutaway/interior layers — not for the default exterior shell.
      const rawShellOpacity = Number(shellMaterial.opacity ?? 0.9);
      const shellOpacity = heroLike ? 1 : rawShellOpacity;
      const mesh = new THREE.Mesh(
        geom,
        createFootprintSurfaceMaterial({
          color: parseHexColor(shellMaterial.color, 0xffd36b),
          opacity: shellOpacity,
          doubleSide: true,
        }),
      );
      mesh.renderOrder = 5;
      applyFootprintLayer(mesh, {
        ...layer,
        lift: Number(worldView.lift ?? layer.lift),
      });
      results.push({
        meshId: `${prim.id}:shell`,
        mesh,
        entityId: String(prim.featureEntityId || prim.id || ""),
        layerTags: [`${prim.id}:shell`, prim.id],
        shellMesh: true,
      });
      if (heroLike) {
        results.push(...this.buildHeroExteriorMeshes(prim, ring, envelope, height));
      }
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
      const buildingPrim = (this._worldPlan?.primitives || []).find(
        (item) => String(item?.id || "") === buildingId,
      );
      // Overview uses opaque shell; floor slabs are for cutaway/interior layers.
      // Skip in default hero overview so the tower reads as a solid mass.
      if (buildingPrim?.worldEnterable === true || Boolean(buildingPrim?.mapHero)) {
        return results;
      }
      const semantics = this._buildingSemantics.get(buildingId) || {};
      const profile = semantics.interiorProfile || {};
      const elevation = Number(
        prim.elevation ?? worldView.elevation ?? profile.floorElevation ?? 0.05,
      );
      const slab = prim.slab || material;
      const floorThickness = 0.22;
      const floorOpacity = Number(slab.opacity ?? material.opacity ?? 0.96);
      let floor;
      if (envelope.ring.length >= 3) {
        // Thick extruded slab (not a zero-thickness plane) so floors read clearly
        // through glass wall bands instead of looking like a hollow cage.
        const shape = ringToShape(envelope.ring, this._siteOrigin);
        const floorGeom = new THREE.ExtrudeGeometry(shape, {
          depth: floorThickness,
          bevelEnabled: false,
        });
        floorGeom.rotateX(-Math.PI / 2);
        floor = new THREE.Mesh(
          floorGeom,
          new THREE.MeshStandardMaterial({
            color: parseHexColor(slab.color ?? material.color, 0xd9c7a2),
            transparent: floorOpacity < 0.98,
            opacity: floorOpacity < 0.98 ? floorOpacity : 1,
            side: THREE.DoubleSide,
            depthWrite: true,
            depthTest: true,
            roughness: 0.85,
            metalness: 0.02,
          }),
        );
        floor.position.set(0, elevation, 0);
      } else {
        floor = new THREE.Mesh(
          new THREE.BoxGeometry(envelope.halfW * 2, floorThickness, envelope.halfD * 2),
          new THREE.MeshStandardMaterial({
            color: parseHexColor(slab.color ?? material.color, 0xd9c7a2),
            side: THREE.DoubleSide,
            depthWrite: true,
          }),
        );
        floor.position.set(envelope.center.x, elevation + floorThickness * 0.5, envelope.center.z);
      }
      floor.renderOrder = 8;
      results.push({ meshId: prim.id, mesh: floor, layerTags: [prim.id] });
      return results;
    }

    if (kind === "wall_ring") {
      const buildingId = resolveBuildingIdFromParent(prim.parent, this._worldPlan);
      const envelope = this.resolveBuildingFootprint(buildingId);
      if (!envelope) return results;
      const buildingPrim = (this._worldPlan?.primitives || []).find(
        (item) => String(item?.id || "") === buildingId,
      );
      // Opaque exterior shell already forms the tower. Glass wall bands on the same
      // footprint only create a hollow/flicker look in overview — keep them for
      // non-hero buildings or future cutaway modes.
      if (buildingPrim?.worldEnterable === true || Boolean(buildingPrim?.mapHero)) {
        return results;
      }
      const semantics = this._buildingSemantics.get(buildingId) || {};
      const profile = semantics.interiorProfile || {};
      const wallHeight = Number(
        prim.height ?? worldView.height ?? profile.wallHeight ?? 3.2,
      );
      const thickness = Number(
        prim.thickness ?? worldView.thickness ?? profile.wallThickness ?? 0.12,
      );
      const parentId = String(prim.parent || "").trim();
      const parentPrim = (this._worldPlan?.primitives || []).find(
        (item) => String(item?.id || "") === parentId,
      );
      const parentElevation =
        String(parentPrim?.kind || "") === "floor"
          ? Number(
              parentPrim.elevation ??
                parentPrim.worldView?.elevation ??
                parentPrim.world_view?.elevation ??
                0,
            )
          : 0;
      const relativeLift = Number(worldView.lift ?? worldView.elevationOffset ?? 0);
      const absoluteBase = worldView.elevation;
      const baseElevation =
        String(parentPrim?.kind || "") === "floor"
          ? parentElevation + relativeLift
          : Number(
              absoluteBase ??
                worldView.baseElevation ??
                parentElevation + relativeLift ??
                0,
            );
      const wallOpacity = Number(material.opacity ?? 0.82);
      const wallMaterial = new THREE.MeshStandardMaterial({
        color: parseHexColor(material.color, 0xf5f0e6),
        transparent: wallOpacity < 0.999,
        opacity: wallOpacity,
        side: THREE.DoubleSide,
        depthWrite: wallOpacity >= 0.85,
        depthTest: true,
        polygonOffset: true,
        polygonOffsetFactor: -1,
        polygonOffsetUnits: -1,
      });
      const wallY = baseElevation + wallHeight / 2;
      const edgeWalls = wallSegmentsFromRing(
        envelope.ring,
        this._siteOrigin,
        thickness,
        Math.max(thickness * 2.5, 0.35),
      );
      if (edgeWalls.length) {
        edgeWalls.forEach((seg, index) => {
          const wall = new THREE.Mesh(
            new THREE.BoxGeometry(thickness, wallHeight, seg.length),
            wallMaterial.clone(),
          );
          wall.position.set(seg.x, wallY, seg.z);
          wall.rotation.y = seg.yaw;
          wall.renderOrder = 20;
          results.push({
            meshId: `${prim.id}_${index + 1}`,
            mesh: wall,
            layerTags: [prim.id],
          });
        });
        return results;
      }
      const { center, halfW, halfD } = envelope;
      const wallSpecs = [
        [center.x, wallY, center.z - halfD, halfW * 2, wallHeight, thickness],
        [center.x, wallY, center.z + halfD, halfW * 2, wallHeight, thickness],
        [center.x - halfW, wallY, center.z, thickness, wallHeight, halfD * 2],
        [center.x + halfW, wallY, center.z, thickness, wallHeight, halfD * 2],
      ];
      wallSpecs.forEach((spec, index) => {
        const [x, y, z, w, h, d] = spec;
        const wall = new THREE.Mesh(new THREE.BoxGeometry(w, h, d), wallMaterial.clone());
        wall.position.set(x, y, z);
        wall.renderOrder = 20;
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
      const roofOpacity = Number(slabMaterial.opacity ?? material.opacity ?? 0.92);
      const roofMat = new THREE.MeshStandardMaterial({
        color: parseHexColor(slabMaterial.color ?? material.color, 0x8b5e34),
        transparent: roofOpacity < 0.95,
        opacity: roofOpacity < 0.95 ? roofOpacity : 1,
        side: THREE.DoubleSide,
        depthWrite: true,
        depthTest: true,
      });
      let roof;
      if (envelope.ring.length >= 3) {
        // Same footprint shape as shell/floors — AABB box skews on rotated parcels.
        const shape = ringToShape(envelope.ring, this._siteOrigin);
        const geom = new THREE.ExtrudeGeometry(shape, {
          depth: Math.max(thickness, 0.35),
          bevelEnabled: false,
        });
        geom.rotateX(-Math.PI / 2);
        roof = new THREE.Mesh(geom, roofMat);
        roof.position.set(0, elevation - Math.max(thickness, 0.35) * 0.5, 0);
      } else {
        const { center, halfW, halfD } = envelope;
        roof = new THREE.Mesh(
          new THREE.BoxGeometry(halfW * 2, Math.max(thickness, 0.35), halfD * 2),
          roofMat,
        );
        roof.position.set(center.x, elevation, center.z);
      }
      roof.renderOrder = 25;
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

  buildHeroExteriorMeshes(prim, ring, envelope, height) {
    if (!envelope || !THREE) return [];
    const buildingId = String(prim?.id || "hero");
    const entityId = String(prim?.featureEntityId || buildingId);
    const layerTags = [`${buildingId}:shell`, buildingId];
    const results = [];

    // ExtrudeGeometry caps can be back-facing after the Y-up transform. Add an
    // explicit double-sided crown cap so the building is never an open shell.
    if (Array.isArray(ring) && ring.length >= 3) {
      const capShape = ringToShape(ring, this._siteOrigin);
      const capGeometry = new THREE.ShapeGeometry(capShape);
      capGeometry.rotateX(-Math.PI / 2);
      const cap = new THREE.Mesh(
        capGeometry,
        new THREE.MeshStandardMaterial({
          color: parseHexColor(prim?.shellMaterial?.color, 0xf97316),
          side: THREE.DoubleSide,
          depthWrite: true,
          depthTest: true,
          roughness: 0.78,
          metalness: 0.04,
        }),
      );
      cap.position.y = Number(height) + 0.06;
      cap.renderOrder = 12;
      results.push({
        meshId: `${buildingId}:crown-cap`,
        mesh: cap,
        entityId,
        layerTags: [...layerTags, `${buildingId}:crown-cap`],
        roofMesh: true,
      });
    }

    const localPoints = ring
      .filter((coord) => Array.isArray(coord) && coord.length >= 2)
      .map((coord) => {
        const local = geoToLocal(coord[0], coord[1], this._siteOrigin);
        return { x: local.x, y: local.z };
      });
    if (localPoints.length >= 3) {
      const first = localPoints[0];
      const last = localPoints[localPoints.length - 1];
      if (first.x !== last.x || first.y !== last.y) {
        localPoints.push({ x: first.x, y: first.y });
      }
      const overlay = buildLocalHeroFacadeOverlay(THREE, localPoints, height, {
        ...prim,
        entityId,
        label: prim?.worldEnterLabel || prim?.label || buildingId,
      });
      if (overlay) {
        results.push({
          meshId: `${buildingId}:facade-overlay`,
          mesh: overlay,
          entityId,
          layerTags: [
            ...layerTags,
            `${buildingId}:facade-overlay`,
            `${buildingId}:billboard`,
          ],
        });
      }
    }
    return results;
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
      this.invalidate();
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
      const interiorMode =
        this._viewLayerVisibility.get("floor_1") === true ||
        this._viewLayerVisibility.get("play_floor_1") === true;
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
    this.invalidate();
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
    if (ids.length) this.invalidate();
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
    } else {
      this.invalidate();
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
    const minDistance = this.updateOrbitDistanceBounds(box);
    const distance = Math.max(
      Math.max(size.x, size.z, size.y) * 2.4 + 8,
      minDistance * 1.35,
      this.distanceToFitBox(box),
    );
    this._camera.position.set(
      center.x + distance * 0.55,
      center.y + distance * 0.72,
      center.z + distance * 0.55,
    );
    this._camera.lookAt(center);
    this.syncCameraNav(center);
    this.updateStatus(`focus ${entityId}`);
    this.invalidate();
  }

  applyCameraPreset(preset) {
    if (!preset || !this._camera) return;
    const mode = String(preset.mode || "layout").trim();
    const targetEntity = String(preset.targetEntity || preset.target_entity || "").trim();
    const targetMesh =
      this._meshes.get(targetEntity) ||
      this._meshes.get(`${targetEntity}:shell`) ||
      this._meshes.values().next().value;
    const box = targetMesh ? new THREE.Box3().setFromObject(targetMesh) : null;
    const center = box
      ? box.getCenter(new THREE.Vector3())
      : new THREE.Vector3(0, 0, 0);
    if (preset.cutaway || preset.default_layers || preset.defaultLayers) {
      this.applyCutawayState(preset);
    }
    if (mode === "inspect") {
      if (this._controls) {
        this._controls.minDistance = ORBIT_MIN_DISTANCE;
        this._controls.maxDistance = ORBIT_MAX_DISTANCE;
      }
      this._orbitAnchor = null;
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
      this.invalidate();
      return;
    }
    const minDistance = box
      ? this.updateOrbitDistanceBounds(box)
      : ORBIT_MIN_DISTANCE;
    const requestedDistance = Number(preset.distance ?? 36);
    const distance = Math.max(
      Number.isFinite(requestedDistance) ? requestedDistance : 36,
      minDistance * 1.35,
      box ? this.distanceToFitBox(box) : ORBIT_MIN_DISTANCE,
    );
    const pitch = Number(preset.pitch ?? 68);
    const bearing = Number(preset.bearing ?? 24);
    const pitchRad = degToRad(pitch);
    const bearingRad = degToRad(bearing);
    const y = center.y + distance * Math.sin(pitchRad);
    const planar = distance * Math.cos(pitchRad);
    const x = center.x + planar * Math.sin(bearingRad);
    const z = center.z + planar * Math.cos(bearingRad);
    if (String(preset.projection || "").trim() === "orthographic") {
      this._camera.fov = 38;
      this._camera.updateProjectionMatrix();
    }
    this._camera.position.set(x, Math.max(y, center.y + 12), z);
    this._camera.lookAt(center);
    this.syncCameraNav(center);
    this.updateStatus(`layout ${targetEntity || "site"}`);
    this.invalidate();
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
      this.invalidate();
      return true;
    }
    if (resolved.type === "hide_group" || resolved.type === "hideGroup") {
      if (resolved.groupId) this.setGroupVisible(resolved.groupId, false);
      this.setMeshGroupVisible(resolved.meshIds || resolved.meshes, false);
      this.invalidate();
      return true;
    }
    if (resolved.type === "cutaway_toggle" || resolved.type === "cutawayToggle") {
      const roofOn = this._viewLayerVisibility.get("roof") === true;
      this.applyViewLayerPreset(
        roofOn
          ? ["site", "floor_1", "props"]
          : ["site", "floor_1", "roof", "props"],
      );
      this.invalidate();
      return true;
    }
    if (resolved.entityId) {
      this.focusEntity(resolved.entityId);
      this.applyStageEntityVisibility(resolved.entityId);
    }
    if (preset) {
      this.applyCameraPreset(preset);
    }
    if (resolved.groupId) {
      this.setGroupVisible(resolved.groupId, true);
    }
    this.invalidate();
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
    const stage = resolveActiveWorldStage();
    stage?._controls?.reset();
    stage?.invalidate?.();
  },
};
