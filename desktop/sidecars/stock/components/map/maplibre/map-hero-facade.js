/**
 * L4 procedural facade + optional glTF load with window_bands fallback.
 * Contract: docs/mei-lang-v2/03-ui/0337-map-hero-custom-layer.md
 *
 * Visual extras for landmark heroes:
 * - large rectangular facade panels on each wall
 * - street-facing billboard text (e.g. 「广东国际」)
 */

const DEFAULT_FLOOR_HEIGHT = 4.0;
const BAND_INSET = 0.08;
const PANEL_OUTSET = 0.35;
const BILLBOARD_OUTSET = 1.2;

function parseLngLat(pair) {
  if (!Array.isArray(pair) || pair.length < 2) return null;
  const lng = Number(pair[0]);
  const lat = Number(pair[1]);
  if (!Number.isFinite(lng) || !Number.isFinite(lat)) return null;
  return [lng, lat];
}

export function normalizeHeroRing(coordinates) {
  if (!Array.isArray(coordinates)) return [];
  const ring = [];
  for (const pair of coordinates) {
    const ll = parseLngLat(pair);
    if (ll) ring.push(ll);
  }
  if (ring.length < 3) return [];
  const first = ring[0];
  const last = ring[ring.length - 1];
  if (first[0] !== last[0] || first[1] !== last[1]) {
    ring.push([first[0], first[1]]);
  }
  return ring;
}

function ringToLocalMeters(ring, maplibregl) {
  const originMc = maplibregl.MercatorCoordinate.fromLngLat(ring[0], 0);
  const meter = originMc.meterInMercatorCoordinateUnits();
  const points = ring.map(([lng, lat]) => {
    const mc = maplibregl.MercatorCoordinate.fromLngLat([lng, lat], 0);
    return {
      x: (mc.x - originMc.x) / meter,
      y: (mc.y - originMc.y) / meter,
    };
  });
  return { originMc, meter, points };
}

function ringCentroid(points) {
  const n = Math.max(points.length - 1, 1);
  let cx = 0;
  let cy = 0;
  for (let i = 0; i < n; i += 1) {
    cx += points[i].x;
    cy += points[i].y;
  }
  return { x: cx / n, y: cy / n };
}

function pushQuad(positions, normals, uvs, a, b, c, d, nx, ny, nz) {
  const verts = [a, b, c, a, c, d];
  const uvCorner = [
    [0, 0],
    [1, 0],
    [1, 1],
    [0, 0],
    [1, 1],
    [0, 1],
  ];
  verts.forEach((v, i) => {
    positions.push(v[0], v[1], v[2]);
    normals.push(nx, ny, nz);
    if (uvs) uvs.push(uvCorner[i][0], uvCorner[i][1]);
  });
}

function buildWallQuads(points, z0, z1, positions, normals) {
  for (let i = 0; i < points.length - 1; i += 1) {
    const p0 = points[i];
    const p1 = points[i + 1];
    const dx = p1.x - p0.x;
    const dy = p1.y - p0.y;
    const len = Math.hypot(dx, dy) || 1;
    const nx = dy / len;
    const ny = -dx / len;
    pushQuad(
      positions,
      normals,
      null,
      [p0.x, z0, p0.y],
      [p1.x, z0, p1.y],
      [p1.x, z1, p1.y],
      [p0.x, z1, p0.y],
      nx,
      0,
      ny,
    );
  }
}

function buildRoof(points, height, positions, normals) {
  if (points.length < 4) return;
  const c = ringCentroid(points);
  for (let i = 0; i < points.length - 1; i += 1) {
    const p0 = points[i];
    const p1 = points[i + 1];
    positions.push(c.x, height, c.y, p0.x, height, p0.y, p1.x, height, p1.y);
    normals.push(0, 1, 0, 0, 1, 0, 0, 1, 0);
  }
}

