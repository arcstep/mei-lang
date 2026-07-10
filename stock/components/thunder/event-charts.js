/**
 * Thunder 时序三图（ECharts）：随选中事件换套。
 * - 级别阶梯：柱色 = 黄/橙/红
 * - Eabs / 闪频：markLine 产品阈值参考线（完整语义标签）
 * Fill-down + 主题字号；playbackAt 高亮当前切片。
 */
import { deferUntilDisplayed } from "../dataset/runtime-query.js";
import { parseProps, escapeHtml } from "../cockpit/shared.js";
import {
  COCKPIT_TYPE,
  cockpitCssVars,
  readThemeColor,
  readThemeTypography,
  readThemeUiFontFamily,
} from "../cockpit/tokens.js";
import { color } from "../mei/theme-style.js";
import { ensureEChartsGlobal } from "../vendor/runtime-libs.js";
import { getThunderStore, subscribeThunderState } from "./event-bus.js";
import {
  EFIELD_ABS_THRESHOLDS,
  LIGHTNING_FREQ_THRESHOLDS,
  levelCodeColor,
} from "./thresholds.js";

const SPLIT_LINE = {
  show: true,
  lineStyle: { color: "rgba(148, 163, 184, 0.18)", type: "dashed", width: 1 },
};

const LEVEL_LABEL = { 1: "黄", 2: "橙", 3: "红" };

const PANELS = [
  {
    key: "lifecycle",
    title: "预警级别（黄 / 橙 / 红）",
    field: "级别",
    unit: "",
    /** 仅三档离散值：矮行 + 精简坐标 */
    compactLevel: true,
    yMaxFixed: 3,
    colorMode: "level",
    thresholds: null,
  },
  {
    key: "efield",
    title: "Eabs 电场强度 · 参考 3/7/9 kV/m",
    field: "Eabs",
    unit: " kV/m",
    colorMode: "solid",
    solidColorToken: "chart_2",
    solidFallback: "#38bdf8",
    thresholds: EFIELD_ABS_THRESHOLDS,
  },
  {
    key: "frequency",
    title: "闪电频次 5min · 参考 ≥1次 / >3次 / >5次",
    field: "次数",
    unit: " 次",
    colorMode: "solid",
    solidColorToken: "status_error",
    solidFallback: "#f87171",
    thresholds: LIGHTNING_FREQ_THRESHOLDS,
  },
];

function maxOf(rows, field) {
  let max = 0;
  for (const row of rows) {
    const n = Number(row?.[field]);
    if (Number.isFinite(n) && n > max) max = n;
  }
  return max || 0;
}

function scaleMax(dataMax, thresholds, fixed) {
  if (Number.isFinite(fixed) && fixed > 0) return fixed;
  let max = Math.max(1, Number(dataMax) || 1);
  for (const t of thresholds || []) {
    const v = Number(t?.value);
    if (Number.isFinite(v) && v > max) max = v;
  }
  return Math.ceil(max * 1.08 * 10) / 10;
}

function markLineFromThresholds(thresholds, typography) {
  const list = Array.isArray(thresholds) ? thresholds : [];
  if (!list.length) return undefined;
  return {
    symbol: "none",
    silent: true,
    animation: false,
    data: list.map((t) => ({
      yAxis: Number(t.value),
      name: String(t.label || t.tag || t.value),
      lineStyle: {
        type: "dashed",
        width: 1.25,
        color: t.color,
        opacity: 0.92,
      },
      label: {
        show: true,
        formatter: String(t.tag || t.label || t.value),
        position: "insideStartTop",
        color: t.color,
        fontSize: Math.max(10, Math.min(12, (typography?.unit || 12) - 2)),
        fontWeight: 600,
        backgroundColor: "rgba(8, 24, 48, 0.78)",
        padding: [1, 4],
        borderRadius: 2,
      },
    })),
  };
}

