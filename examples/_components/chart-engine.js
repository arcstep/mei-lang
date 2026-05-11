const ECHARTS_CDN = "https://cdn.jsdelivr.net/npm/echarts@5/dist/echarts.min.js";
const CARTESIAN_KINDS = new Set(["line", "area", "trend", "column", "bar", "scatter"]);
const PIE_KINDS = new Set(["pie", "donut", "rose"]);

let echartsPromise = null;

export function defineChartElement(tagName, chartKind, defaultTitle) {
  if (customElements.get(tagName)) {
    return;
  }
  class MeiChartElement extends HTMLElement {
    connectedCallback() {
      this.attachShadow({ mode: "open" });
      this.shadowRoot.innerHTML = chartShellHtml(defaultTitle);
      this.chartEl = this.shadowRoot.querySelector(".chart");
      this.metaEl = this.shadowRoot.querySelector(".meta");
      this.errorEl = this.shadowRoot.querySelector(".error");
      this.refresh = () => this.renderChart();
      window.addEventListener("meilang:preview-updated", this.refresh);
      this.resizeObserver = new ResizeObserver(() => {
        if (this.chart) {
          this.chart.resize();
        }
      });
      this.resizeObserver.observe(this);
      this.renderChart();
    }

    disconnectedCallback() {
      window.removeEventListener("meilang:preview-updated", this.refresh);
      if (this.resizeObserver) {
        this.resizeObserver.disconnect();
      }
      if (this.chart) {
        this.chart.dispose();
        this.chart = null;
      }
    }

    async renderChart() {
      const props = parseProps(this);
      const diagnostics = [];
      const model = buildChartModel(chartKind, props, diagnostics);
      this.metaEl.textContent = model.meta;
      if (diagnostics.length > 0) {
        this.errorEl.textContent = diagnostics.join(" | ");
      } else {
        this.errorEl.textContent = "";
      }
      try {
        const echarts = await ensureECharts();
        if (!this.chart) {
          this.chart = echarts.init(this.chartEl);
        }
        this.chart.setOption(model.option, true);
      } catch (error) {
        this.errorEl.textContent = "图表引擎加载失败: " + String(error?.message || error);
      }
    }
  }
  customElements.define(tagName, MeiChartElement);
}

function chartShellHtml(defaultTitle) {
  return `
    <style>
      :host { display: block; }
      .wrap {
        display: grid;
        gap: 8px;
        padding: 14px;
        border-radius: 14px;
        border: 1px solid rgba(148,163,184,.2);
        background: rgba(15,23,42,.64);
      }
      .head {
        display: flex;
        justify-content: space-between;
        gap: 8px;
        align-items: baseline;
        color: #e2e8f0;
      }
      .title { margin: 0; font-size: 14px; color: #f8fafc; }
      .meta { font-size: 12px; color: #94a3b8; }
      .chart { width: 100%; min-height: 260px; }
      .error { min-height: 18px; font-size: 12px; color: #fca5a5; }
    </style>
    <section class="wrap">
      <div class="head">
        <h4 class="title">${escapeHtml(defaultTitle)}</h4>
        <span class="meta"></span>
      </div>
      <div class="chart"></div>
      <div class="error"></div>
    </section>
  `;
}

async function ensureECharts() {
  if (window.echarts) {
    return window.echarts;
  }
  if (!echartsPromise) {
    echartsPromise = new Promise((resolve, reject) => {
      const existing = document.querySelector(`script[data-mei-echarts="true"]`);
      if (existing) {
        existing.addEventListener("load", () => resolve(window.echarts));
        existing.addEventListener("error", () => reject(new Error("echarts script error")));
        return;
      }
      const script = document.createElement("script");
      script.src = ECHARTS_CDN;
      script.async = true;
      script.dataset.meiEcharts = "true";
      script.onload = () => resolve(window.echarts);
      script.onerror = () => reject(new Error("echarts script load failed"));
      document.head.appendChild(script);
    });
  }
  return echartsPromise;
}

function buildChartModel(kind, props, diagnostics) {
  const rows = resolveRows(props);
  const columns = resolveColumns(props, rows);
  const mapping = resolveMapping(props, columns);
  const legacy = resolveLegacyBehavior(props);
  const option = buildOption(kind, rows, mapping, legacy, diagnostics);
  return {
    option,
    meta: `${mapping.titleLeft} -> ${mapping.titleRight}`,
  };
}

function resolveRows(props) {
  const candidates = [props.data, props.value];
  for (const source of candidates) {
    if (!source || typeof source !== "object") continue;
    if (Array.isArray(source.rows)) return source.rows;
    if (Array.isArray(source.value)) return source.value;
    if (source.shape === "scalar" && source.value && typeof source.value === "object") {
      return Object.entries(source.value).map(([label, value]) => ({
        label,
        value,
      }));
    }
  }
  const dataset = props.dataset?.dataset || props.dataset || {};
  if (Array.isArray(dataset.rows)) {
    return dataset.rows;
  }
  return [];
}