function meshFromBuffers(THREE, positions, normals, color, opacity, uvs = null, map = null) {
  const geometry = new THREE.BufferGeometry();
  geometry.setAttribute("position", new THREE.Float32BufferAttribute(positions, 3));
  geometry.setAttribute("normal", new THREE.Float32BufferAttribute(normals, 3));
  if (uvs && uvs.length) {
    geometry.setAttribute("uv", new THREE.Float32BufferAttribute(uvs, 2));
  }
  geometry.computeBoundingSphere();
  const opaque = opacity >= 0.9 && !map;
  const material = new THREE.MeshStandardMaterial({
    color: map ? 0xffffff : color,
    map: map || null,
    transparent: !opaque && (opacity < 0.999 || Boolean(map)),
    opacity: opaque ? 1 : opacity,
    metalness: map ? 0.15 : 0.05,
    roughness: map ? 0.45 : 0.72,
    side: THREE.DoubleSide,
    depthWrite: opaque || !map,
    // Three.js warns if a Material param is present but undefined — omit via black+0 when no map.
    emissive: map ? new THREE.Color("#1e3a5f") : 0x000000,
    emissiveIntensity: map ? 0.35 : 0,
  });
  return new THREE.Mesh(geometry, material);
}

/** Edges of closed ring (skip duplicate close point). */
function wallEdges(points) {
  const edges = [];
  for (let i = 0; i < points.length - 1; i += 1) {
    const p0 = points[i];
    const p1 = points[i + 1];
    const dx = p1.x - p0.x;
    const dy = p1.y - p0.y;
    const len = Math.hypot(dx, dy);
    if (len < 1.5) continue;
    const nx = dy / len;
    const ny = -dx / len;
    edges.push({ p0, p1, dx, dy, len, nx, ny, index: i });
  }
  return edges;
}

function ringSignedArea(points) {
  let area = 0;
  for (let i = 0; i < points.length - 1; i += 1) {
    area += points[i].x * points[i + 1].y - points[i + 1].x * points[i].y;
  }
  return area * 0.5;
}

/**
 * Outward normal in footprint XZ. GeoJSON exteriors are usually CCW → outward = right of edge.
 * Always force away from centroid so CW rings / digitizing noise cannot put boards inside.
 */
function outwardNormal(edge, centroid, ringCCW) {
  const rightX = edge.dy / edge.len;
  const rightY = -edge.dx / edge.len;
  let nx = ringCCW ? rightX : -rightX;
  let ny = ringCCW ? rightY : -rightY;
  const mx = (edge.p0.x + edge.p1.x) * 0.5;
  const my = (edge.p0.y + edge.p1.y) * 0.5;
  if ((mx - centroid.x) * nx + (my - centroid.y) * ny < 0) {
    nx = -nx;
    ny = -ny;
  }
  return { nx, ny };
}

/**
 * Quad on a wall, offset along outward normal.
 * Vertex order is CCW when viewed from outside so FrontSide / UV text reads correctly (not mirrored).
 */
function wallQuadAt(edge, centroid, z0, z1, u0, u1, outset, ringCCW) {
  const { nx, ny } = outwardNormal(edge, centroid, ringCCW);
  const pAt = (u) => ({
    x: edge.p0.x + edge.dx * u + nx * outset,
    y: edge.p0.y + edge.dy * u + ny * outset,
  });
  let left = pAt(u0);
  let right = pAt(u1);
  // +right when viewed from outside = up × outward = (ny, -nx)
  const viewRightX = ny;
  const viewRightY = -nx;
  const alongX = edge.dx * (u1 - u0);
  const alongY = edge.dy * (u1 - u0);
  if (alongX * viewRightX + alongY * viewRightY < 0) {
    const tmp = left;
    left = right;
    right = tmp;
  }
  return {
    a: [left.x, z0, left.y],
    b: [right.x, z0, right.y],
    c: [right.x, z1, right.y],
    d: [left.x, z1, left.y],
    nx,
    ny,
  };
}

