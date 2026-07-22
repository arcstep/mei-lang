/**
 * MapLibre Custom Layer hosting sparse L4 hero meshes (Three.js).
 * Contract: docs/mei-lang-v2/03-ui/0337-map-hero-custom-layer.md
 *
 * MapLibre 5.x passes projection via `args.defaultProjectionData.mainMatrix`
 * (not a bare matrix array). Uses shared map WebGL context (official pattern).
 */

import { ensureThreeGlobal } from "../../vendor/runtime-libs.js";
import { buildHeroFacade } from "./map-hero-facade.js";

export const MAP_HERO_LAYER_ID = "mei-map-heroes";
const DEFAULT_MAX_HEROES = 3;

function runtimeDiag() {
  return typeof window !== "undefined" ? window.__meiBrowserRuntimeDiag : null;
}

function readMapHeroes(worldRef) {
  const proj = typeof window !== "undefined" ? window.__mei?.map_projection : null;
  if (!proj || typeof proj !== "object") return [];
  const flat = Array.isArray(proj.heroes) ? proj.heroes : [];
  if (worldRef) {
    const scoped = proj.worlds?.[worldRef]?.heroes;
    if (Array.isArray(scoped) && scoped.length) return scoped;
    return flat.filter((h) => !h?.worldRef || String(h.worldRef) === String(worldRef));
  }
  if (flat.length) return flat;
  const worlds = proj.worlds && typeof proj.worlds === "object" ? Object.values(proj.worlds) : [];
  const out = [];
  for (const w of worlds) {
    if (Array.isArray(w?.heroes)) out.push(...w.heroes);
  }
  return out;
}

function disposeObject3D(obj) {
  if (!obj) return;
  obj.traverse?.((child) => {
    if (child.geometry) child.geometry.dispose?.();
    if (child.material) {
      if (Array.isArray(child.material)) {
        child.material.forEach((m) => m.dispose?.());
      } else {
        child.material.dispose?.();
      }
    }
  });
}

/** MapLibre 5 args object or legacy Float32/64 matrix array. */
function resolveMainMatrix(argsOrMatrix) {
  if (!argsOrMatrix) return null;
  if (Array.isArray(argsOrMatrix) || ArrayBuffer.isView(argsOrMatrix)) {
    return argsOrMatrix;
  }
  if (argsOrMatrix.defaultProjectionData?.mainMatrix) {
    return argsOrMatrix.defaultProjectionData.mainMatrix;
  }
  if (argsOrMatrix.mainMatrix) return argsOrMatrix.mainMatrix;
  return null;
}

/**
 * @param {object} options
 * @param {() => string} options.getWorldRef
 * @param {(entityIds: string[]) => void} options.onActiveHeroesChange
 * @param {(hero: object, point: {x:number,y:number}) => void} [options.onHeroClick]
 * @param {number} [options.maxHeroes]
 */
