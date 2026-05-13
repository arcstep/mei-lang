import { escapeHtml, LABOR_FIGMA_ASSETS, parseProps } from "./labor-shared.js";

const A = LABOR_FIGMA_ASSETS;

class MeiCockpitLaborHeaderWeather extends HTMLElement {
  connectedCallback() {
    if (!this.shadowRoot) this.attachShadow({ mode: "open" });
    const p = parseProps(this);
    const temp = p.temp || "28°C";
    const sky = p.sky || "多云";
    this.shadowRoot.innerHTML = `
      <style>
        :host { display: block; justify-self: start; align-self: center; min-width: 0; }
        .wrap {
          display: inline-flex;
          align-items: center;
          gap: 10px;
          min-width: 0;
          color: #e3f4fc;
        }
        .ico {
          width: 46px;
          height: 46px;
          flex: 0 0 auto;
          object-fit: contain;
          filter: drop-shadow(0 0 8px rgba(0, 145, 255, 0.45));
        }
        .line1 {
          font-family: "DIN Alternate", "DINPro", "Barlow Condensed", "Arial Narrow", Arial, sans-serif;
          font-size: 20px;
          font-weight: 700;
          line-height: 1.1;
          letter-spacing: 1px;
        }
        .line2 {
          font-size: 12px;
          line-height: 1.2;
          opacity: 0.6;
          letter-spacing: 1px;
        }
      </style>
      <div class="wrap">
        <img class="ico" src="${A}/labor-weather-icon.svg" alt="" />
        <div>
          <div class="line1">${escapeHtml(temp)}</div>
          <div class="line2">${escapeHtml(sky)}</div>
        </div>
      </div>
    `;
  }
}

if (!customElements.get("mei-cockpit-labor-header-weather")) {
  customElements.define("mei-cockpit-labor-header-weather", MeiCockpitLaborHeaderWeather);
}
