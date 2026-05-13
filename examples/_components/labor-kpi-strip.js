import { escapeHtml, parseProps, rowsOf } from "./labor-shared.js";

class MeiCockpitLaborKpiStrip extends HTMLElement {
  connectedCallback() {
    if (!this.shadowRoot) this.attachShadow({ mode: "open" });
    const p = parseProps(this);
    const rows = rowsOf(p.kpis ?? p.dataset);
    const body = rows
      .map(
        (row) => `
        <div class="kpi-card">
          <div class="kpi-icon"></div>
          <div class="kpi-body">
            <div class="kpi-value">${escapeHtml(row.value)}<span class="kpi-unit">${escapeHtml(row.unit || "")}</span></div>
            <div class="kpi-label">${escapeHtml(row.label)}</div>
          </div>
        </div>`,
      )
      .join("");
    this.shadowRoot.innerHTML = `
      <style>
        :host { display: block; }
        .kpi-row { display: flex; flex-wrap: wrap; gap: 10px; justify-content: center; }
        .kpi-card {
          flex: 1 1 160px; max-width: 200px;
          display: flex; gap: 10px; align-items: center;
          padding: 10px 12px; border-radius: 6px;
          background: linear-gradient(135deg, rgba(8,47,73,.75), rgba(15,23,42,.65));
          border: 1px solid rgba(56,189,248,.2);
          color: #e0f2fe;
        }
        .kpi-icon {
          width: 44px; height: 44px; border-radius: 8px;
          background: conic-gradient(from 200deg, #0ea5e9, #22d3ee, #0369a1, #0ea5e9);
          opacity: .9;
        }
        .kpi-value { font-size: 24px; font-weight: 900; color: #f0f9ff; font-variant-numeric: tabular-nums; }
        .kpi-unit { font-size: 12px; font-weight: 600; color: #7dd3fc; margin-left: 4px; }
        .kpi-label { font-size: 12px; color: #94a3b8; margin-top: 2px; }
      </style>
      <div class="kpi-row">${body}</div>
    `;
  }
}

if (!customElements.get("mei-cockpit-labor-kpi-strip")) {
  customElements.define("mei-cockpit-labor-kpi-strip", MeiCockpitLaborKpiStrip);
}