function resolveColumns(props, rows) {
  const candidates = [props.data, props.value, props.dataset];
  for (const source of candidates) {
    if (!source || typeof source !== "object") continue;
    if (Array.isArray(source.schema) && source.schema.length > 0) {
      return source.schema.map((col) => col?.name).filter(Boolean);
    }
    if (Array.isArray(source.columns) && source.columns.length > 0) {
      return source.columns;
    }
    if (source.dataset && Array.isArray(source.dataset.columns)) {
      return source.dataset.columns;
    }
  }
  if (rows.length > 0 && rows[0] && typeof rows[0] === "object") {
    return Object.keys(rows[0]);
  }
  return ["label", "value"];
}

function resolveMapping(props, columns) {
  const mapping = props.mapping || {};
  const x = channelList(mapping.x, props.labelField, columns[0] || "label");
  let y = channelList(mapping.y, props.valueField, columns[1] || "value");
  if (y.length === 0 && Array.isArray(props.metrics) && props.metrics.length > 0) {
    y = props.metrics
      .map((field) => String(field || "").trim())
      .filter(Boolean)
      .map((field) => ({ field, name: field }));
  }
  const label = channelList(mapping.label, null, x[0]?.field || "label");
  const group = channelList(mapping.group, null, "");
  const color = channelList(mapping.color, null, "");
  const shape = channelList(mapping.shape, null, "");
  const size = channelList(mapping.size, null, "");
  let boxplot = channelList(mapping.boxplot, null, "");
  if (boxplot.length === 0 && Array.isArray(props.boxplot)) {
    boxplot = props.boxplot
      .map((field) => String(field || "").trim())
      .filter(Boolean)
      .map((field) => ({ field, name: field }));
  }
  const radarDimensions = Array.isArray(props.dimensions)
    ? props.dimensions.map((field) => ({ field, name: field }))
    : [];
  return {
    x,
    y,
    label,
    group,
    color,
    shape,
    size,
    boxplot,
    radarDimensions,
    titleLeft: x[0]?.name || label[0]?.name || "x",
    titleRight: y[0]?.name || "y",
  };
}

function resolveLegacyBehavior(props) {
  return {
    stack: !!props.stack,
    percent: props.transform?.mode === "percent",
    dataZoom: props.dataZoom !== false,
    metrics: Array.isArray(props.metrics) ? props.metrics : [],
  };
}

function channelList(channel, legacyField, fallbackField) {
  if (Array.isArray(channel) && channel.length > 0) {
    return channel
      .map((item) => {
        if (!item || typeof item !== "object") return null;
        const field = item.field || item.value || "";
        if (!field) return null;
        return { field, name: item.name || field };
      })
      .filter(Boolean);
  }
  const field = legacyField || fallbackField;
  if (!field) return [];
  return [{ field, name: field }];
}

function buildOption(kind, rows, mapping, legacy, diagnostics) {
  const chartKind = normalizeKind(kind);
  if (chartKind === "radar") {
    return buildRadarOption(rows, mapping, legacy, diagnostics);
  }
  if (chartKind === "boxplot") {
    return buildBoxplotOption(rows, mapping, legacy, diagnostics);
  }
  if (PIE_KINDS.has(chartKind)) {
    return buildPieOption(chartKind, rows, mapping, diagnostics);
  }
  if (chartKind === "scatter") {
    return buildScatterOption(rows, mapping, diagnostics);
  }
  return buildCartesianOption(chartKind, rows, mapping, legacy, diagnostics);
}