export function createMapHeroLayerController(options = {}) {
  const getWorldRef = options.getWorldRef || (() => "");
  const onActiveHeroesChange = options.onActiveHeroesChange || (() => {});
  const onHeroClick = options.onHeroClick || null;
  const maxHeroesCap = Number(options.maxHeroes) > 0 ? Number(options.maxHeroes) : DEFAULT_MAX_HEROES;

  let map = null;
  let THREE = null;
  let renderer = null;
  let scene = null;
  let camera = null;
  let activeEntityIds = [];
  let heroGroups = new Map();
  let buildToken = 0;
  let attached = false;
  let threeReady = false;
  let pendingGl = null;
  let clickBound = false;

  function setActiveEntityIds(ids) {
    const next = [...ids].sort();
    const prev = [...activeEntityIds].sort();
    const same = next.length === prev.length && next.every((id, i) => id === prev[i]);
    activeEntityIds = ids;
    if (!same) onActiveHeroesChange(ids);
  }

  function clearHeroes() {
    for (const group of heroGroups.values()) {
      scene?.remove(group);
      disposeObject3D(group);
    }
    heroGroups.clear();
    setActiveEntityIds([]);
  }

  function ensureRenderer(gl) {
    if (!THREE || !map || renderer) return;
    renderer = new THREE.WebGLRenderer({
      canvas: map.getCanvas(),
      context: gl,
      antialias: true,
    });
    renderer.autoClear = false;
  }

  async function bootstrapThree(gl) {
    if (threeReady) {
      ensureRenderer(gl);
      return;
    }
    THREE = await ensureThreeGlobal();
    scene = new THREE.Scene();
    camera = new THREE.Camera();
    const light = new THREE.DirectionalLight(0xffffff, 1.05);
    light.position.set(80, 120, 60);
    scene.add(light);
    scene.add(new THREE.AmbientLight(0xffffff, 0.55));
    ensureRenderer(gl || pendingGl);
    threeReady = true;
    await rebuildHeroes();
    map?.triggerRepaint();
  }

  async function rebuildHeroes() {
    if (!map || !THREE || !scene) return;
    const maplibre = window.maplibregl || maplibreglGlobal();
    if (!maplibre?.MercatorCoordinate) return;

    const token = ++buildToken;
    const zoom = map.getZoom();
    const worldRef = getWorldRef();
    const catalog = readMapHeroes(worldRef);
    const eligible = catalog
      .filter((h) => h && String(h.entityId || "").trim())
      .filter((h) => {
        const minZoom = Number(h.minZoom);
        return !Number.isFinite(minZoom) || zoom >= minZoom;
      });

    if (!eligible.length) {
      clearHeroes();
      map.triggerRepaint();
      return;
    }

    let budget = maxHeroesCap;
    for (const h of eligible) {
      const b = Number(h.maxCountBudget);
      if (Number.isFinite(b) && b > 0) budget = Math.min(budget, b);
    }
    const selected = eligible.slice(0, Math.max(1, budget));
    const nextIds = new Set(selected.map((h) => String(h.entityId || "")));

    for (const [id, group] of [...heroGroups.entries()]) {
      if (!nextIds.has(id)) {
        scene.remove(group);
        disposeObject3D(group);
        heroGroups.delete(id);
      }
    }

    const built = [];
    for (const hero of selected) {
      if (token !== buildToken) return;
      const id = String(hero.entityId || "");
      if (!id) continue;
      if (heroGroups.has(id)) {
        built.push(id);
        continue;
      }
      let group = null;
      try {
        group = await buildHeroFacade(THREE, maplibre, hero);
      } catch (error) {
        console.warn("[mei-map-heroes] build facade failed", id, error);
      }
      if (token !== buildToken) {
        disposeObject3D(group);
        return;
      }
      if (!group) continue;
      scene.add(group);
      heroGroups.set(id, group);
      built.push(id);
    }

    // Only hide L3 extrusion when at least one hero mesh is actually in the scene.
    setActiveEntityIds(built);
    runtimeDiag()?.recordMap?.("map_heroes_rebuild", {
      count: built.length,
      entityIds: built,
      zoom,
      worldRef,
      catalog: catalog.length,
    });
    map.triggerRepaint();
  }

  function maplibreglGlobal() {
    return typeof window !== "undefined" ? window.maplibregl : null;
  }

  function renderHeroes(argsOrMatrix) {
    if (!renderer || !scene || !camera || !THREE || !heroGroups.size) return;
    const matrix = resolveMainMatrix(argsOrMatrix);
    if (!matrix) {
      runtimeDiag()?.recordMap?.("map_heroes_matrix_missing", {});
      return;
    }

    const m = new THREE.Matrix4().fromArray(matrix);
    renderer.resetState();

    for (const group of heroGroups.values()) {
      const originMc = group.userData?.originMc;
      const meter = Number(group.userData?.meter);
      if (!originMc || !Number.isFinite(meter) || meter <= 0) continue;

      const rotationX = new THREE.Matrix4().makeRotationAxis(new THREE.Vector3(1, 0, 0), Math.PI / 2);
      const l = new THREE.Matrix4()
        .makeTranslation(originMc.x, originMc.y, originMc.z)
        .scale(new THREE.Vector3(meter, -meter, meter))
        .multiply(rotationX);

      for (const other of heroGroups.values()) {
        other.visible = other === group;
      }
      camera.projectionMatrix = m.clone().multiply(l);
      renderer.render(scene, camera);
    }
    for (const group of heroGroups.values()) {
      group.visible = true;
    }
  }

  function pickHero(clientX, clientY) {
    if (!map || !heroGroups.size) return null;
    const canvas = map.getCanvas();
    const rect = canvas.getBoundingClientRect();
    let best = null;
    let bestDist = Infinity;
    for (const group of heroGroups.values()) {
      const originMc = group.userData?.originMc;
      if (!originMc?.toLngLat) continue;
      const lngLat = originMc.toLngLat();
      const point = map.project([lngLat.lng, lngLat.lat]);
      const dx = point.x - (clientX - rect.left);
      const dy = point.y - (clientY - rect.top);
      const dist = Math.hypot(dx, dy);
      if (dist < bestDist && dist < 64) {
        bestDist = dist;
        best = group;
      }
    }
    return best;
  }

  function onMapClick(event) {
    if (!onHeroClick || !heroGroups.size) return;
    const oe = event.originalEvent;
    if (!oe) return;
    const target = pickHero(oe.clientX, oe.clientY);
    if (!target) return;
    onHeroClick(target.userData || {}, { x: oe.clientX, y: oe.clientY });
  }

  const customLayer = {
    id: MAP_HERO_LAYER_ID,
    type: "custom",
    renderingMode: "3d",
    onAdd(mapInstance, gl) {
      map = mapInstance;
      pendingGl = gl;
      // MapLibre does not await async onAdd — bootstrap Three without blocking registration.
      bootstrapThree(gl).catch((error) => {
        console.warn("[mei-map-heroes] bootstrap failed", error);
        setActiveEntityIds([]);
      });
      if (onHeroClick && !clickBound) {
        map.on("click", onMapClick);
        clickBound = true;
      }
      attached = true;
      map.on("zoomend", onZoomOrMoveEnd);
      map.on("moveend", onZoomOrMoveEnd);
    },
    render(gl, argsOrMatrix) {
      if (!threeReady) {
        pendingGl = gl;
        return;
      }
      ensureRenderer(gl);
      renderHeroes(argsOrMatrix);
    },
    onRemove() {
      map?.off("zoomend", onZoomOrMoveEnd);
      map?.off("moveend", onZoomOrMoveEnd);
      if (clickBound && map) {
        map.off("click", onMapClick);
        clickBound = false;
      }
      clearHeroes();
      // Do not dispose the shared map WebGL context — only release our Three wrapper.
      renderer = null;
      scene = null;
      camera = null;
      THREE = null;
      threeReady = false;
      pendingGl = null;
      attached = false;
      map = null;
      setActiveEntityIds([]);
    },
  };

  function onZoomOrMoveEnd() {
    if (!threeReady) return;
    rebuildHeroes().catch(() => {});
  }

  return {
    customLayer,
    isAttached: () => attached,
    getActiveEntityIds: () => [...activeEntityIds],
    rebuild: () => rebuildHeroes(),
    dispose: () => {
      if (map?.getLayer?.(MAP_HERO_LAYER_ID)) {
        try {
          map.removeLayer(MAP_HERO_LAYER_ID);
        } catch (_) {
          /* ignore */
        }
      } else {
        customLayer.onRemove?.();
      }
    },
  };
}

export function collectMapHeroesForWorld(worldRef) {
  return readMapHeroes(worldRef);
}