function buildBarOption({ host, rows, panel, playbackAt }) {
  const list = Array.isArray(rows) ? rows : [];
  const typography = readThemeTypography(host);
  const muted = readThemeColor(host, "text_muted") || "#94a3b8";
  const body = readThemeColor(host, "text_body") || "#e2e8f0";
  const fontFamily = readThemeUiFontFamily(host) || "sans-serif";
  const categories = list.map((row) => String(row?.["时段"] ?? ""));
  const dataMax = maxOf(list, panel.field);
  const yMax = scaleMax(dataMax, panel.thresholds, panel.yMaxFixed);
  const compactLevel = panel.compactLevel === true;
  const solid =
    panel.colorMode === "solid"
      ? readThemeColor(host, panel.solidColorToken) || panel.solidFallback
      : panel.solidFallback;
  const labelSize = Math.max(10, (typography.unit || 12) - (compactLevel ? 2 : 1));

  const data = list.map((row) => {
    const x = String(row?.["时段"] ?? "");
    const v = Number(row?.[panel.field] ?? 0);
    const active = x === playbackAt;
    const barColor =
      panel.colorMode === "level" ? levelCodeColor(row?.["级别"]) : solid;
    const levelText = LEVEL_LABEL[v] || "";
    return {
      value: Number.isFinite(v) ? v : 0,
      itemStyle: {
        color: barColor,
        opacity: active ? 1 : 0.78,
        borderColor: active ? "rgba(255,255,255,0.55)" : "transparent",
        borderWidth: active ? 1 : 0,
        borderRadius: compactLevel ? [2, 2, 0, 0] : [2, 2, 0, 0],
      },
      label: compactLevel
        ? {
            show: true,
            position: "top",
            formatter: levelText,
            color: barColor,
            fontSize: labelSize,
            fontWeight: active ? 700 : 600,
            distance: 2,
          }
        : undefined,
    };
  });

  const markLine = compactLevel ? undefined : markLineFromThresholds(panel.thresholds, typography);

  return {
    backgroundColor: "transparent",
    animationDuration: compactLevel ? 160 : 280,
    textStyle: { fontFamily, color: muted },
    grid: compactLevel
      ? { left: 6, right: 6, top: 16, bottom: 16, containLabel: false }
      : {
          left: 28,
          right: 8,
          top: markLine ? 18 : 10,
          bottom: 22,
          containLabel: false,
        },
    tooltip: {
      trigger: "axis",
      axisPointer: { type: compactLevel ? "line" : "shadow" },
      backgroundColor: "rgba(8, 24, 48, 0.92)",
      borderColor: "rgba(56, 189, 248, 0.45)",
      borderWidth: 1,
      textStyle: {
        color: body,
        fontSize: typography.label || 12,
        fontFamily,
      },
      formatter(params) {
        const items = Array.isArray(params) ? params : [params];
        const head = items[0];
        if (!head) return "";
        const name = String(head.axisValueLabel ?? head.name ?? "");
        if (compactLevel) {
          const code = Number(head.value);
          const label = LEVEL_LABEL[code] || String(head.value ?? "");
          return `<div>${escapeHtml(name)}</div><div>级别：<b style="color:${levelCodeColor(code)}">${escapeHtml(label)}</b></div>`;
        }
        const lines = items
          .filter((p) => p.seriesType === "bar")
          .map((p) => `${p.marker}${panel.field}: <b>${p.value}${panel.unit || ""}</b>`);
        const refs = (panel.thresholds || [])
          .map((t) => `<span style="color:${t.color}">${escapeHtml(t.tag)}</span>`)
          .join(" · ");
        return [`<div>${escapeHtml(name)}</div>`, ...lines, refs ? `<div style="margin-top:4px;opacity:.85">${refs}</div>` : ""]
          .filter(Boolean)
          .join("");
      },
    },
    xAxis: {
      type: "category",
      data: categories,
      axisTick: { show: false },
      axisLine: {
        show: !compactLevel,
        lineStyle: { color: "rgba(148,163,184,0.35)" },
      },
      axisLabel: {
        show: !compactLevel || categories.length <= 8,
        color: muted,
        fontSize: labelSize,
        fontFamily,
        hideOverlap: true,
        interval: compactLevel ? "auto" : 0,
        rotate: !compactLevel && categories.length > 8 ? 30 : 0,
      },
    },
    yAxis: compactLevel
      ? {
          type: "value",
          min: 0,
          max: 3,
          interval: 1,
          axisTick: { show: false },
          axisLine: { show: false },
          axisLabel: { show: false },
          splitLine: { show: false },
        }
      : {
          type: "value",
          min: 0,
          max: yMax,
          splitNumber: 4,
          axisTick: { show: false },
          axisLine: { show: false },
          axisLabel: {
            color: muted,
            fontSize: labelSize,
            fontFamily,
            formatter: (v) => String(v),
          },
          splitLine: SPLIT_LINE,
        },
    series: [
      {
        type: "bar",
        name: panel.field,
        data,
        barMaxWidth: compactLevel ? 14 : 18,
        barCategoryGap: compactLevel ? "36%" : "28%",
        markLine,
        z: 2,
      },
    ],
  };
}

class MeiThunderEventCharts extends HTMLElement {
  connectedCallback() {
    this._props = parseProps(this);
    this._charts = [];
    this._state = getThunderStore();
    this.style.display = "block";
    this.style.width = "100%";
    this.style.height = "100%";
    this.style.minHeight = "0";
    this.style.overflow = "hidden";
    this.style.boxSizing = "border-box";
    if (!this.shadowRoot) this.attachShadow({ mode: "open" });
    this._unsub = subscribeThunderState((detail) => {
      this._state = detail || getThunderStore();
      this.schedulePaint();
    });
    this._deferUntilVisibleCleanup = deferUntilDisplayed(this, () => {
      this._deferUntilVisibleCleanup = null;
      this.bootstrap();
    });
  }

  bootstrap() {
    this.renderShell();
    this.resizeObserver = new ResizeObserver(() => {
      this._charts.forEach((chart) => chart?.resize?.());
    });
    this.resizeObserver.observe(this);
    this.schedulePaint();
  }