function createPanelPatternTexture() {
  if (typeof document === "undefined") return null;
  const canvas = document.createElement("canvas");
  canvas.width = 512;
  canvas.height = 768;
  const ctx = canvas.getContext("2d");
  if (!ctx) return null;

  // Deep night facade plate
  const grad = ctx.createLinearGradient(0, 0, 0, canvas.height);
  grad.addColorStop(0, "#0c4a6e");
  grad.addColorStop(0.45, "#075985");
  grad.addColorStop(1, "#0f172a");
  ctx.fillStyle = grad;
  ctx.fillRect(0, 0, canvas.width, canvas.height);

  // Outer gold frame
  ctx.strokeStyle = "#fbbf24";
  ctx.lineWidth = 18;
  ctx.strokeRect(28, 28, canvas.width - 56, canvas.height - 56);

  // Inner cyan rectangle motif
  ctx.strokeStyle = "#38bdf8";
  ctx.lineWidth = 10;
  ctx.strokeRect(70, 90, canvas.width - 140, canvas.height - 220);

  // Grid of luminous cells
  const cols = 4;
  const rows = 6;
  const x0 = 95;
  const y0 = 120;
  const cellW = (canvas.width - 190) / cols;
  const cellH = (canvas.height - 280) / rows;
  for (let r = 0; r < rows; r += 1) {
    for (let c = 0; c < cols; c += 1) {
      const x = x0 + c * cellW;
      const y = y0 + r * cellH;
      ctx.fillStyle = (r + c) % 2 === 0 ? "rgba(56,189,248,0.35)" : "rgba(251,191,36,0.18)";
      ctx.fillRect(x + 8, y + 8, cellW - 16, cellH - 16);
      ctx.strokeStyle = "rgba(226,232,240,0.45)";
      ctx.lineWidth = 2;
      ctx.strokeRect(x + 8, y + 8, cellW - 16, cellH - 16);
    }
  }

  // Bottom accent bar
  ctx.fillStyle = "#f97316";
  ctx.fillRect(70, canvas.height - 100, canvas.width - 140, 28);

  const texture = { canvas, needsUpdate: true };
  return texture;
}

function createBillboardTexture(text) {
  if (typeof document === "undefined") return null;
  const canvas = document.createElement("canvas");
  canvas.width = 1024;
  canvas.height = 288;
  const ctx = canvas.getContext("2d");
  if (!ctx) return null;

  ctx.clearRect(0, 0, canvas.width, canvas.height);

  // Sign board
  const pad = 16;
  ctx.fillStyle = "rgba(15, 23, 42, 0.92)";
  ctx.strokeStyle = "#fbbf24";
  ctx.lineWidth = 10;
  roundRect(ctx, pad, pad, canvas.width - pad * 2, canvas.height - pad * 2, 18);
  ctx.fill();
  ctx.stroke();

  // Inner glow bar
  ctx.fillStyle = "#0ea5e9";
  ctx.fillRect(pad + 24, canvas.height - 52, canvas.width - pad * 2 - 48, 10);

  ctx.fillStyle = "#f8fafc";
  ctx.textAlign = "center";
  ctx.textBaseline = "middle";
  ctx.font = "bold 140px \"PingFang SC\", \"Noto Sans SC\", \"Microsoft YaHei\", sans-serif";
  ctx.shadowColor = "rgba(14,165,233,0.65)";
  ctx.shadowBlur = 24;
  ctx.fillText(text, canvas.width / 2, canvas.height / 2 - 6);
  ctx.shadowBlur = 0;

  return { canvas, needsUpdate: true };
}

function roundRect(ctx, x, y, w, h, r) {
  const radius = Math.min(r, w / 2, h / 2);
  ctx.beginPath();
  ctx.moveTo(x + radius, y);
  ctx.arcTo(x + w, y, x + w, y + h, radius);
  ctx.arcTo(x + w, y + h, x, y + h, radius);
  ctx.arcTo(x, y + h, x, y, radius);
  ctx.arcTo(x, y, x + w, y, radius);
  ctx.closePath();
}

