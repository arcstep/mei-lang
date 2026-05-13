import { escapeHtml, parseProps, rowsOf } from "./labor-shared.js";

class MeiCockpitLaborSubcontractTable extends HTMLElement {
  connectedCallback() {
    if (!this.shadowRoot) this.attachShadow({ mode: "open" });
    const p = parseProps(this);
    const rows = rowsOf(p.dataset);
    const h1 = p.col1 || "劳务分包";
    const h2 = p.col2 || "公司名称";
    const h3 = p.col3 || "金额";
    const body = rows
      .map(
        (row) => `
        <tr>
          <td>${escapeHtml(row.category)}</td>
          <td>${escapeHtml(row.company)}</td>
          <td class="num">${escapeHtml(row.amount)}</td>
        </tr>`,
      )
      .join("");
    this.shadowRoot.innerHTML = `
      <style>
        :host { display: block; }
        .labor-table-wrap { max-height: 280px; overflow: auto; border-radius: 4px; }
        table.labor-table {
          width: 100%; border-collapse: collapse; font-size: 12px;
          color: #e0f2fe;
        }
        .labor-table th, .labor-table td {
          padding: 8px 6px; text-align: left; border-bottom: 1px solid rgba(148,163,184,.12);
        }
        .labor-table th {
          color: #7dd3fc; font-weight: 600; position: sticky; top: 0;
          background: rgba(8,47,73,.95);
        }
        .labor-table td.num { color: #fde68a; font-weight: 700; text-align: right; white-space: nowrap; }
      </style>
      <div class="labor-table-wrap">
        <table class="labor-table">
          <thead><tr><th>${escapeHtml(h1)}</th><th>${escapeHtml(h2)}</th><th>${escapeHtml(h3)}</th></tr></thead>
          <tbody>${body}</tbody>
        </table>
      </div>
    `;
  }
}

if (!customElements.get("mei-cockpit-labor-subcontract-table")) {
  customElements.define("mei-cockpit-labor-subcontract-table", MeiCockpitLaborSubcontractTable);
}
