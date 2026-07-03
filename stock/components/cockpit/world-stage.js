import { escapeHtml, parseProps } from "./shared.js";
import * as THREE from "../vendor/three/three.module.min.js";
import { OrbitControls } from "../vendor/three/OrbitControls.js";
import {
  ensureWorldStageInputPlane,
  layoutWorldStageInputPlane,
  resolveCockpitMapToolHost,
  setWorldStageInputPlaneActive,
} from "./cockpit-stage-overlay.js";
import { resolveCockpitStageSurface } from "./map-focus-inset.js";

const TAG = "mei-world-stage";
const WORLD_RUNTIME_INSTANCES = new Set();
const boot = (window.__meiLangBoot = window.__meiLangBoot || {});
const ORBIT_MIN_DISTANCE = 5;
const ORBIT_MAX_DISTANCE = 96;
const ORBIT_MIN_POLAR_DEG = 18;
const ORBIT_MAX_POLAR_DEG = 80;
const PICK_MOVE_THRESHOLD_PX = 6;

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
    this._cutaway = { hideRoof: true };
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
      </style>
      <div class="viewport">
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
      const site = this.props?.worldSpec?.site || {};
      this._siteOrigin = {
        lng: Number(site.originLng ?? site.origin_lng ?? 106.38224),
        lat: Number(site.originLat ?? site.origin_lat ?? 29.62396),
      };
      this._scene = new THREE.Scene();
      this._scene.background = new THREE.Color(0x071526);
      this._scene.fog = new THREE.Fog(0x071526, 80, 260);
      const width = Math.max(320, this.clientWidth || viewport.clientWidth || 320);
      const height = Math.max(240, this.clientHeight || viewport.clientHeight || 240);
      this._camera = new THREE.PerspectiveCamera(52, width / height, 0.1, 500);
      this._camera.position.set(26, 34, 38);
      this._renderer = new THREE.WebGLRenderer({ antialias: true, alpha: false });
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
      ground.position.y = -0.02;
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
    const worldSpec = this.props?.worldSpec || {};
    const bridgeUrl = String(
      worldSpec.bridgeFixtureUrl ||
        worldSpec.bridge_fixture_url ||
        this.props?.bridgeFixtureUrl ||
        "",
    ).trim();
    const geoUrl = String(
      worldSpec.footprintGeoJsonUrl ||
        worldSpec.footprint_geojson_url ||
        "",
    ).trim();
    const interiorUrl = String(
      worldSpec.interiorFixtureUrl ||
        worldSpec.interior_fixture_url ||
        "",
    ).trim();
    const [bridge, geojson, interior] = await Promise.all([
      bridgeUrl ? this.fetchJson(bridgeUrl) : Promise.resolve(null),
      geoUrl ? this.fetchJson(geoUrl) : Promise.resolve(null),
      interiorUrl ? this.fetchJson(interiorUrl) : Promise.resolve(null),
    ]);
    if (geojson?.features) {
      this.buildFootprintMeshes(geojson, bridge);
    }
    if (interior) {
      this.buildInteriorMeshes(interior);
    }
  }

  async fetchJson(url) {
    const response = await fetch(url, { credentials: "same-origin" });
    if (!response.ok) {
      throw new Error(`加载 ${url} 失败 (${response.status})`);
    }
    return response.json();
  }

  buildFootprintMeshes(geojson, bridge) {
    const projections = bridge?.objects || {};
    for (const feature of geojson.features || []) {
      const entityId = readEntityId(feature);
      if (!entityId) continue;
      const projection = projections?.[entityId]?.projections?.world_3d || {};
      const renderFamily = String(projection.renderFamily || "").trim();
      const geometry = feature.geometry;
      if (!geometry) continue;
      let mesh = null;
      if (geometry.type === "Polygon" && renderFamily === "extrude_shell") {
        const ring = geometry.coordinates?.[0] || [];
        const shape = ringToShape(ring, this._siteOrigin);
        const height = Number(projection.height ?? 8.6);
        const geom = new THREE.ExtrudeGeometry(shape, {
          depth: height,
          bevelEnabled: false,
        });
        geom.rotateX(-Math.PI / 2);
        mesh = new THREE.Mesh(
          geom,
          new THREE.MeshStandardMaterial({
            color: entityId === "lake_pavilion" ? 0xffd36b : 0x5d8fd6,
            roughness: 0.58,
            metalness: 0.08,
            transparent: true,
            opacity: entityId === "lake_pavilion" ? 0.9 : 0.72,
          }),
        );
        mesh.userData.roofMesh = true;
      } else if (geometry.type === "Polygon") {
        const ring = geometry.coordinates?.[0] || [];
        const shape = ringToShape(ring, this._siteOrigin);
        const geom = new THREE.ShapeGeometry(shape);
        geom.rotateX(-Math.PI / 2);
        const color =
          renderFamily === "flat_fill"
            ? 0x2d7fb0
            : renderFamily === "site_outline"
              ? 0x93c5fd
              : 0x1f4f74;
        mesh = new THREE.Mesh(
          geom,
          new THREE.MeshStandardMaterial({
            color,
            transparent: true,
            opacity: renderFamily === "site_outline" ? 0.18 : 0.62,
            side: THREE.DoubleSide,
          }),
        );
      } else if (geometry.type === "LineString" && renderFamily === "route_ribbon") {
        const points = (geometry.coordinates || []).map((coord) => {
          const local = geoToLocal(coord[0], coord[1], this._siteOrigin);
          return new THREE.Vector3(local.x, 0.35, local.z);
        });
        const curve = new THREE.CatmullRomCurve3(points);
        const geom = new THREE.TubeGeometry(curve, 48, 0.45, 8, false);
        mesh = new THREE.Mesh(
          geom,
          new THREE.MeshStandardMaterial({ color: 0xfde68a, emissive: 0x5c4b12, emissiveIntensity: 0.2 }),
        );
      }
      if (!mesh) continue;
      mesh.name = entityId;
      mesh.userData.entityId = entityId;
      this._scene.add(mesh);
      this._meshes.set(entityId, mesh);
      const groupIds = this.resolveEntityGroups(entityId);
      groupIds.forEach((groupId) => {
        if (!this._groups.has(groupId)) {
          this._groups.set(groupId, new Set());
        }
        this._groups.get(groupId).add(entityId);
      });
    }
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

  buildInteriorMeshes(interior) {
    const buildingId = String(interior.buildingId || "lake_pavilion");
    const building = this._meshes.get(buildingId);
    if (!building) return;
    const box = new THREE.Box3().setFromObject(building);
    const center = box.getCenter(new THREE.Vector3());
    const size = box.getSize(new THREE.Vector3());
    const floor = new THREE.Mesh(
      new THREE.PlaneGeometry(Math.max(size.x, 4), Math.max(size.z, 4)),
      new THREE.MeshStandardMaterial({ color: 0xd9c7a2, side: THREE.DoubleSide }),
    );
    floor.rotation.x = -Math.PI / 2;
    floor.position.set(center.x, 0.05, center.z);
    floor.name = "floor_1";
    floor.userData.entityId = "floor_1";
    this._scene.add(floor);
    this._meshes.set("floor_1", floor);
    const wallMaterial = new THREE.MeshStandardMaterial({
      color: 0xf5f0e6,
      transparent: true,
      opacity: 0.82,
      side: THREE.DoubleSide,
    });
    const wallHeight = 3.2;
    const halfW = Math.max(size.x * 0.42, 2.2);
    const halfD = Math.max(size.z * 0.42, 2.2);
    const walls = [
      [center.x, wallHeight / 2, center.z - halfD, halfW * 2, wallHeight, 0.12],
      [center.x, wallHeight / 2, center.z + halfD, halfW * 2, wallHeight, 0.12],
      [center.x - halfW, wallHeight / 2, center.z, 0.12, wallHeight, halfD * 2],
      [center.x + halfW, wallHeight / 2, center.z, 0.12, wallHeight, halfD * 2],
    ];
    walls.forEach((spec, index) => {
      const [x, y, z, w, h, d] = spec;
      const wall = new THREE.Mesh(new THREE.BoxGeometry(w, h, d), wallMaterial.clone());
      wall.position.set(x, y, z);
      wall.name = `wall_${index + 1}`;
      wall.userData.interior = true;
      this._scene.add(wall);
      this._meshes.set(wall.name, wall);
    });
    const roof = new THREE.Mesh(
      new THREE.BoxGeometry(halfW * 2.1, 0.18, halfD * 2.1),
      new THREE.MeshStandardMaterial({ color: 0x8b5e34, transparent: true, opacity: 0.88 }),
    );
    roof.position.set(center.x, wallHeight + 0.1, center.z);
    roof.name = "roof_shell";
    roof.userData.roofMesh = true;
    this._scene.add(roof);
    this._meshes.set("roof_shell", roof);
    for (const device of interior.devices || []) {
      const position = device.position || [center.x + 1, 1.4, center.z + 0.6];
      const screen = new THREE.Mesh(
        new THREE.PlaneGeometry(1.2, 0.72),
        new THREE.MeshStandardMaterial({
          color: 0x38bdf8,
          emissive: 0x0ea5e9,
          emissiveIntensity: 0.45,
          side: THREE.DoubleSide,
        }),
      );
      screen.position.set(
        center.x + Number(position[0]),
        Number(position[1]),
        center.z + Number(position[2]),
      );
      screen.name = String(device.id || "info_screen_1");
      screen.userData.entityId = screen.name;
      screen.userData.device = true;
      this._scene.add(screen);
      this._meshes.set(screen.name, screen);
    }
    this.applyCutawayState(interior.cutaway || { hideRoof: true });
  }

  applyCutawayState(cutaway) {
    this._cutaway = {
      hideRoof: cutaway?.hideRoof !== false && cutaway?.roof !== true,
      ...(cutaway || {}),
    };
    for (const mesh of this._meshes.values()) {
      if (mesh.userData?.roofMesh || mesh.name === "roof_shell") {
        mesh.visible = !this._cutaway.hideRoof;
      }
    }
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
    const mesh = this._meshes.get(String(entityId || "").trim());
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
      this._meshes.get("lake_pavilion") ||
      this._meshes.values().next().value;
    const box = targetMesh ? new THREE.Box3().setFromObject(targetMesh) : null;
    const center = box
      ? box.getCenter(new THREE.Vector3())
      : new THREE.Vector3(0, 0, 0);
    if (preset.cutaway) {
      this.applyCutawayState(preset.cutaway);
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
      this.applyCutawayState({
        ...this._cutaway,
        hideRoof: !this._cutaway.hideRoof,
      });
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
