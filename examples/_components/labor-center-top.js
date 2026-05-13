import { escapeAttr, parseProps } from "./labor-shared.js";
import "./labor-kpi-strip.js";

class MeiCockpitLaborCenterTop extends HTMLElement {
  connectedCallback() {
    if (!this.shadowRoot) this.attachShadow({ mode: "open" });
    this.render();
  }

  render() {
    const p = parseProps(this);
    const kpiProps = escapeAttr(JSON.stringify({ kpis: p.kpis }));
    this.shadowRoot.innerHTML = `
      <style>
        :host {
          display: block;
          color: var(--mei-color-text-primary, #e0f2fe);
          font-size: var(--mei-font-2, 14px);
          font-family: ui-sans-serif, system-ui, -apple-system, "Segoe UI", Roboto, "PingFang SC", "Microsoft YaHei", sans-serif;
        }
        .wrap {
          display: flex;
          flex-direction: column;
          gap: 12px;
          padding: 12px 14px 16px;
          min-width: 0;
        }
        .cards {
          display: grid;
          grid-template-columns: 1fr 1fr;
          gap: 10px;
        }
        @media (max-width: 900px) {
          .cards { grid-template-columns: 1fr; }
        }
        .stat-card {
          border-radius: 4px;
          padding: 10px 12px;
          border: 1px solid rgba(56,189,248,.15);
          background: rgba(8,47,73,.4);
        }
        .stat-card h4 { margin: 0 0 8px; font-size: var(--mei-font-2, 13px); color: var(--mei-color-text-primary, #bae6fd); font-weight: 600; }
        .stat-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 8px; font-size: var(--mei-font-1, 12px); color: var(--mei-color-text-muted, #94a3b8); }
        .stat-grid strong { display: block; font-size: var(--mei-font-4, 20px); color: var(--mei-color-text-accent, #fde68a); margin-top: 2px; }
        .digits { display: flex; gap: 6px; margin-top: 10px; flex-wrap: wrap; }
        .digit {
          min-width: 34px;
          text-align: center;
          padding: 6px 4px;
          border-radius: 4px;
          font-size: var(--mei-font-3, 18px);
          font-weight: 800;
          color: #f0f9ff;
          background: rgba(15,23,42,.85);
          border: 1px solid rgba(56,189,248,.2);
        }
      </style>
      <div class="wrap">
        <mei-cockpit-labor-kpi-strip data-props="${kpiProps}"></mei-cockpit-labor-kpi-strip>
        <div class="cards">
          <div class="stat-card">
            <h4>本年工人劳动合同签订人数</h4>
            <div class="stat-grid">
              <div>累计代发农民工工资金额：<strong>258.12</strong><span style="font-size:11px;color:#64748b"> 万元</span></div>
              <div>累计代发农民工工资金额：<strong>258.12</strong><span style="font-size:11px;color:#64748b"> 万元</span></div>
            </div>
            <div class="digits"><span class="digit">2</span><span class="digit">3</span><span class="digit">5</span><span class="digit">1</span></div>
          </div>
          <div class="stat-card">
            <h4>本年工人劳动合同签订人数</h4>
            <div class="stat-grid">
              <div>累计代发农民工工资金额：<strong>258.12</strong><span style="font-size:11px;color:#64748b"> 万元</span></div>
              <div>累计代发农民工工资金额：<strong>258.12</strong><span style="font-size:11px;color:#64748b"> 万元</span></div>
            </div>
            <div class="digits"><span class="digit">3</span><span class="digit">5</span><span class="digit">5</span><span class="digit">8</span></div>
          </div>
        </div>
      </div>
    `;
  }
}

if (!customElements.get("mei-cockpit-labor-center-top")) {
  customElements.define("mei-cockpit-labor-center-top", MeiCockpitLaborCenterTop);
}
