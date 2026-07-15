import "./step-bridge.js";
import "./affordance-bar.js";
import "./board-grid.js";
import "./outcome-hud.js";
import { escapeHtml, parseProps } from "./runtime.js";

class MeiSimScene extends HTMLElement {
  connectedCallback() {
    this.props = parseProps(this);
    if (!this.shadowRoot) {
      this.attachShadow({ mode: "open" });
    }
    this.render();
  }

  render() {
    const encodedProps = escapeHtml(JSON.stringify(this.props || {}));
    this.shadowRoot.innerHTML = `
      <style>
        :host { display: block; }
        .wrap { display: grid; gap: 12px; padding: 16px; border-radius: 16px; background: rgba(15,23,42,.78); border: 1px solid rgba(148,163,184,.18); }
        .cols { display: grid; gap: 12px; grid-template-columns: minmax(0, 2fr) minmax(280px, 1fr); align-items: start; }
        .side { display: grid; gap: 12px; }
        @media (max-width: 980px) {
          .cols { grid-template-columns: 1fr; }
        }
      </style>
      <section class="wrap">
        <mei-sim-step-bridge data-props="${encodedProps}"></mei-sim-step-bridge>
        <mei-sim-outcome-hud data-props="${encodedProps}"></mei-sim-outcome-hud>
        <mei-sim-affordance-bar data-props="${encodedProps}"></mei-sim-affordance-bar>
        <mei-sim-board-grid data-props="${encodedProps}"></mei-sim-board-grid>
      </section>
    `;
  }
}

if (!customElements.get("mei-sim-scene")) {
  customElements.define("mei-sim-scene", MeiSimScene);
}