function toThreeTexture(THREE, sketch) {
  if (!sketch?.canvas) return null;
  const tex = new THREE.CanvasTexture(sketch.canvas);
  if (THREE.SRGBColorSpace != null) {
    tex.colorSpace = THREE.SRGBColorSpace;
  } else if (THREE.sRGBEncoding != null) {
    tex.encoding = THREE.sRGBEncoding;
  }
  tex.needsUpdate = true;
  tex.anisotropy = 4;
  return tex;
}

function resolveBillboardText(hero) {
  const label = String(hero?.worldEnterLabel || hero?.label || "").trim();
  if (label.includes("广东国际")) return "广东国际";
  if (label && label.length <= 8) return label.replace(/\s*3D\s*$/i, "").trim() || "广东国际";
  return "广东国际";
}

function addFacadeDecor(THREE, group, points, height, hero) {
  const edges = wallEdges(points);
  if (!edges.length) return;
  const centroid = ringCentroid(points);
  const ringCCW = ringSignedArea(points) > 0;

  // Pick longest edge as street-facing billboard wall.
  let front = edges[0];
  for (const edge of edges) {
    if (edge.len > front.len) front = edge;
  }

  const panelTex = toThreeTexture(THREE, createPanelPatternTexture());
  const panelH = Math.min(Math.max(height * 0.28, 28), 56);
  const panelBottom = Math.min(Math.max(height * 0.42, 36), height - panelH - 12);
  const panelTop = panelBottom + panelH;

  for (const edge of edges) {
    // One oversized plate per facade, inset from corners so it reads as a mural.
    const margin = Math.min(0.18, 12 / edge.len);
    const u0 = margin;
    const u1 = 1 - margin;
    const q = wallQuadAt(edge, centroid, panelBottom, panelTop, u0, u1, PANEL_OUTSET, ringCCW);
    const pos = [];
    const nrm = [];
    const uvs = [];
    pushQuad(pos, nrm, uvs, q.a, q.b, q.c, q.d, q.nx, 0, q.ny);
    const mesh = meshFromBuffers(THREE, pos, nrm, "#0ea5e9", 0.92, uvs, panelTex);
    mesh.name = `facade-panel:${edge.index}`;
    group.add(mesh);

    // Slim gold frame bars (top + bottom accents)
    for (const [z0, z1] of [
      [panelTop + 0.4, panelTop + 1.6],
      [panelBottom - 1.6, panelBottom - 0.4],
    ]) {
      if (z0 < 2 || z1 > height - 2) continue;
      const frame = wallQuadAt(edge, centroid, z0, z1, u0, u1, PANEL_OUTSET + 0.08, ringCCW);
      const fp = [];
      const fn = [];
      pushQuad(fp, fn, null, frame.a, frame.b, frame.c, frame.d, frame.nx, 0, frame.ny);
      group.add(meshFromBuffers(THREE, fp, fn, "#fbbf24", 0.95));
    }
  }

  // Billboard near the crown on the longest facade (street-facing).
  const billboardText = resolveBillboardText(hero);
  const boardTex = toThreeTexture(THREE, createBillboardTexture(billboardText));
  const boardW = Math.min(front.len * 0.72, 48);
  const boardH = Math.min(Math.max(boardW * 0.28, 10), 16);
  const boardCenterU = 0.5;
  const halfU = boardW / 2 / front.len;
  const topClearance = Math.max(2.5, height * 0.012);
  const boardZ1 = height - topClearance;
  const boardZ0 = Math.max(boardZ1 - boardH, height * 0.82);
  const board = wallQuadAt(
    front,
    centroid,
    boardZ0,
    boardZ1,
    boardCenterU - halfU,
    boardCenterU + halfU,
    BILLBOARD_OUTSET,
    ringCCW,
  );
  const bPos = [];
  const bNrm = [];
  const bUv = [];
  pushQuad(bPos, bNrm, bUv, board.a, board.b, board.c, board.d, board.nx, 0, board.ny);
  const billboard = meshFromBuffers(THREE, bPos, bNrm, "#0f172a", 1, bUv, boardTex);
  billboard.name = "facade-billboard";
  // FrontSide only: avoid reading the mirrored back face from the street.
  if (billboard.material) {
    billboard.material.side = THREE.FrontSide;
    billboard.material.emissive = new THREE.Color("#38bdf8");
    billboard.material.emissiveIntensity = 0.25;
    billboard.material.metalness = 0.05;
    billboard.material.roughness = 0.4;
  }
  group.add(billboard);

  // Small support poles under the board (visual “sticker” depth)
  const poleH = 2.2;
  for (const u of [boardCenterU - halfU * 0.85, boardCenterU + halfU * 0.85]) {
    const pole = wallQuadAt(
      front,
      centroid,
      boardZ0 - poleH,
      boardZ0,
      u - 0.01,
      u + 0.01,
      BILLBOARD_OUTSET - 0.15,
      ringCCW,
    );
    const pp = [];
    const pn = [];
    pushQuad(pp, pn, null, pole.a, pole.b, pole.c, pole.d, pole.nx, 0, pole.ny);
    group.add(meshFromBuffers(THREE, pp, pn, "#94a3b8", 0.95));
  }
}

