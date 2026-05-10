class MeiDatasetTable extends HTMLElement {
  connectedCallback() {
    const props = parseProps(this);
    const dataset = props.dataset?.dataset || props.dataset || {};
    const columns = dataset.columns || [];
    const rows = dataset.rows || [];
    this.attachShadow({ mode: "open" });
    this.shadowRoot.innerHTML = `
      <style>
        :host { display: block; }
        .wrap { display: grid; gap: 12px; padding: 16px; border-radius: 14px; background: rgba(15,23,42,.72); border: 1px solid rgba(148,163,184,.18); color: #e2e8f0; }
        .meta { display: flex; justify-content: space-between; gap: 12px; flex-wrap: wrap; color: #94a3b8; font-size: 12px; }
        .table-wrap { overflow: auto; border-radius: 12px; border: 1px solid rgba(148,163,184,.16); }
        table { width: 100%; border-collapse: collapse; min-width: 560px; }
        th, td { padding: 10px 12px; text-align: left; border-bottom: 1px solid rgba(148,163,184,.12); font-size: 12px; vertical-align: top; }
        th { background: rgba(30,41,59,.92); color: #f8fafc; position: sticky; top: 0; }
        td { color: #cbd5e1; }
      </style>
      <div class="wrap">
        <div class="meta">
          <strong>${escapeHtml(dataset.title || dataset.id || "Dataset")}</strong>
          <span>${rows.length} rows</span>
        </div>
        <div class="table-wrap">
          <table>
            <thead>
              <tr>${columns.map((column) => `<th>${escapeHtml(column)}</th>`).join("")}</tr>
            </thead>
            <tbody>
              ${rows.map((row) => `<tr>${columns.map((column) => `<td>${escapeHtml(row[column] ?? "")}</td>`).join("")}</tr>`).join("")}
            </tbody>
          </table>
        </div>
      </div>
    `;
  }
}

function parseProps(element) {
  try {
    return JSON.parse(element.dataset.props || "{}");
  } catch {
    return {};
  }
}

function escapeHtml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;");
}

customElements.define("mei-dataset-table", MeiDatasetTable);
