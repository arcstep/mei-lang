import { escapeHtml, parseProps, rowsOf } from "./labor-shared.js";

class MeiCockpitLaborPersonCards extends HTMLElement {
  connectedCallback() {
    if (!this.shadowRoot) this.attachShadow({ mode: "open" });
    const p = parseProps(this);
    const rows = rowsOf(p.dataset);
    const monthLabel = p.monthLabel || "7月";
    const body = rows
      .map(
        (row) => `
        <div class="person-card">
          <div class="person-avatar"></div>
          <div class="person-main">
            <div class="person-name">${escapeHtml(row.name)}</div>
            <div class="person-line">${escapeHtml(monthLabel)}出勤天数：<strong>${escapeHtml(row.days)}</strong> 天</div>
            <div class="person-co">${escapeHtml(row.company)}</div>
          </div>
          <div class="person-pay">
            <div class="pay-block"><span class="muted">${escapeHtml(monthLabel)}</span><strong>${escapeHtml(row.month_amt)}</strong></div>
            <div class="pay-block"><span class="muted">累计</span><strong>${escapeHtml(row.total_amt)}</strong></div>
          </div>
        </div>`,
      )
      .join("");
    this.shadowRoot.innerHTML = `
      <style>
        :host { display: block; }
        .person-card {
          display: grid;
          grid-template-columns: 56px 1fr minmax(120px, 34%);
          gap: 10px; align-items: stretch;
          padding: 10px; margin-bottom: 8px; border-radius: 4px;
          background: rgba(15,23,42,.55);
          border: 1px solid rgba(56,189,248,.12);
          color: #e0f2fe;
        }
        .person-card:last-child { margin-bottom: 0; }
        .person-avatar {
          border-radius: 4px;
          background: linear-gradient(145deg, rgba(56,189,248,.35), rgba(30,58,138,.5));
          border: 1px solid rgba(125,211,252,.25);
        }
        .person-name { font-weight: 700; font-size: 14px; color: #f8fafc; }
        .person-line { font-size: 12px; color: #94a3b8; margin-top: 4px; }
        .person-co { font-size: 11px; color: #64748b; margin-top: 4px; line-height: 1.35; }
        .person-pay { display: grid; grid-template-columns: 1fr 1fr; gap: 6px; align-content: center; }
        .pay-block {
          background: rgba(8,47,73,.55); border-radius: 4px; padding: 6px 8px; text-align: center;
          border: 1px solid rgba(45,212,191,.12);
        }
        .pay-block .muted { display: block; font-size: 11px; color: #94a3b8; margin-bottom: 2px; }
        .pay-block strong { font-size: 13px; color: #fde68a; }
      </style>
      ${body}
    `;
  }
}

if (!customElements.get("mei-cockpit-labor-person-cards")) {
  customElements.define("mei-cockpit-labor-person-cards", MeiCockpitLaborPersonCards);
}