  disconnectedCallback() {
    if (typeof this._deferUntilVisibleCleanup === "function") {
      this._deferUntilVisibleCleanup();
      this._deferUntilVisibleCleanup = null;
    }
    if (typeof this._unsub === "function") {
      this._unsub();
      this._unsub = null;
    }
    if (this._paintRaf != null) {
      cancelAnimationFrame(this._paintRaf);
      this._paintRaf = null;
    }
    if (this.resizeObserver) {
      this.resizeObserver.disconnect();
      this.resizeObserver = null;
    }
    this._charts.forEach((chart) => chart?.dispose?.());
    this._charts = [];
  }

  renderShell() {
    this.shadowRoot.innerHTML = `
      <style>
        :host {
          display: block;
          width: 100%;
          height: 100%;
          min-height: 0;
          max-height: 100%;
          overflow: hidden;
          box-sizing: border-box;
          ${cockpitCssVars()}
          font-family: var(--cockpit-font-family-ui);
        }
        .stack {
          display: grid;
          /* 级别仅三档：约占半行；电场/闪频吃满剩余 */
          grid-template-rows: minmax(0, 0.5fr) minmax(0, 1fr) minmax(0, 1fr);
          gap: 6px;
          width: 100%;
          height: 100%;
          min-height: 0;
          max-height: 100%;
          box-sizing: border-box;
        }
        .panel {
          display: flex;
          flex-direction: column;
          min-width: 0;
          min-height: 0;
          max-height: 100%;
          padding: 4px 6px 2px;
          border-radius: 4px;
          background: rgba(10, 40, 78, 0.72);
          border: 1px solid rgba(56, 160, 240, 0.28);
          box-sizing: border-box;
          overflow: hidden;
        }
        .panel[data-panel="lifecycle"] {
          padding: 3px 6px 2px;
        }
        .panel[data-panel="lifecycle"] .title {
          margin-bottom: 0;
        }
        .title {
          flex: 0 0 auto;
          font-size: ${COCKPIT_TYPE.chartTitle};
          line-height: 1.2;
          color: ${color("text_muted")};
          margin-bottom: 2px;
          white-space: nowrap;
          overflow: hidden;
          text-overflow: ellipsis;
        }
        .chart {
          flex: 1 1 auto;
          width: 100%;
          min-height: 0;
          min-width: 0;
          position: relative;
        }
        .chart .empty {
          position: absolute;
          inset: 0;
          display: flex;
          align-items: center;
          justify-content: center;
          color: ${color("text_muted")};
          font-size: ${COCKPIT_TYPE.chartLabel};
        }
        .empty {
          flex: 1;
          display: flex;
          align-items: center;
          justify-content: center;
          color: ${color("text_muted")};
          font-size: ${COCKPIT_TYPE.chartLabel};
          min-height: 0;
        }
      </style>
      <div class="stack">
        ${PANELS.map(
          (panel, idx) => `
          <div class="panel" data-panel="${escapeHtml(panel.key)}">
            <div class="title">${escapeHtml(panel.title)}</div>
            <div class="chart" data-idx="${idx}"></div>
          </div>`,
        ).join("")}
      </div>
    `;
    this._chartEls = [...this.shadowRoot.querySelectorAll(".chart")];
  }

  schedulePaint() {
    if (this._paintRaf != null) cancelAnimationFrame(this._paintRaf);
    this._paintRaf = requestAnimationFrame(() => {
      this._paintRaf = null;
      this.paint().catch(() => {});
    });
  }

  async paint() {
    if (!this.shadowRoot || !this._chartEls?.length) return;
    const echarts = await ensureEChartsGlobal();
    if (!this.isConnected) return;
    const event = this._state?.event || null;
    const playbackAt = String(this._state?.playbackAt || "").trim();
    const charts = event?.charts || {};
    const seriesByKey = {
      lifecycle: charts.lifecycle,
      efield: charts.efield,
      frequency: charts.frequency,
    };

    for (let i = 0; i < PANELS.length; i += 1) {
      const panel = PANELS[i];
      const el = this._chartEls[i];
      if (!el) continue;
      const rows = seriesByKey[panel.key];
      const empty = !Array.isArray(rows) || rows.length === 0;

      if (empty) {
        if (this._charts[i]) {
          this._charts[i].dispose();
          this._charts[i] = null;
        }
        el.innerHTML = `<div class="empty">无数据</div>`;
        continue;
      }

      if (el.querySelector(".empty")) el.innerHTML = "";
      let chart = this._charts[i];
      if (!chart) {
        chart = echarts.init(el, null, { renderer: "canvas" });
        this._charts[i] = chart;
      }
      chart.setOption(
        buildBarOption({
          host: this,
          rows,
          panel,
          playbackAt,
        }),
        true,
      );
      chart.resize();
    }
  }
}

if (!customElements.get("mei-thunder-event-charts")) {
  customElements.define("mei-thunder-event-charts", MeiThunderEventCharts);
}