function buildCartesianOption(kind, rows, mapping, legacy, diagnostics) {
  const xField = mapping.x[0]?.field;
  if (!xField) {
    diagnostics.push("缺少 mapping.x");
  }
  const yFields = mapping.y.length > 0 ? mapping.y.map((item) => item.field) : inferYFields(rows, xField);
  if (yFields.length === 0) {
    diagnostics.push("缺少 mapping.y");
  }
  if (kind === "trend" && yFields.length !== 1) {
    diagnostics.push("chart.trend 需要且仅支持一个 y 通道");
  }
  const categories = unique(rows.map((row) => String(row?.[xField] ?? ""))).filter(Boolean);
  const grouped = mapping.group[0]?.field;
  const groups = grouped ? unique(rows.map((row) => String(row?.[grouped] ?? ""))).filter(Boolean) : [];
  const series = [];
  const isBar = kind === "column" || kind === "bar";
  const seriesType = isBar ? "bar" : "line";
  for (const yField of yFields) {
    if (groups.length === 0) {
      series.push({
        name: yField,
        type: seriesType,
        smooth: kind === "trend",
        areaStyle: kind === "area" ? {} : undefined,
        stack: legacy.stack ? "total" : undefined,
        data: categories.map((category) => aggregateValue(rows, xField, category, yField)),
      });
    } else {
      for (const groupName of groups) {
        series.push({
          name: `${groupName} · ${yField}`,
          type: seriesType,
          smooth: kind === "trend",
          areaStyle: kind === "area" ? {} : undefined,
          stack: legacy.stack ? "total" : undefined,
          data: categories.map((category) =>
            aggregateValue(rows, xField, category, yField, grouped, groupName),
          ),
        });
      }
    }
  }
  if (legacy.percent) {
    applyPercentTransform(series);
  }
  const option = {
    tooltip: { trigger: "axis" },
    legend: { top: 0 },
    toolbox: { feature: { saveAsImage: {} } },
    grid: { left: 44, right: 22, top: 38, bottom: 34 },
    xAxis: kind === "bar" ? { type: "value" } : { type: "category", data: categories },
    yAxis: kind === "bar" ? { type: "category", data: categories } : { type: "value" },
    series,
  };
  if (kind === "bar") {
    option.series = option.series.map((item) => ({
      ...item,
      data: categories.map((category) => {
        const value = item.data[categories.indexOf(category)] || 0;
        return value;
      }),
    }));
  }
  if (legacy.dataZoom && categories.length > 16) {
    option.dataZoom = [{ type: "inside" }, { type: "slider" }];
  }
  return option;
}

function buildPieOption(kind, rows, mapping, diagnostics) {
  const labelField = mapping.label[0]?.field || mapping.x[0]?.field;
  const valueField = mapping.y[0]?.field;
  if (!labelField || !valueField) {
    diagnostics.push("pie/donut/rose 需要 mapping.label(x) 与 mapping.y");
  }
  const data = rows
    .map((row) => ({
      name: String(row?.[labelField] ?? ""),
      value: toNumber(row?.[valueField]),
    }))
    .filter((item) => item.name && Number.isFinite(item.value));
  return {
    tooltip: { trigger: "item" },
    legend: { top: 0 },
    toolbox: { feature: { saveAsImage: {} } },
    series: [
      {
        type: "pie",
        radius: kind === "donut" ? ["45%", "72%"] : "70%",
        roseType: kind === "rose" ? "radius" : undefined,
        data,
      },
    ],
  };
}

function buildScatterOption(rows, mapping, diagnostics) {
  const xField = mapping.x[0]?.field;
  const yField = mapping.y[0]?.field;
  const sizeField = mapping.size[0]?.field;
  const colorField = mapping.color[0]?.field;
  if (!xField || !yField) {
    diagnostics.push("scatter 需要 mapping.x 与 mapping.y");
  }
  const groups = colorField
    ? unique(rows.map((row) => String(row?.[colorField] ?? "")).filter(Boolean))
    : [""];
  const series = groups.map((groupName) => {
    const points = rows
      .filter((row) => !colorField || String(row?.[colorField] ?? "") === groupName)
      .map((row) => {
        const point = [toNumber(row?.[xField]), toNumber(row?.[yField])];
        if (sizeField) point.push(toNumber(row?.[sizeField]));
        return point;
      })
      .filter((item) => Number.isFinite(item[0]) && Number.isFinite(item[1]));
    return {
      name: groupName || (mapping.y[0]?.name || yField),
      type: "scatter",
      symbolSize: (value) => {
        if (!sizeField) return 12;
        const size = Number(value?.[2]);
        if (!Number.isFinite(size)) return 10;
        return Math.max(6, Math.min(24, size / 8));
      },
      data: points,
    };
  });
  return {
    tooltip: { trigger: "item" },
    legend: { top: 0, show: !!colorField },
    toolbox: { feature: { saveAsImage: {} } },
    xAxis: { type: "value" },
    yAxis: { type: "value" },
    series,
  };
}

function buildRadarOption(rows, mapping, legacy, diagnostics) {
  const dimensions = mapping.radarDimensions.length > 0
    ? mapping.radarDimensions
    : mapping.y;
  if (dimensions.length === 0) {
    diagnostics.push("radar 需要 dimensions 或 mapping.y");
  }
  const indicators = dimensions.map((item) => ({
    name: item.name || item.field,
    max: maxByField(rows, item.field) || 100,
  }));
  const labelField = mapping.label[0]?.field || mapping.x[0]?.field;
  const data = rows.slice(0, 12).map((row, index) => ({
    name: labelField ? String(row?.[labelField] ?? `item-${index + 1}`) : `item-${index + 1}`,
    value: dimensions.map((dim) => toNumber(row?.[dim.field])),
  }));
  if (legacy.percent) {
    normalizeRadarData(data);
  }
  return {
    tooltip: { trigger: "item" },
    legend: { top: 0 },
    toolbox: { feature: { saveAsImage: {} } },
    radar: { indicator: indicators },
    series: [{ type: "radar", data }],
  };
}