/**
 * Build the complete procedural facade overlay in local meters (Y-up).
 * `points` is a closed footprint ring in `{x, y}` where `y` maps to world Z.
 * Shared by MapLibre L4 and standalone Three world L5.
 */
export function buildLocalHeroFacadeOverlay(THREE, points, height, hero = {}) {
  if (!Array.isArray(points) || points.length < 4) return null;
  const safeHeight = Math.max(Number(height) || 0, 1);
  const group = new THREE.Group();
  group.name = `hero-facade-overlay:${hero.entityId || hero.id || "unknown"}`;

  const floorH = DEFAULT_FLOOR_HEIGHT;
  const floors = Math.max(1, Math.floor(safeHeight / floorH));
  const bandPos = [];
  const bandNrm = [];
  const centroid = ringCentroid(points);
  // Bands sit outside the opaque shell; the old inward inset made them easy
  // to hide in L5. Keep a small outward offset for stable shared rendering.
  const bandPoints = points.map((point) => {
    const vx = point.x - centroid.x;
    const vy = point.y - centroid.y;
    const len = Math.hypot(vx, vy) || 1;
    return {
      x: point.x + (vx / len) * BAND_INSET,
      y: point.y + (vy / len) * BAND_INSET,
    };
  });
  for (let floor = 0; floor < floors; floor += 1) {
    const z0 = floor * floorH + floorH * 0.28;
    const z1 = Math.min(z0 + floorH * 0.42, safeHeight - 0.2);
    if (z1 <= z0) continue;
    buildWallQuads(bandPoints, z0, z1, bandPos, bandNrm);
  }
  if (bandPos.length) {
    const bands = meshFromBuffers(THREE, bandPos, bandNrm, "#0ea5e9", 0.78);
    bands.name = "facade-window-bands";
    bands.renderOrder = 35;
    group.add(bands);
  }

  addFacadeDecor(THREE, group, points, safeHeight, hero);
  group.traverse((child) => {
    if (child !== group && child.renderOrder === 0) child.renderOrder = 40;
  });
  return group;
}

/**
 * Build a procedural window-band facade group in local meters (Y-up).
 */
