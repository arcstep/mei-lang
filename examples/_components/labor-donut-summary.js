import { escapeAttr, parseProps } from "./labor-shared.js";
import "./labor-donut-row.js";

class MeiCockpitLaborDonutSummary extends HTMLElement {
  connectedCallback() {
    if (!this.shadowRoot) this.attachShadow({ mode: "open" });
    this.render();
  }

  render() {
    const p = parseProps(this);
    const donutProps = escapeAttr(
      JSON.stringify({
        donutVisit: p.donutVisit,
        donutStd: p.donutStd,
      }),
    );
    this.shadowRoot.innerHTML = `
      <style>
        :host {
          display: block;
          color: #e0f2fe;
          font-family: ui-sans-serif, system-ui, -apple-system, "Segoe UI", Roboto, "PingFang SC", "Microsoft YaHei", sans-serif;
        }
        .wrap { padding: 6px 12px 14px; }
        .foot-metrics {
          display: grid;
          grid-template-columns: repeat(3, 1fr);
          gap: 8px;
          font-size: 11px;
          color: #94a3b8;
          margin-top: 10px;
        }
        .foot-metrics div { text-align: center; padding: 6px; border-radius: 4px; background: rgba(15,23,42,.5); }
        .foot-metrics strong { display: block; color: #fde68a; margin-top: 4px; font-size: 13px; }
      </style>
      <div class="wrap">
        <mei-cockpit-labor-donut-row data-props="${donutProps}"></mei-cockpit-labor-donut-row>
        <div class="foot-metrics">
          <div>累计<strong>3542人次</strong></div>
          <div>本年<strong>3542人次</strong></div>
          <div>7月<strong>340人次</strong></div>
        </div>
      </div>
    `;
  }
}

if (!customElements.get("mei-cockpit-labor-donut-summary")) {
  customElements.define("mei-cockpit-labor-donut-summary", MeiCockpitLaborDonutSummary);
}
