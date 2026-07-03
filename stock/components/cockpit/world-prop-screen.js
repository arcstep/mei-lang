import * as THREE from "../vendor/three/three.module.min.js";

const TAG = "mei-world-prop-screen";

export function parsePropHexColor(hex, fallback = 0x38bdf8) {
  const raw = String(hex || "").trim().replace("#", "");
  if (!raw) return fallback;
  const expanded =
    raw.length === 3 ? raw.split("").map((c) => c + c).join("") : raw;
  const parsed = Number.parseInt(expanded, 16);
  return Number.isFinite(parsed) ? parsed : fallback;
}

export function createWorldPropScreenMesh(props = {}) {
  const label = String(props.label || "导览屏").trim();
  const width = Number(props.width ?? 1.2);
  const height = Number(props.height ?? 0.72);
  const color = parsePropHexColor(props.color, 0x38bdf8);
  const mesh = new THREE.Mesh(
    new THREE.PlaneGeometry(width, height),
    new THREE.MeshStandardMaterial({
      color,
      emissive: parsePropHexColor(props.emissive, 0x0ea5e9),
      emissiveIntensity: Number(props.emissiveIntensity ?? 0.45),
      side: THREE.DoubleSide,
    }),
  );
  mesh.userData.propLabel = label;
  mesh.userData.device = true;
  mesh.userData.propComponent = TAG;
  return mesh;
}

class MeiWorldPropScreen extends HTMLElement {
  connectedCallback() {
    if (!this.shadowRoot) {
      this.attachShadow({ mode: "open" });
    }
    const label = String(this.getAttribute("label") || "导览屏").trim();
    const decal = String(this.getAttribute("decal") || "").trim();
    this.shadowRoot.innerHTML = `
      <style>
        :host {
          display: inline-flex;
          align-items: center;
          justify-content: center;
          min-width: 96px;
          min-height: 56px;
          padding: 8px 12px;
          border-radius: 6px;
          background: linear-gradient(145deg, #0c4a6e, #0369a1);
          color: #e0f2fe;
          font: 12px/1.3 system-ui, sans-serif;
          box-shadow: 0 0 12px rgba(14, 165, 233, 0.35);
        }
      </style>
      <span>${label}${decal ? ` · ${decal}` : ""}</span>
    `;
  }
}

if (!customElements.get(TAG)) {
  customElements.define(TAG, MeiWorldPropScreen);
}
