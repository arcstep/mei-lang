class MeiCockpitMetricCards extends HTMLElement {
  connectedCallback() {
    const props = parseProps(this);
    const dataset = props.dataset || {};
    const rows = dataset.rows || [];
    this.attachShadow({ mode: "open" });
    this.shadowRoot.innerHTML = `
      <style>
        :host { display: block; }
        .grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(160px, 1fr)); gap: 12px; }
        .card { display: grid; gap: 8px; padding: 14px; border-radius: 16px; background: linear-gradient(135deg, rgba(15,23,42,.92), rgba(30,41,59,.74)); border: 1px solid rgba(59,130,246,.22); }
        .label { color: #94a3b8; font-size: 12px; }
        .value { color: #f8fafc; font-size: 28px; font-weight: 800; }
        .unit { color: #67e8f9; font-size: 12px; }
      </style>
      <div class="grid">
        ${rows.map((row) => `
          <article class="card">
            <div class="label">${escapeHtml(row.label || row.metric || "指标")}</div>
            <div class="value">${escapeHtml(row.value || "--")}</div>
            <div class="unit">${escapeHtml(row.unit || "")}</div>
          </article>
        `).join("")}
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

customElements.define("mei-cockpit-metric-cards", MeiCockpitMetricCards);
