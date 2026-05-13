import { escapeHtml, parseProps, rowsOf } from "./labor-shared.js";

class MeiCockpitLaborRankingList extends HTMLElement {
  connectedCallback() {
    if (!this.shadowRoot) this.attachShadow({ mode: "open" });
    const p = parseProps(this);
    const rows = rowsOf(p.dataset);
    const headers = Array.isArray(p.headers) ? p.headers : ["字段名称一", "字段名称一", "字段名称一"];
    const n = Math.min(3, Math.max(1, headers.length));
    const gridTpl =
      n === 1 ? "1fr" : n === 2 ? "1.4fr 1fr" : "1.2fr 0.8fr 0.7fr";
    const headCells = headers
      .slice(0, 3)
      .map((h) => `<span>${escapeHtml(h)}</span>`)
      .join("");
    const body = rows
      .map((row) => {
        const pct = Math.min(100, Math.max(0, Number(row.bar_pct) || 0));
        const cells =
          n >= 3
            ? `<span>${escapeHtml(row.col_a)}</span><span>${escapeHtml(row.col_b)}</span><span>${escapeHtml(row.col_c)}</span>`
            : `<span>${escapeHtml(row.col_a)}</span><span>${escapeHtml(row.col_b)}</span>`;
        return `
          <div class="rank-row">
            <div class="rank-badge">${escapeHtml(row.rank)}</div>
            <div class="rank-cols">
              <div class="rank-line" style="grid-template-columns:${gridTpl}">${cells}</div>
              <div class="rank-bar"><span style="width:${pct}%"></span></div>
            </div>
          </div>`;
      })
      .join("");
    const headPad = "44px";
    this.shadowRoot.innerHTML = `
      <style>
        :host { display: block; color: #e0f2fe; }
        .rank-head {
          display: grid;
          padding: 0 0 6px ${headPad};
          font-size: 12px;
          color: #7dd3fc;
          font-weight: 600;
        }
        .rank-head.rank-line { margin-bottom: 2px; }
        .rank-row { display: flex; gap: 8px; padding: 8px 0; border-bottom: 1px solid rgba(51,65,85,.5); }
        .rank-row:last-child { border-bottom: none; }
        .rank-badge {
          flex: 0 0 36px; height: 36px; border-radius: 50%;
          display: grid; place-items: center; font-size: 12px; font-weight: 800;
          background: radial-gradient(circle at 30% 25%, rgba(125,211,252,.35), rgba(8,47,73,.9));
          border: 1px solid rgba(56,189,248,.35); color: #f0f9ff;
        }
        .rank-cols { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 6px; }
        .rank-line {
          display: grid; gap: 6px; font-size: 12px; color: #cbd5e1;
        }
        .rank-bar { height: 5px; border-radius: 3px; background: rgba(30,41,59,.9); overflow: hidden; }
        .rank-bar > span {
          display: block; height: 100%; border-radius: 3px;
          background: linear-gradient(90deg, #0ea5e9, #22d3ee);
        }
      </style>
      <div class="rank-head rank-line" style="grid-template-columns:${gridTpl}">${headCells}</div>
      ${body}
    `;
  }
}

if (!customElements.get("mei-cockpit-labor-ranking-list")) {
  customElements.define("mei-cockpit-labor-ranking-list", MeiCockpitLaborRankingList);
}
