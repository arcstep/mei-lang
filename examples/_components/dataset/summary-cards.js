class MeiDatasetSummaryCards extends HTMLElement {
  connectedCallback() {
    const props = parseProps(this);
    const scalarMetric = resolveScalarMetric(props);
    const dataset = props.data?.rows ? props.data : props.dataset?.dataset || props.dataset || {};
    const rows = Array.isArray(dataset.rows) ? dataset.rows : [];
    const metrics = scalarMetric || normalizeMetrics(props.metrics, dataset.columns || [], rows);
    this.attachShadow({ mode: "open" });
    this.shadowRoot.innerHTML = `
      <style>
        :host { display: block; }
        .grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(140px, 1fr)); gap: 12px; }
        .card { display: grid; gap: 6px; padding: 12px; border-radius: 14px; background: linear-gradient(150deg, rgba(15,23,42,.92), rgba(30,41,59,.75)); border: 1px solid rgba(59,130,246,.25); }
        .label { color: #94a3b8; font-size: 12px; }
        .value { color: #f8fafc; font-size: 24px; font-weight: 800; line-height: 1.1; }
        .hint { color: #67e8f9; font-size: 12px; }
      </style>
      <section class="grid">
        ${metrics.map((metric) => `
          <article class="card">
            <div class="label">${escapeHtml(metric.label)}</div>
            <div class="value">${escapeHtml(metric.value)}</div>
            <div class="hint">${escapeHtml(metric.hint || "")}</div>
          </article>
        `).join("")}
      </section>
    `;
  }
}

function resolveScalarMetric(props) {
  const metric = props.value || props.data || null;
  if (!metric || metric.shape !== "scalar" || typeof metric.value !== "object" || metric.value === null) {
    return null;
  }
  const schema = Array.isArray(metric.schema) ? metric.schema : [];
  const units = new Map(
    schema
      .filter((column) => column && typeof column.name === "string")
      .map((column) => [column.name, column.unit || ""])
  );
  return Object.entries(metric.value).map(([key, rawValue]) => {
    const unit = units.get(key) || "";
    const value = formatMetricValue(rawValue);
    return {
      label: key,
      value: `${value}${unit}`.trim(),
      hint: metric.id || "scalar_map",
    };
  });
}

function normalizeMetrics(rawMetrics, columns, rows) {
  if (Array.isArray(rawMetrics) && rawMetrics.length > 0) {
    return rawMetrics.map((metric) => evaluateMetric(metric, rows));
  }
  const fallbackMetrics = [{ label: "总行数", op: "count", column: null }];
  const numericColumn = firstNumericColumn(columns, rows);
  if (numericColumn) {
    fallbackMetrics.push({ label: "数值总和", op: "sum", column: numericColumn });
  }
  return fallbackMetrics.map((metric) => evaluateMetric(metric, rows));
}

function firstNumericColumn(columns, rows) {
  return (columns || []).find((column) =>
    rows.some((row) => Number.isFinite(Number(row?.[column])))
  );
}

function evaluateMetric(metric, rows) {
  const label = metric?.label || "指标";
  const column = metric?.column ?? null;
  const op = metric?.op || "count";
  const unit = metric?.unit || "";
  const values = column
    ? rows
        .map((row) => Number(row?.[column]))
        .filter((value) => Number.isFinite(value))
    : [];
  let numericResult = 0;
  if (op === "sum") {
    numericResult = values.reduce((sum, value) => sum + value, 0);
  } else if (op === "avg") {
    numericResult = values.length === 0 ? 0 : values.reduce((sum, value) => sum + value, 0) / values.length;
  } else if (op === "max") {
    numericResult = values.length === 0 ? 0 : Math.max(...values);
  } else if (op === "min") {
    numericResult = values.length === 0 ? 0 : Math.min(...values);
  } else {
    numericResult = rows.length;
  }
  const value = op === "avg" ? numericResult.toFixed(1) : String(Math.round(numericResult));
  return {
    label,
    value: `${value}${unit}`.trim(),
    hint: column ? `${op}(${column})` : "count(rows)",
  };
}

function formatMetricValue(value) {
  const numeric = Number(value);
  if (Number.isFinite(numeric)) {
    return Number.isInteger(numeric) ? String(numeric) : numeric.toFixed(2);
  }
  return String(value ?? "");
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

customElements.define("mei-dataset-summary-cards", MeiDatasetSummaryCards);
