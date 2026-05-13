import { escapeAttr, parseProps } from "./labor-shared.js";
import "./labor-header-weather.js";
import "./labor-header-title.js";
import "./labor-header-clock.js";

class MeiCockpitLaborHeaderBar extends HTMLElement {
  connectedCallback() {
    if (!this.shadowRoot) this.attachShadow({ mode: "open" });
    this.render();
  }

  render() {
    const p = parseProps(this);
    const weatherProps = escapeAttr(
      JSON.stringify({
        temp: p.temp || "28°C",
        sky: p.sky || "多云",
      }),
    );
    const titleProps = escapeAttr(
      JSON.stringify({
        title: p.title || "这是标题可视化大屏",
        titleBandWidth: p.titleBandWidth,
        titleBandMinWidth: p.titleBandMinWidth,
        stripPad: p.stripPad,
      }),
    );
    const clockProps = escapeAttr(
      JSON.stringify({
        time: p.time,
        weekday: p.weekday,
        date: p.date,
      }),
    );

    this.shadowRoot.innerHTML = `
      <style>
        :host {
          display: block;
          width: 100%;
          min-width: 0;
          min-height: 92px;
          overflow: visible;
        }
        .bar {
          position: relative;
          width: 100%;
          min-width: 0;
          min-height: 92px;
          display: grid;
          grid-template-columns: minmax(180px, 1fr) auto minmax(220px, 1fr);
          align-items: start;
          gap: 0;
          overflow: visible;
        }
        .bar::before {
          content: "";
          position: absolute;
          left: 0;
          right: 0;
          top: 2px;
          height: 56px;
          pointer-events: none;
          background:
            linear-gradient(180deg, rgba(222, 234, 248, 0.12) 0%, rgba(119, 160, 209, 0.10) 24%, rgba(9, 31, 58, 0.76) 100%);
          box-shadow:
            inset 0 -1px 0 rgba(94, 212, 255, 0.14),
            0 8px 24px rgba(2, 8, 23, 0.16);
        }
        .slot {
          position: relative;
          z-index: 1;
          min-width: 0;
        }
        .slot-weather {
          justify-self: stretch;
          padding-top: 10px;
        }
        .slot-title {
          justify-self: center;
          z-index: 2;
          padding-top: 6px;
        }
        .slot-clock {
          justify-self: stretch;
          padding-top: 10px;
        }
        @media (max-width: 900px) {
          .bar {
            grid-template-columns: 1fr;
            gap: 8px;
            justify-items: center;
          }
        }
      </style>
      <div class="bar">
        <div class="slot slot-weather">
          <mei-cockpit-labor-header-weather data-props="${weatherProps}"></mei-cockpit-labor-header-weather>
        </div>
        <div class="slot slot-title">
          <mei-cockpit-labor-header-title data-props="${titleProps}"></mei-cockpit-labor-header-title>
        </div>
        <div class="slot slot-clock">
          <mei-cockpit-labor-header-clock data-props="${clockProps}"></mei-cockpit-labor-header-clock>
        </div>
      </div>
    `;
  }
}

if (!customElements.get("mei-cockpit-labor-header-bar")) {
  customElements.define("mei-cockpit-labor-header-bar", MeiCockpitLaborHeaderBar);
}