function buildBoxplotOption(rows, mapping, _legacy, diagnostics) {
  const labelField = mapping.x[0]?.field || "label";
  const def = mapping.boxplot;
  if (def.length >= 5) {
    const [minField, q1Field, medianField, q3Field, maxField] = def.map((item) => item.field);
    const labels = rows.map((row) => String(row?.[labelField] ?? ""));
    const data = rows.map((row) => [
      toNumber(row?.[minField]),
      toNumber(row?.[q1Field]),
      toNumber(row?.[medianField]),
      toNumber(row?.[q3Field]),
      toNumber(row?.[maxField]),
    ]);
    return {
      tooltip: { trigger: "item" },
      xAxis: { type: "category", data: labels },
      yAxis: { type: "value" },
      series: [{ type: "boxplot", data }],
    };
  }
  const valueField = mapping.y[0]?.field;
  if (!valueField) {
    diagnostics.push("boxplot 需要 mapping.boxplot 或 mapping.y");
  }
  const grouped = groupNumbersBy(rows, labelField, valueField);
  const labels = Object.keys(grouped);
  const data = labels.map((label) => toBoxStats(grouped[label]));
  return {
    tooltip: { trigger: "item" },
    xAxis: { type: "category", data: labels },
    yAxis: { type: "value" },
    series: [{ type: "boxplot", data }],
  };
}

function aggregateValue(rows, xField, category, yField, groupField, groupName) {
  const values = rows
    .filter((row) => {
      if (String(row?.[xField] ?? "") !== category) return false;
      if (!groupField) return true;
      return String(row?.[groupField] ?? "") === groupName;
    })
    .map((row) => toNumber(row?.[yField]))
    .filter((value) => Number.isFinite(value));
  if (values.length === 0) return 0;
  return values.reduce((sum, value) => sum + value, 0);
}

function applyPercentTransform(series) {
  if (!Array.isArray(series) || series.length === 0) return;
  const points = Math.max(...series.map((item) => item.data.length));
  for (let idx = 0; idx < points; idx += 1) {
    const total = series.reduce((sum, item) => sum + toNumber(item.data[idx]), 0);
    if (total <= 0) continue;
    for (const item of series) {
      item.data[idx] = Number(((toNumber(item.data[idx]) / total) * 100).toFixed(2));
    }
  }
}

function inferYFields(rows, xField) {
  if (!Array.isArray(rows) || rows.length === 0) return [];
  const first = rows.find((row) => row && typeof row === "object");
  if (!first) return [];
  return Object.keys(first).filter((key) => key !== xField && Number.isFinite(toNumber(first[key])));
}

function normalizeKind(kind) {
  if (kind === "chart.trend") return "trend";
  return String(kind || "").replace("chart.", "");
}

function groupNumbersBy(rows, keyField, valueField) {
  const out = {};
  for (const row of rows) {
    const key = String(row?.[keyField] ?? "");
    const value = toNumber(row?.[valueField]);
    if (!key || !Number.isFinite(value)) continue;
    if (!out[key]) out[key] = [];
    out[key].push(value);
  }
  return out;
}

function toBoxStats(values) {
  const sorted = [...values].sort((a, b) => a - b);
  if (sorted.length === 0) return [0, 0, 0, 0, 0];
  return [
    sorted[0],
    quantile(sorted, 0.25),
    quantile(sorted, 0.5),
    quantile(sorted, 0.75),
    sorted[sorted.length - 1],
  ];
}

function quantile(sorted, q) {
  const pos = (sorted.length - 1) * q;
  const base = Math.floor(pos);
  const rest = pos - base;
  if (sorted[base + 1] !== undefined) {
    return sorted[base] + rest * (sorted[base + 1] - sorted[base]);
  }
  return sorted[base];
}

function normalizeRadarData(data) {
  if (!Array.isArray(data) || data.length === 0) return;
  const dims = data[0].value.length;
  for (let idx = 0; idx < dims; idx += 1) {
    const max = Math.max(...data.map((item) => toNumber(item.value[idx])), 1);
    for (const item of data) {
      item.value[idx] = Number(((toNumber(item.value[idx]) / max) * 100).toFixed(2));
    }
  }
}

function maxByField(rows, field) {
  const values = rows.map((row) => toNumber(row?.[field])).filter((value) => Number.isFinite(value));
  if (values.length === 0) return 0;
  return Math.max(...values);
}

function parseProps(element) {
  try {
    return JSON.parse(element.dataset.props || "{}");
  } catch {
    return {};
  }
}

function toNumber(value) {
  const n = Number(value);
  return Number.isFinite(n) ? n : NaN;
}

function unique(items) {
  return [...new Set(items)];
}

function escapeHtml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;");
}
