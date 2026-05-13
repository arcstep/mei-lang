import { escapeAttr, parseProps } from "./shared.js";
import "./donut-row.js";

class MeiCockpitDonutSummary extends HTMLElement {
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
          color: var(--mei-color-text-primary, #e0f2fe);
          font-size: var(--mei-font-2, 14px);
          font-family: ui-sans-serif, system-ui, -apple-system, "Segoe UI", Roboto, "PingFang SC", "Microsoft YaHei", sans-serif;
        }
        .wrap { padding: 6px 12px 14px; }
        .foot-metrics {
          display: grid;
          grid-template-columns: repeat(3, 1fr);
          gap: 8px;
          font-size: var(--mei-font-1, 11px);
          color: var(--mei-color-text-muted, #94a3b8);
          margin-top: 10px;
        }
        .foot-metrics div { text-align: center; padding: 6px; border-radius: 4px; background: rgba(15,23,42,.5); }
        .foot-metrics strong { display: block; color: var(--mei-color-text-accent, #fde68a); margin-top: 4px; font-size: var(--mei-font-2, 13px); }
      </style>
      <div class="wrap">
        <mei-cockpit-donut-row data-props="${donutProps}"></mei-cockpit-donut-row>
        <div class="foot-metrics">
          <div>累计<strong>3542人次</strong></div>
          <div>本年<strong>3542人次</strong></div>
          <div>7月<strong>340人次</strong></div>
        </div>
      </div>
    `;
  }
}

if (!customElements.get("mei-cockpit-donut-summary")) {
  customElements.define("mei-cockpit-donut-summary", MeiCockpitDonutSummary);
}
