class MeiChartBarMini extends HTMLElement {
  connectedCallback() {
    const props = parseProps(this);
    const dataset = props.dataset?.dataset || props.dataset || {};
    const rows = Array.isArray(dataset.rows) ? dataset.rows : [];
    const columns = Array.isArray(dataset.columns) ? dataset.columns : [];
    const labelField = props.labelField || columns[0] || "label";
    const valueField = props.valueField || columns[1] || "value";
    const points = rows
      .map((row) => ({
        label: String(row?.[labelField] ?? ""),
        rawValue: Number(row?.[valueField]),
      }))
      .filter((item) => item.label && Number.isFinite(item.rawValue));
    const maxValue = points.length === 0 ? 1 : Math.max(...points.map((item) => item.rawValue), 1);
    this.attachShadow({ mode: "open" });
    this.shadowRoot.innerHTML = `
      <style>
        :host { display: block; }
        .wrap { display: grid; gap: 12px; padding: 16px; border-radius: 14px; background: rgba(15,23,42,.72); border: 1px solid rgba(148,163,184,.18); color: #e2e8f0; }
        .head { display: flex; justify-content: space-between; gap: 12px; align-items: baseline; flex-wrap: wrap; }
        .title { margin: 0; font-size: 14px; color: #f8fafc; }
        .meta { color: #94a3b8; font-size: 12px; }
        .rows { display: grid; gap: 10px; }
        .row { display: grid; gap: 6px; }
        .label { font-size: 12px; color: #cbd5e1; display: flex; justify-content: space-between; gap: 8px; }
        .bar-track { height: 10px; border-radius: 999px; background: rgba(148,163,184,.16); overflow: hidden; }
        .bar-fill { height: 100%; border-radius: 999px; background: linear-gradient(90deg, #38bdf8, #6366f1); }
        .empty { color: #94a3b8; font-size: 12px; }
      </style>
      <section class="wrap">
        <div class="head">
          <h4 class="title">${escapeHtml(props.title || "最小图表示例")}</h4>
          <span class="meta">${escapeHtml(`${labelField} -> ${valueField}`)}</span>
        </div>
        <div class="rows">
          ${renderRows(points, maxValue)}
        </div>
      </section>
    `;
  }
}

function renderRows(points, maxValue) {
  if (points.length === 0) {
    return `<p class="empty">缺少可绘制数据，请检查 labelField / valueField。</p>`;
  }
  return points
    .map((point) => {
      const width = Math.max(6, Math.round((point.rawValue / maxValue) * 100));
      return `
        <div class="row">
          <div class="label">
            <span>${escapeHtml(point.label)}</span>
            <span>${escapeHtml(String(point.rawValue))}</span>
          </div>
          <div class="bar-track">
            <div class="bar-fill" style="width:${width}%;"></div>
          </div>
        </div>
      `;
    })
    .join("");
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

customElements.define("mei-chart-bar-mini", MeiChartBarMini);