export function buildWindowBandsFacade(THREE, maplibregl, hero) {
  const ring = normalizeHeroRing(hero?.coordinates);
  if (!ring.length) return null;
  const height = Math.max(Number(hero?.height) || 0, 1);
  const { originMc, meter, points } = ringToLocalMeters(ring, maplibregl);
  const shellColor = hero?.shellColor || "#94a3b8";
  const shellOpacity = Number(hero?.shellOpacity);
  const opacity = Number.isFinite(shellOpacity)
    ? Math.min(Math.max(shellOpacity, 0.88), 0.98)
    : 0.92;

  const shellPos = [];
  const shellNrm = [];
  buildWallQuads(points, 0, height, shellPos, shellNrm);
  buildRoof(points, height, shellPos, shellNrm);

  const group = new THREE.Group();
  group.name = `hero:${hero.entityId || "unknown"}`;
  group.userData = {
    entityId: String(hero.entityId || ""),
    worldRef: String(hero.worldRef || ""),
    worldEnterable: hero.worldEnterable === true,
    worldEnterViewpoint: hero.worldEnterViewpoint || null,
    worldEnterLabel: hero.worldEnterLabel || null,
    label: hero.label || hero.entityId || "",
  };
  group.add(meshFromBuffers(THREE, shellPos, shellNrm, shellColor, opacity));

  const overlay = buildLocalHeroFacadeOverlay(THREE, points, height, hero);
  if (overlay) group.add(overlay);

  group.userData.originMc = originMc;
  group.userData.meter = meter;
  return group;
}

function fitGltfToFootprint(THREE, scene, points, height) {
  const box = new THREE.Box3().setFromObject(scene);
  const size = new THREE.Vector3();
  box.getSize(size);
  const center = new THREE.Vector3();
  box.getCenter(center);
  scene.position.sub(center);
  scene.position.y += size.y / 2;

  let minX = Infinity;
  let maxX = -Infinity;
  let minY = Infinity;
  let maxY = -Infinity;
  for (const p of points) {
    minX = Math.min(minX, p.x);
    maxX = Math.max(maxX, p.x);
    minY = Math.min(minY, p.y);
    maxY = Math.max(maxY, p.y);
  }
  const targetW = Math.max(maxX - minX, 1);
  const targetD = Math.max(maxY - minY, 1);
  const sx = size.x > 1e-3 ? targetW / size.x : 1;
  const sz = size.z > 1e-3 ? targetD / size.z : 1;
  const sy = size.y > 1e-3 ? height / size.y : 1;
  const scale = Math.min(sx, sz, sy);
  scene.scale.setScalar(scale);
}

/**
 * Try to load glTF; return null on any failure (caller falls back to window_bands).
 */
export async function tryLoadGltfFacade(THREE, maplibregl, hero) {
  const url = String(hero?.modelUrl || "").trim();
  if (!url) return null;
  const ring = normalizeHeroRing(hero?.coordinates);
  if (!ring.length) return null;
  const height = Math.max(Number(hero?.height) || 0, 1);
  const { originMc, meter, points } = ringToLocalMeters(ring, maplibregl);

  try {
    const { GLTFLoader } = await import(
      /* @vite-ignore */ "/workspace-components/vendor/three/GLTFLoader.js"
    );
    const loader = new GLTFLoader();
    const gltf = await new Promise((resolve, reject) => {
      loader.load(url, resolve, undefined, reject);
    });
    const root = gltf?.scene;
    if (!root) return null;
    fitGltfToFootprint(THREE, root, points, height);
    const group = new THREE.Group();
    group.name = `hero-gltf:${hero.entityId || "unknown"}`;
    group.userData = {
      entityId: String(hero.entityId || ""),
      worldRef: String(hero.worldRef || ""),
      worldEnterable: hero.worldEnterable === true,
      worldEnterViewpoint: hero.worldEnterViewpoint || null,
      worldEnterLabel: hero.worldEnterLabel || null,
      label: hero.label || hero.entityId || "",
      originMc,
      meter,
      fromGltf: true,
    };
    group.add(root);
    addFacadeDecor(THREE, group, points, height, hero);
    return group;
  } catch (error) {
    console.warn("[mei-map-heroes] glTF load failed, fallback to window_bands", url, error);
    return null;
  }
}

export async function buildHeroFacade(THREE, maplibregl, hero) {
  const style = String(hero?.style || "window_bands").trim();
  const wantModel =
    style === "gltf" || style === "model" || (style !== "window_bands" && hero?.modelUrl);
  if (wantModel) {
    const gltf = await tryLoadGltfFacade(THREE, maplibregl, hero);
    if (gltf) return gltf;
  }
  return buildWindowBandsFacade(THREE, maplibregl, hero);
}
